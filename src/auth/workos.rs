use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::http::StatusCode;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::{Client, Url};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{ApiError, Result};

pub struct WorkOs {
    api_key: String,
    client_id: String,
    pub(super) redirect_uri: Url,
    organization_id: String,
    client: Client,
    api_url: Url,
}

pub(super) struct Session {
    pub user_id: String,
    pub session_id: String,
    refresh_token: String,
    valid_until: Instant,
}

#[derive(Deserialize)]
struct Authentication {
    user: User,
    organization_id: Option<String>,
    access_token: String,
    refresh_token: String,
}

#[derive(Deserialize)]
struct User {
    id: String,
    email_verified: bool,
}

#[derive(Deserialize)]
struct Claims {
    sid: String,
    sub: String,
    org_id: String,
    exp: u64,
}

impl WorkOs {
    pub fn new(
        api_key: String,
        client_id: String,
        redirect_uri: String,
        organization_id: String,
    ) -> std::result::Result<Self, String> {
        if [&api_key, &client_id, &organization_id]
            .iter()
            .any(|v| v.trim().is_empty())
        {
            return Err("WorkOS API key, client ID and organization ID must not be empty".into());
        }
        let redirect_uri = Url::parse(&redirect_uri).map_err(|_| "invalid WORKOS_REDIRECT_URI")?;
        let local = matches!(
            redirect_uri.host_str(),
            Some("localhost" | "127.0.0.1" | "[::1]")
        );
        if !(redirect_uri.scheme() == "https" || redirect_uri.scheme() == "http" && local)
            || redirect_uri.host_str().is_none()
            || !redirect_uri.username().is_empty()
            || redirect_uri.password().is_some()
            || redirect_uri.query().is_some()
            || redirect_uri.fragment().is_some()
            || redirect_uri.path() != "/api/auth/callback"
        {
            return Err("WORKOS_REDIRECT_URI must be https://<host>/api/auth/callback (HTTP is allowed only on loopback)".into());
        }
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "could not initialize WorkOS HTTP client")?;
        Ok(Self {
            api_key,
            client_id,
            redirect_uri,
            organization_id,
            client,
            api_url: Url::parse("https://api.workos.com/").unwrap(),
        })
    }

    pub(super) fn authorization_url(&self, state: &str, challenge: &str) -> Url {
        let mut url = self.api_url.join("user_management/authorize").unwrap();
        url.query_pairs_mut().extend_pairs([
            ("client_id", self.client_id.as_str()),
            ("redirect_uri", self.redirect_uri.as_str()),
            ("response_type", "code"),
            ("provider", "authkit"),
            ("organization_id", &self.organization_id),
            ("state", state),
            ("code_challenge", challenge),
            ("code_challenge_method", "S256"),
            ("screen_hint", "sign-in"),
        ]);
        url
    }

    pub(super) fn logout_url(&self, session_id: &str) -> String {
        let mut url = self
            .api_url
            .join("user_management/sessions/logout")
            .unwrap();
        let return_to = self.redirect_uri.join("/admin").unwrap();
        url.query_pairs_mut().extend_pairs([
            ("session_id", session_id),
            ("return_to", return_to.as_str()),
        ]);
        url.into()
    }

    pub(super) async fn exchange(&self, code: &str, verifier: &str) -> Result<Session> {
        self.authenticate(
            json!({"grant_type": "authorization_code", "code": code, "code_verifier": verifier}),
        )
        .await
    }

    pub(super) async fn validate(&self, session: &mut Session) -> Result<()> {
        if session.valid_until > Instant::now() {
            return Ok(());
        }
        let refreshed = self
            .authenticate(json!({
                "grant_type": "refresh_token", "refresh_token": session.refresh_token,
                "organization_id": self.organization_id,
            }))
            .await?;
        if refreshed.user_id != session.user_id || refreshed.session_id != session.session_id {
            return Err(denied());
        }
        *session = refreshed;
        Ok(())
    }

    async fn authenticate(&self, mut body: Value) -> Result<Session> {
        body["client_id"] = json!(self.client_id);
        body["client_secret"] = json!(self.api_key);
        let response = self
            .client
            .post(self.api_url.join("user_management/authenticate").unwrap())
            .json(&body)
            .send()
            .await
            .map_err(|_| unavailable())?;
        if !response.status().is_success() {
            return Err(if response.status().is_client_error() {
                denied()
            } else {
                unavailable()
            });
        }
        let result: Authentication = response.json().await.map_err(|_| unavailable())?;
        self.authorized_session(result)
    }

    fn authorized_session(&self, result: Authentication) -> Result<Session> {
        if result.organization_id.as_deref() != Some(&self.organization_id)
            || !result.user.email_verified
            || result.user.id.is_empty()
            || result.refresh_token.is_empty()
        {
            return Err(denied());
        }
        // This token comes only from the authenticated TLS API response above, never
        // from a browser. Decode expiry/session metadata; do not accept bearer JWTs.
        let payload = result.access_token.split('.').nth(1).ok_or_else(denied)?;
        let claims: Claims =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload).map_err(|_| denied())?)
                .map_err(|_| denied())?;
        if claims.sub != result.user.id
            || claims.org_id != self.organization_id
            || claims.sid.is_empty()
        {
            return Err(denied());
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| denied())?
            .as_secs();
        let remaining = claims
            .exp
            .checked_sub(now)
            .filter(|v| *v > 0)
            .ok_or_else(denied)?;
        Ok(Session {
            user_id: result.user.id,
            session_id: claims.sid,
            refresh_token: result.refresh_token,
            valid_until: Instant::now() + Duration::from_secs(remaining.min(300)),
        })
    }
}

fn denied() -> ApiError {
    ApiError(StatusCode::UNAUTHORIZED, "WorkOS access denied".into())
}
fn unavailable() -> ApiError {
    ApiError(
        StatusCode::SERVICE_UNAVAILABLE,
        "WorkOS is unavailable".into(),
    )
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
