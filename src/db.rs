//! Database jobs. The server's pool lives only inside `Database`, so every
//! request-time connection is counted against one of its slot limits.
//! Writes to CMS data take `&mut Locked`, which only a `cms_lock` transaction
//! hands out.
use std::sync::Arc;

use axum::http::StatusCode;
use mysql::{
    Binary, Opts, Params, Pool, PooledConn, QueryResult, Statement, Text, Transaction, TxOpts,
    prelude::{AsStatement, Queryable},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::{ApiError, Result, db_error};

/// Limit on editorial jobs per Database.
const EDITORIAL_JOBS: usize = 16;
/// Limit on public reads per Database.
const PUBLIC_JOBS: usize = 12;
/// Limit on sign-in and session queries per Database.
const SESSION_JOBS: usize = 4;

/// Connections a server pool needs so every slot can hold one.
pub(crate) const CONNECTIONS: usize = EDITORIAL_JOBS + PUBLIC_JOBS + SESSION_JOBS;

/// Fails when the URL's pool_max cannot give `min_connections` jobs a connection each.
pub(crate) fn connect(
    database_url: &str,
    min_connections: usize,
) -> std::result::Result<Pool, String> {
    let opts = Opts::from_url(database_url).map_err(|_| "DATABASE_URL must be a MySQL URL")?;
    let max = opts.get_pool_opts().constraints().max();
    if max < min_connections {
        return Err(format!(
            "DATABASE_URL pool_max is {max}; it must be at least {min_connections} so every database job can get a connection"
        ));
    }
    Pool::new(opts).map_err(|e| {
        db_error(e);
        "could not connect to MySQL".into()
    })
}

#[derive(Clone)]
pub(crate) struct Database {
    pool: Pool,
    /// Editor-side jobs: locked transactions, package work and scheduled publishing.
    editorial: Arc<Semaphore>,
    /// Lock-free public reads: content, alias lookups and health checks.
    public: Arc<Semaphore>,
    /// Sign-in and session queries, which run before any editorial job.
    sessions: Arc<Semaphore>,
}

impl Database {
    pub(crate) fn new(pool: Pool) -> Self {
        Self::with_jobs(pool, [EDITORIAL_JOBS, PUBLIC_JOBS, SESSION_JOBS])
    }

    fn with_jobs(pool: Pool, [editorial, public, sessions]: [usize; 3]) -> Self {
        // Bound running and queued blocking jobs independently of Tokio's thread pool.
        // Each kind of job has its own limit, so one kind cannot starve another.
        Self {
            pool,
            editorial: Arc::new(Semaphore::new(editorial)),
            public: Arc::new(Semaphore::new(public)),
            sessions: Arc::new(Semaphore::new(sessions)),
        }
    }

    /// Editorial work inside the `cms_lock` transaction.
    pub(crate) async fn run<T, F>(&self, f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Locked<'_>) -> Result<T> + Send + 'static,
    {
        self.blocking(slot(&self.editorial)?, move |db| {
            transaction(db, TransactionMode::Commit, f)
        })
        .await
    }

    /// Editorial work that must not wait on, or hold, the `cms_lock` row.
    pub(crate) async fn unlocked<T, F>(&self, f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut PooledConn) -> Result<T> + Send + 'static,
    {
        self.blocking(slot(&self.editorial)?, move |db| {
            f(&mut db.get_conn().map_err(db_error)?)
        })
        .await
    }

    /// Sign-in and session queries. These wait for a slot instead of failing,
    /// so a burst of sign-ins queues rather than turning into errors.
    pub(crate) async fn sessions<T, F>(&self, f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut PooledConn) -> Result<T> + Send + 'static,
    {
        let permit = self.sessions.clone().acquire_owned().await.unwrap();
        self.blocking(permit, move |db| f(&mut db.get_conn().map_err(db_error)?))
            .await
    }

    /// Public, lock-free reads.
    pub(crate) async fn read<T, F>(&self, f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut PooledConn) -> Result<T> + Send + 'static,
    {
        self.blocking(slot(&self.public)?, move |db| {
            f(&mut db.get_conn().map_err(db_error)?)
        })
        .await
    }

    async fn blocking<T, F>(&self, permit: OwnedSemaphorePermit, f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&Pool) -> Result<T> + Send + 'static,
    {
        let db = self.pool.clone();
        tokio::task::spawn_blocking(move || {
            // Keep the permit until the job ends, even if its HTTP request is cancelled.
            let _permit = permit;
            f(&db)
        })
        .await
        .map_err(|_| {
            ApiError(
                StatusCode::INTERNAL_SERVER_ERROR,
                "database task failed".into(),
            )
        })?
    }
}

fn slot(slots: &Arc<Semaphore>) -> Result<OwnedSemaphorePermit> {
    slots.clone().try_acquire_owned().map_err(|_| {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "database is busy; retry later".into(),
        )
    })
}

/// A transaction holding the `cms_lock` row. Only `transaction` creates one.
pub(crate) struct Locked<'a>(Transaction<'a>);

impl Locked<'_> {
    pub(crate) fn affected_rows(&self) -> u64 {
        self.0.affected_rows()
    }
}

impl Queryable for Locked<'_> {
    fn query_iter<Q: AsRef<str>>(
        &mut self,
        query: Q,
    ) -> mysql::Result<QueryResult<'_, '_, '_, Text>> {
        self.0.query_iter(query)
    }

    fn prep<Q: AsRef<str>>(&mut self, query: Q) -> mysql::Result<Statement> {
        self.0.prep(query)
    }

    fn close(&mut self, stmt: Statement) -> mysql::Result<()> {
        self.0.close(stmt)
    }

    fn exec_iter<S, P>(
        &mut self,
        stmt: S,
        params: P,
    ) -> mysql::Result<QueryResult<'_, '_, '_, Binary>>
    where
        S: AsStatement,
        P: Into<Params>,
    {
        self.0.exec_iter(stmt, params)
    }

    fn exec_batch<S, P, I>(&mut self, stmt: S, params: I) -> mysql::Result<()>
    where
        S: AsStatement,
        P: Into<Params>,
        I: IntoIterator<Item = P>,
    {
        self.0.exec_batch(stmt, params)
    }
}

#[derive(Clone, Copy)]
pub(crate) enum TransactionMode {
    Commit,
    Rollback,
}

/// Runs `f` holding the `cms_lock` row. Synchronous, for startup and the CLI.
pub(crate) fn transaction<T>(
    db: &Pool,
    mode: TransactionMode,
    f: impl FnOnce(&mut Locked<'_>) -> Result<T>,
) -> Result<T> {
    let mut conn = db.get_conn().map_err(db_error)?;
    let tx = conn
        .start_transaction(TxOpts::default())
        .map_err(db_error)?;
    let mut tx = Locked(tx);
    // All server and CLI writes share the same lock. Errors roll back the batch.
    tx.query_drop("SELECT id FROM cms_lock WHERE id=1 FOR UPDATE")
        .map_err(db_error)?;
    let result = f(&mut tx)?;
    match mode {
        TransactionMode::Commit => tx.0.commit(),
        TransactionMode::Rollback => tx.0.rollback(),
    }
    .map_err(db_error)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn cancelled_requests_keep_their_database_slot_until_work_finishes() {
        // An empty pool is sufficient: this test never connects to a database.
        let db = Database::with_jobs(
            Pool::new(Opts::from_url("mysql://127.0.0.1/test?pool_min=0&pool_max=1").unwrap())
                .unwrap(),
            [1, 1, 1],
        );
        let (started, ready) = tokio::sync::oneshot::channel();
        let (finish, wait) = std::sync::mpsc::channel();
        let running = {
            let db = db.clone();
            tokio::spawn(async move {
                db.blocking(slot(&db.editorial)?, move |_| {
                    started.send(()).unwrap();
                    wait.recv_timeout(Duration::from_secs(5)).unwrap();
                    Ok(())
                })
                .await
            })
        };
        ready.await.unwrap();
        assert_eq!(
            slot(&db.editorial).unwrap_err().0,
            StatusCode::SERVICE_UNAVAILABLE
        );
        running.abort();
        assert!(running.await.unwrap_err().is_cancelled());
        assert_eq!(
            slot(&db.editorial).unwrap_err().0,
            StatusCode::SERVICE_UNAVAILABLE
        );
        finish.send(()).unwrap();
        let permit = tokio::time::timeout(Duration::from_secs(2), db.editorial.acquire())
            .await
            .unwrap()
            .unwrap();
        drop(permit);
        assert_eq!(
            db.blocking(slot(&db.editorial).unwrap(), |_| Ok(42))
                .await
                .unwrap(),
            42
        );
    }
}
