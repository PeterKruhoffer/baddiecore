use std::{env, net::SocketAddr};

use baddiecore::{router, AppState};

#[tokio::main]
async fn main() {
    let password = env::var("BADDIE_ADMIN_PASSWORD").unwrap_or_else(|_| {
        eprintln!("BADDIE_ADMIN_PASSWORD is required"); std::process::exit(2)
    });
    let bind = env::var("BADDIE_BIND").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let address: SocketAddr = bind.parse().unwrap_or_else(|_| { eprintln!("BADDIE_BIND is invalid"); std::process::exit(2) });
    let db = env::var("BADDIE_DB").unwrap_or_else(|_| "data/baddiecore.db".into());
    let static_dir = env::var("BADDIE_STATIC").unwrap_or_else(|_| "web/dist".into());
    let secure_cookie = env::var("BADDIE_SECURE_COOKIE").is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
    let state = AppState::open(db, password, secure_cookie).unwrap_or_else(|e| { eprintln!("startup failed: {e}"); std::process::exit(2) });
    let listener = tokio::net::TcpListener::bind(address).await.unwrap_or_else(|e| { eprintln!("bind failed: {e}"); std::process::exit(2) });
    println!("listening on {address}");
    axum::serve(listener, router(state, static_dir)).with_graceful_shutdown(async { let _ = tokio::signal::ctrl_c().await; }).await.unwrap();
}
