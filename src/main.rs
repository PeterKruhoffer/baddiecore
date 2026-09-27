use std::{env, net::SocketAddr, sync::Arc};

use baddiecore::{AppState, auth::Auth, router};

#[tokio::main]
async fn main() {
    let bind = bind_address(env::var("BADDIE_BIND").ok(), env::var("PORT").ok());
    let address: SocketAddr = bind.parse().unwrap_or_else(|_| {
        eprintln!("BADDIE_BIND is invalid");
        std::process::exit(2)
    });
    let db = env::var("DATABASE_URL").unwrap_or_else(|_| {
        eprintln!("DATABASE_URL is required and must be a MySQL URL");
        std::process::exit(2)
    });
    let static_dir = env::var("BADDIE_STATIC").unwrap_or_else(|_| "web/dist".into());
    let secure_cookie =
        env::var("BADDIE_SECURE_COOKIE").is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
    let auth = Auth::from_env(secure_cookie).unwrap_or_else(|e| {
        eprintln!("authentication configuration failed: {e}");
        std::process::exit(2)
    });
    let state = AppState::open_with_auth(&db, Arc::new(auth)).unwrap_or_else(|e| {
        eprintln!("startup failed: {e}");
        std::process::exit(2)
    });
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .unwrap_or_else(|e| {
            eprintln!("bind failed: {e}");
            std::process::exit(2)
        });
    println!("listening on {address}");
    axum::serve(listener, router(state, static_dir))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

fn bind_address(bind: Option<String>, port: Option<String>) -> String {
    bind.unwrap_or_else(|| match port {
        Some(port) => format!("0.0.0.0:{port}"),
        None => "127.0.0.1:3000".into(),
    })
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("failed to install SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = terminate.recv() => {},
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn railway_port_and_explicit_bind() {
        assert_eq!(bind_address(None, None), "127.0.0.1:3000");
        assert_eq!(bind_address(None, Some("8123".into())), "0.0.0.0:8123");
        assert_eq!(
            bind_address(Some("127.0.0.1:4567".into()), Some("8123".into())),
            "127.0.0.1:4567"
        );
    }
}
