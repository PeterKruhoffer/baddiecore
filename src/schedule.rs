//! Scheduled publication. Scheduling stores a validated snapshot, exactly as
//! publishing would build it, and releases that snapshot at `publish_at`.
//! Later draft or schema edits therefore never go live unreviewed.
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Schedule {
    pub page_id: String,
    /// The page revision that goes live.
    pub revision: i64,
    /// Unix time in seconds.
    pub publish_at: i64,
    pub scheduled_by: String,
    /// Why publication failed at `publish_at`. A failed schedule waits until it
    /// is replaced or cancelled.
    pub error: Option<String>,
}

pub(crate) fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

const TEN_YEARS: i64 = 10 * 366 * 24 * 60 * 60;

pub(crate) fn validate_time(publish_at: i64) -> Result<()> {
    let now = now();
    if publish_at <= now || publish_at > now + TEN_YEARS {
        return Err(ApiError::bad(
            "publish time must be in the future and within ten years",
        ));
    }
    Ok(())
}

/// Replaces the page's pending schedule. Callers build and validate the snapshot
/// within this same locked transaction.
pub(crate) fn store(
    db: &mut Locked<'_>,
    snapshot: &Content,
    publish_at: i64,
    scheduled_by: &str,
) -> Result<Schedule> {
    check_published_paths(db, &snapshot.page)?;
    let schedule = Schedule {
        page_id: snapshot.page.id.clone(),
        revision: snapshot.page.revision,
        publish_at,
        scheduled_by: scheduled_by.into(),
        error: None,
    };
    db.exec_drop(
        "REPLACE INTO scheduled_publications(page_id,publish_at,revision,scheduled_by,error,data) VALUES(?,?,?,?,NULL,?)",
        (
            &schedule.page_id,
            schedule.publish_at,
            schedule.revision,
            &schedule.scheduled_by,
            json(snapshot)?,
        ),
    )
    .map_err(db_error)?;
    Ok(schedule)
}

pub(crate) fn load(db: &mut impl Queryable) -> Result<Vec<Schedule>> {
    db.query_map(
        "SELECT page_id,revision,publish_at,scheduled_by,error FROM scheduled_publications ORDER BY publish_at",
        |(page_id, revision, publish_at, scheduled_by, error)| Schedule {
            page_id,
            revision,
            publish_at,
            scheduled_by,
            error,
        },
    )
    .map_err(db_error)
}

#[derive(Deserialize)]
pub(super) struct NewSchedule {
    revision: i64,
    publish_at: i64,
}

pub(super) async fn schedule_page(
    State(state): State<AppState>,
    Extension(editor): Extension<auth::Editor>,
    AxumPath(id): AxumPath<String>,
    Json(input): Json<NewSchedule>,
) -> Result<Json<Schedule>> {
    state
        .run_as(editor, move |db, access| {
            access.admin()?;
            let page = require_page(db, &id)?;
            if page.revision != input.revision {
                return Err(ApiError::conflict("stale revision"));
            }
            validate_time(input.publish_at)?;
            let snapshot = workflow::current_content(db, page)?;
            Ok(Json(store(db, &snapshot, input.publish_at, &access.id)?))
        })
        .await
}

pub(super) async fn cancel(
    State(state): State<AppState>,
    Extension(editor): Extension<auth::Editor>,
    AxumPath(id): AxumPath<String>,
) -> Result<StatusCode> {
    state
        .run_as(editor, move |db, access| {
            access.reviewer()?;
            db.exec_drop("DELETE FROM scheduled_publications WHERE page_id=?", (id,))
                .map_err(db_error)?;
            if db.affected_rows() == 0 {
                Err(ApiError::not_found())
            } else {
                Ok(StatusCode::NO_CONTENT)
            }
        })
        .await
}

/// Publishes the page's scheduled snapshot if it is still due and has not failed.
fn publish_if_due(db: &mut Locked<'_>, page_id: &str, now: i64) -> Result<bool> {
    let data: Option<String> = db
        .exec_first(
            "SELECT data FROM scheduled_publications WHERE page_id=? AND publish_at<=? AND error IS NULL",
            (page_id, now),
        )
        .map_err(db_error)?;
    match data {
        Some(data) => publish(db, parse(&data)?).map(|_| true),
        None => Ok(false),
    }
}

impl AppState {
    /// Publishes every scheduled page whose time has come and returns how many
    /// went live. A schedule that cannot publish, for example because another
    /// page now owns its path, records the error and stops retrying. Database
    /// failures return an error and leave the schedule for the next run.
    pub async fn publish_due(&self) -> std::result::Result<usize, String> {
        let now = now();
        let due: Vec<String> = self
            .db
            .unlocked(move |db| {
                db.exec(
                    "SELECT page_id FROM scheduled_publications WHERE publish_at<=? AND error IS NULL ORDER BY publish_at",
                    (now,),
                )
                .map_err(db_error)
            })
            .await
            .map_err(|e| e.1)?;
        let mut published = 0;
        for id in due {
            let page_id = id.clone();
            match self
                .db
                .run(move |db| publish_if_due(db, &page_id, now))
                .await
            {
                Ok(done) => published += usize::from(done),
                Err(ApiError(status, message)) if status.is_client_error() => {
                    self.db.run(move |db| {
                        db.exec_drop(
                            "UPDATE scheduled_publications SET error=? WHERE page_id=? AND publish_at<=? AND error IS NULL",
                            (message, id, now),
                        )
                        .map_err(db_error)
                    })
                    .await
                    .map_err(|e| e.1)?;
                }
                Err(error) => return Err(error.1),
            }
        }
        Ok(published)
    }

    /// Calls `publish_due` every 15 seconds. Servers spawn this once alongside
    /// the router; several instances may run it against one database.
    pub async fn publish_scheduled_pages(self) {
        loop {
            if let Err(error) = self.publish_due().await {
                eprintln!("scheduled publishing failed: {error}");
            }
            tokio::time::sleep(Duration::from_secs(15)).await;
        }
    }
}
