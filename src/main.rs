use std::{env, net::SocketAddr, sync::Arc};

use baddiecore::{AppState, auth::Auth, router, serialization};
use mysql::{Opts, Pool};

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "pull" || a == "push") {
        serialization_cli(&args);
        return;
    }
    if args.first().is_some_and(|a| a == "--help" || a == "-h") {
        print_serialization_help();
        return;
    }
    if !args.is_empty() {
        cli_fail("unknown command; use --help, or no arguments to start the server");
    }
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

fn serialization_cli(args: &[String]) {
    let command = &args[0];
    let mut directory = None;
    let mut pages = false;
    let mut force = false;
    let mut dry_run = false;
    for arg in &args[1..] {
        match arg.as_str() {
            "--pages" => pages = true,
            "--force" if command == "pull" => force = true,
            "--dry-run" if command == "push" => dry_run = true,
            "-h" | "--help" => {
                print_serialization_help();
                return;
            }
            value if !value.starts_with('-') && directory.is_none() => directory = Some(value),
            _ => cli_fail("invalid arguments; use --help"),
        }
    }
    let directory = std::path::Path::new(directory.unwrap_or("baddiecore-content"));
    let url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| cli_fail("DATABASE_URL is required and must be a MySQL URL"));
    let opts =
        Opts::from_url(&url).unwrap_or_else(|_| cli_fail("DATABASE_URL must be a MySQL URL"));
    let pool = Pool::new(opts).unwrap_or_else(|_| cli_fail("could not connect to MySQL"));
    let result = if command == "pull" {
        serialization::pull(&pool, directory, pages, force)
    } else {
        serialization::push(&pool, directory, pages, dry_run)
    };
    match result {
        Ok(c) => println!(
            "{} components, {} templates, {} pages{}",
            c.components,
            c.templates,
            c.pages,
            if dry_run {
                " (dry run; no changes committed)"
            } else {
                ""
            }
        ),
        Err(e) => cli_fail(&format!("{command} failed: {e}")),
    }
}

fn print_serialization_help() {
    println!(
        "baddiecore pull [directory] [--pages] [--force]\n  Export components and templates. Pages are opt-in. Refuses changed files unless --force.\n\nbaddiecore push [directory] [--pages] [--dry-run]\n  Transactionally merge files without deleting or publishing. --dry-run validates and rolls back.\n\nDirectory defaults to baddiecore-content. DATABASE_URL must identify a directly accessible MySQL database."
    );
}

fn cli_fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(2)
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
