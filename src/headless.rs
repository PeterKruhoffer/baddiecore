//! Server-to-server published content and external component registration.

use axum::{
    Json, Router,
    extract::{Path, Request, State},
    http::{Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, put},
};
use mysql::prelude::Queryable;
use serde::Serialize;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::{
    ApiError, AppState, Component, Content, Renderer, Result, content, db_error, json, parse,
    validate_component, validate_schema_change,
};

#[derive(Clone, Default)]
pub struct ApiKeys {
    content: Option<[u8; 32]>,
    components: Option<[u8; 32]>,
}

impl ApiKeys {
    /// Each configured key must contain at least 32 non-whitespace ASCII bytes.
    /// Use independent random secrets. Empty values disable that capability.
    pub fn new(
        content: Option<String>,
        components: Option<String>,
    ) -> std::result::Result<Self, String> {
        fn hash(key: Option<String>) -> std::result::Result<Option<[u8; 32]>, String> {
            match key.filter(|key| !key.is_empty()) {
                None => Ok(None),
                Some(key) if key.len() >= 32 && key.bytes().all(|b| b.is_ascii_graphic()) => {
                    Ok(Some(Sha256::digest(key.as_bytes()).into()))
                }
                Some(_) => Err(
                    "headless API keys must contain at least 32 non-whitespace ASCII bytes".into(),
                ),
            }
        }
        let keys = Self {
            content: hash(content)?,
            components: hash(components)?,
        };
        if keys.content.is_some() && keys.content == keys.components {
            return Err("headless content and component keys must be different".into());
        }
        Ok(keys)
    }
}

impl AppState {
    pub fn with_headless_keys(mut self, keys: ApiKeys) -> Self {
        self.headless = keys;
        self
    }
}

pub(crate) fn routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/content", get(content))
        .route("/pages", get(pages))
        .route("/components/{id}", put(register_component))
        .route_layer(middleware::from_fn_with_state(state, require_key))
}

async fn require_key(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let expected = if matches!(*request.method(), Method::GET | Method::HEAD) {
        state.headless.content
    } else {
        state.headless.components
    };
    let headers = request.headers();
    let supplied = headers
        .get(header::AUTHORIZATION)
        .filter(|_| headers.get_all(header::AUTHORIZATION).iter().count() == 1)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    let authorized = expected.zip(supplied).is_some_and(|(expected, supplied)| {
        let supplied: [u8; 32] = Sha256::digest(supplied.as_bytes()).into();
        bool::from(expected.ct_eq(&supplied))
    });
    let mut response = if authorized {
        next.run(request).await
    } else {
        let mut response =
            ApiError(StatusCode::UNAUTHORIZED, "API key required".into()).into_response();
        response
            .headers_mut()
            .insert(header::WWW_AUTHENTICATE, "Bearer".parse().unwrap());
        response
    };
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response
}

#[derive(Serialize)]
struct PublishedPage {
    id: String,
    title: String,
    slug: String,
    template_id: String,
    revision: i64,
}

async fn pages(State(state): State<AppState>) -> Result<Json<Vec<PublishedPage>>> {
    state
        .read(|db| {
            let snapshots: Vec<String> = db
                .query("SELECT data FROM snapshots ORDER BY slug")
                .map_err(db_error)?;
            let pages = snapshots
                .into_iter()
                .map(|raw| {
                    let snapshot: Content = parse(&raw)?;
                    let page = snapshot.page;
                    Ok(PublishedPage {
                        id: page.id,
                        title: page.title,
                        slug: page.slug,
                        template_id: page.template_id,
                        revision: page.revision,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(Json(pages))
        })
        .await
}

async fn register_component(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(item): Json<Component>,
) -> Result<(StatusCode, Json<Component>)> {
    if id != item.id
        || id.is_empty()
        || id.len() > 200
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return Err(ApiError::bad(
            "component id must match the path and use 1–200 letters, numbers, underscores or hyphens",
        ));
    }
    if item.renderer != Renderer::External {
        return Err(ApiError::bad(
            "headless components must use the external renderer",
        ));
    }
    validate_component(&item)?;
    state
        .run(move |db| {
            let existing: Option<String> = db
                .exec_first("SELECT data FROM components WHERE id=?", (&id,))
                .map_err(db_error)?;
            let status = if let Some(raw) = existing {
                let previous: Component = parse(&raw)?;
                if previous.renderer != Renderer::External {
                    return Err(ApiError::conflict(
                        "cannot replace a CMS-rendered component",
                    ));
                }
                validate_schema_change(db, None, Some(&item))?;
                db.exec_drop(
                    "UPDATE components SET data=? WHERE id=?",
                    (json(&item)?, &id),
                )
                .map_err(db_error)?;
                StatusCode::OK
            } else {
                db.exec_drop("INSERT INTO components VALUES(?,?)", (&id, json(&item)?))
                    .map_err(db_error)?;
                StatusCode::CREATED
            };
            Ok((status, Json(item)))
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_optional_but_nonempty_values_must_be_strong_and_separate() {
        assert!(ApiKeys::new(None, Some(String::new())).is_ok());
        assert!(ApiKeys::new(Some("a".repeat(31)), None).is_err());
        assert!(ApiKeys::new(Some("a".repeat(32)), None).is_ok());
        assert!(ApiKeys::new(None, Some(format!("{} ", "a".repeat(32)))).is_err());
        assert!(ApiKeys::new(Some("a".repeat(32)), Some("a".repeat(32))).is_err());
    }
}
