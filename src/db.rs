//! Database jobs. The server's pool lives only inside `Database`, so every
//! request-time connection is counted against one of its slot limits.
//! Writes to CMS data take `&mut Locked`, which only a `cms_lock` transaction
//! hands out. Also owns schema migrations and the JSON row helpers.
use std::sync::Arc;

use axum::http::StatusCode;
use mysql::{
    Binary, Opts, Params, Pool, PooledConn, QueryResult, Statement, Text, Transaction, TxOpts,
    prelude::{AsStatement, Queryable},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use serde::Serialize;

use crate::{ApiError, Component, ComponentId, Page, PageId, Result, Template, TemplateId};

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

pub(crate) struct StoredData {
    pub(crate) pages: Vec<Page>,
    pub(crate) templates: Vec<Template>,
    pub(crate) components: Vec<Component>,
}

/// Schema changes, applied in order at startup. Version N is entry N-1.
/// Append new statements to the end; never edit, reorder or remove applied ones.
/// Each entry runs once and is recorded in `schema_migrations`. Do not alter
/// `cms_lock` here; migrations run while holding its row lock.
const MIGRATIONS: &[&str] = &[
    // 1-10: baseline. `IF NOT EXISTS` lets databases created before versioning adopt it.
    "CREATE TABLE IF NOT EXISTS components(id VARCHAR(255) COLLATE utf8mb4_bin PRIMARY KEY, data LONGTEXT NOT NULL) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
    "CREATE TABLE IF NOT EXISTS templates(id VARCHAR(255) COLLATE utf8mb4_bin PRIMARY KEY, data LONGTEXT NOT NULL) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
    "CREATE TABLE IF NOT EXISTS pages(id VARCHAR(255) COLLATE utf8mb4_bin PRIMARY KEY, slug VARBINARY(2048) NOT NULL UNIQUE, data LONGTEXT NOT NULL) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
    "CREATE TABLE IF NOT EXISTS snapshots(page_id VARCHAR(255) COLLATE utf8mb4_bin PRIMARY KEY, slug VARBINARY(2048) NOT NULL UNIQUE, data LONGTEXT NOT NULL, FOREIGN KEY(page_id) REFERENCES pages(id) ON DELETE CASCADE) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
    "CREATE TABLE IF NOT EXISTS route_aliases(slug VARBINARY(2048) PRIMARY KEY, page_id VARCHAR(255) COLLATE utf8mb4_bin NOT NULL, FOREIGN KEY(page_id) REFERENCES pages(id) ON DELETE CASCADE) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
    "CREATE TABLE IF NOT EXISTS organization(id VARCHAR(255) COLLATE utf8mb4_bin PRIMARY KEY, data LONGTEXT NOT NULL) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
    // What each Git export path held at this database's last pull or push. See serialization.
    "CREATE TABLE IF NOT EXISTS sync_base(path VARCHAR(255) COLLATE utf8mb4_bin PRIMARY KEY, hash CHAR(64) NOT NULL) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
    "CREATE TABLE IF NOT EXISTS reviews(id VARCHAR(255) COLLATE utf8mb4_bin PRIMARY KEY, data LONGTEXT NOT NULL, FOREIGN KEY(id) REFERENCES pages(id) ON DELETE CASCADE) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
    "CREATE TABLE IF NOT EXISTS accounts(id VARCHAR(255) COLLATE utf8mb4_bin PRIMARY KEY, password_hash VARCHAR(255) NOT NULL) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
    "CREATE TABLE IF NOT EXISTS sessions(token_hash CHAR(64) PRIMARY KEY, account_id VARCHAR(255) COLLATE utf8mb4_bin NOT NULL, expires_at BIGINT NOT NULL, FOREIGN KEY(account_id) REFERENCES accounts(id) ON DELETE CASCADE) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
    // 11: every draft slug and alias, so a write checks its own paths without loading every page. See claim_paths.
    "CREATE TABLE IF NOT EXISTS page_paths(path VARBINARY(2048) PRIMARY KEY, page_id VARCHAR(255) COLLATE utf8mb4_bin NOT NULL, INDEX(page_id), FOREIGN KEY(page_id) REFERENCES pages(id) ON DELETE CASCADE) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
    // 12: at most one snapshot per page waiting to go live at publish_at (Unix seconds). See schedule.
    "CREATE TABLE IF NOT EXISTS scheduled_publications(page_id VARCHAR(255) COLLATE utf8mb4_bin PRIMARY KEY, publish_at BIGINT NOT NULL, revision BIGINT NOT NULL, scheduled_by VARCHAR(255) COLLATE utf8mb4_bin NOT NULL, error TEXT NULL, data LONGTEXT NOT NULL, INDEX(publish_at), FOREIGN KEY(page_id) REFERENCES pages(id) ON DELETE CASCADE) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
    // 13: uploaded images. `data` holds the metadata; bytes never change after upload. See media.
    "CREATE TABLE IF NOT EXISTS media(id VARCHAR(255) COLLATE utf8mb4_bin PRIMARY KEY, data LONGTEXT NOT NULL, bytes LONGBLOB NOT NULL) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4",
];

pub(crate) fn migrate(db: &Pool) -> Result<()> {
    let mut conn = db.get_conn().map_err(db_error)?;
    for statement in [
        "CREATE TABLE IF NOT EXISTS cms_lock(id INT PRIMARY KEY) ENGINE=InnoDB",
        "INSERT IGNORE INTO cms_lock(id) VALUES(1)",
        "CREATE TABLE IF NOT EXISTS schema_migrations(version INT PRIMARY KEY) ENGINE=InnoDB",
    ] {
        conn.query_drop(statement).map_err(db_error)?;
    }
    // DDL commits implicitly, which would release a lock taken on the same
    // connection. Hold the shared lock row on `lock` and migrate on `conn`.
    let mut lock = db.get_conn().map_err(db_error)?;
    let mut tx = lock
        .start_transaction(TxOpts::default())
        .map_err(db_error)?;
    tx.query_drop("SELECT id FROM cms_lock WHERE id=1 FOR UPDATE")
        .map_err(db_error)?;
    let applied: usize = conn
        .query_first("SELECT COALESCE(MAX(version), 0) FROM schema_migrations")
        .map_err(db_error)?
        .unwrap();
    for (index, statement) in MIGRATIONS.iter().enumerate().skip(applied) {
        conn.query_drop(statement).map_err(db_error)?;
        conn.exec_drop(
            "INSERT INTO schema_migrations(version) VALUES(?)",
            (index + 1,),
        )
        .map_err(db_error)?;
    }
    tx.commit().map_err(db_error)
}

pub(crate) fn load_data(db: &mut impl Queryable, pages: bool) -> Result<StoredData> {
    Ok(StoredData {
        pages: if pages {
            load_all(db, "pages")?
        } else {
            Vec::new()
        },
        templates: load_all(db, "templates")?,
        components: load_all(db, "components")?,
    })
}

pub(crate) fn load_all<T: serde::de::DeserializeOwned>(
    db: &mut impl Queryable,
    table: &str,
) -> Result<Vec<T>> {
    let rows: Vec<String> = db
        .query(format!("SELECT data FROM {table} ORDER BY id"))
        .map_err(db_error)?;
    rows.into_iter().map(|v| parse(&v)).collect()
}

pub(crate) fn require_page(db: &mut impl Queryable, id: &PageId) -> Result<Page> {
    load_one(db, "pages", id.as_str())
}

pub(crate) fn require_template(db: &mut impl Queryable, id: &TemplateId) -> Result<Template> {
    load_one(db, "templates", id.as_str())
}

pub(crate) fn require_component(db: &mut impl Queryable, id: &ComponentId) -> Result<Component> {
    load_one(db, "components", id.as_str())
}

pub(crate) fn load_one<T: serde::de::DeserializeOwned>(
    db: &mut impl Queryable,
    table: &str,
    id: &str,
) -> Result<T> {
    let raw: Option<String> = db
        .exec_first(format!("SELECT data FROM {table} WHERE id=?"), (id,))
        .map_err(db_error)?;
    parse(&raw.ok_or_else(ApiError::not_found)?)
}

pub(crate) fn json<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value).map_err(|_| {
        ApiError(
            StatusCode::INTERNAL_SERVER_ERROR,
            "serialization failed".into(),
        )
    })
}

pub(crate) fn parse<T: serde::de::DeserializeOwned>(value: &str) -> Result<T> {
    serde_json::from_str(value).map_err(|_| {
        ApiError(
            StatusCode::INTERNAL_SERVER_ERROR,
            "stored data is invalid".into(),
        )
    })
}

pub(crate) fn db_error(error: mysql::Error) -> ApiError {
    // Server messages can contain submitted content. Log codes, never SQL or values.
    match error {
        mysql::Error::MySqlError(error) => eprintln!(
            "MySQL operation failed: code={}, state={}",
            error.code, error.state
        ),
        mysql::Error::IoError(error) => eprintln!(
            "MySQL I/O failed: kind={:?}, os_code={:?}",
            error.kind(),
            error.raw_os_error()
        ),
        _ => eprintln!("MySQL operation failed: driver or protocol error"),
    }
    ApiError(StatusCode::INTERNAL_SERVER_ERROR, "database error".into())
}

pub(crate) fn constraint_error(error: mysql::Error) -> ApiError {
    if matches!(&error, mysql::Error::MySqlError(error) if error.code == 1062) {
        ApiError::conflict("id or slug already exists")
    } else {
        db_error(error)
    }
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
