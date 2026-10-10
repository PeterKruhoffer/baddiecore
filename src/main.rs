use std::{
    env,
    io::{BufRead, IsTerminal},
    net::SocketAddr,
};

use baddiecore::{AppState, headless::ApiKeys, router, serialization};
use mysql::{Opts, Pool};

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args
        .first()
        .is_some_and(|a| a == "pull" || a == "push" || a == "status")
    {
        serialization_cli(&args);
        return;
    }
    if args.first().is_some_and(|a| a == "reset-admin") {
        reset_admin_cli(&args[1..]);
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
    let headless = ApiKeys::new(
        env::var("BADDIE_CONTENT_API_KEY").ok(),
        env::var("BADDIE_COMPONENT_API_KEY").ok(),
    )
    .unwrap_or_else(|e| {
        eprintln!("headless configuration failed: {e}");
        std::process::exit(2)
    });
    let admin_password = env::var("BADDIE_ADMIN_PASSWORD").ok();
    let state = AppState::open(&db, admin_password, secure_cookie)
        .unwrap_or_else(|e| {
            eprintln!("startup failed: {e}");
            std::process::exit(2)
        })
        .with_headless_keys(headless);
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
    let mut force = false;
    let mut dry_run = false;
    for arg in &args[1..] {
        match arg.as_str() {
            "--force" if command != "status" => force = true,
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
    if command == "status" {
        match serialization::status(&pool, directory) {
            Ok(changes) => print_status(&changes),
            Err(e) => cli_fail(&format!("status failed: {e}")),
        }
        return;
    }
    let result = if command == "pull" {
        serialization::pull(&pool, directory, force)
    } else {
        serialization::push(&pool, directory, force, dry_run)
    };
    let sync = result.unwrap_or_else(|e| cli_fail(&format!("{command} failed: {e}")));
    let target = if command == "pull" {
        "the files"
    } else {
        "the database"
    };
    match sync.applied.len() {
        0 => println!("{command}: nothing to change in {target}"),
        n => println!(
            "{command}: {n} change{} {} {target}",
            if n == 1 { "" } else { "s" },
            if dry_run {
                "would be applied to"
            } else {
                "applied to"
            }
        ),
    }
    print_changes(&sync.applied);
    if !sync.pending.is_empty() {
        let (other, opposite) = if command == "pull" {
            ("the database", "push")
        } else {
            ("the files", "pull")
        };
        println!(
            "{} change{} only in {other}; run {opposite} to copy {}:",
            sync.pending.len(),
            if sync.pending.len() == 1 { "" } else { "s" },
            if sync.pending.len() == 1 {
                "it"
            } else {
                "them"
            },
        );
        print_changes(&sync.pending);
    }
}

fn reset_admin_cli(args: &[String]) {
    let [username] = args else {
        cli_fail("usage: baddiecore reset-admin <username>; reads the password from stdin");
    };
    let url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| cli_fail("DATABASE_URL is required and must be a MySQL URL"));
    let password = if std::io::stdin().is_terminal() {
        rpassword::prompt_password(format!("New password for {username}: "))
            .unwrap_or_else(|_| cli_fail("could not read the password"))
    } else {
        let mut line = String::new();
        std::io::stdin()
            .lock()
            .read_line(&mut line)
            .unwrap_or_else(|_| cli_fail("could not read the password"));
        line.trim_end_matches(['\r', '\n']).to_owned()
    };
    match baddiecore::reset_admin(&url, username, &password) {
        Ok(()) => {
            println!("{username} is an administrator with the new password; their sessions ended.")
        }
        Err(e) => cli_fail(&format!("reset-admin failed: {e}")),
    }
}

fn print_changes(changes: &[serialization::Change]) {
    for change in changes {
        println!("  {:<9} {}", change.action, change.path);
    }
}

fn print_status(changes: &[serialization::Change]) {
    use serialization::Side;
    if changes.is_empty() {
        println!("The database and the files are in sync.");
    }
    for (side, heading) in [
        (
            Side::Database,
            "Changed in the database; pull to copy them to the files:",
        ),
        (
            Side::Files,
            "Changed in the files; push to copy them to the database:",
        ),
        (
            Side::Both,
            "Changed in both; choose with pull --force (keep database) or push --force (keep files):",
        ),
    ] {
        let group: Vec<_> = changes.iter().filter(|c| c.side == side).cloned().collect();
        if !group.is_empty() {
            println!("{heading}");
            print_changes(&group);
        }
    }
}

fn print_serialization_help() {
    println!(
        "baddiecore status [directory]
  Show which components and templates changed in the database and in the files since the
  last pull or push.

baddiecore pull [directory] [--force]
  Copy database changes, including deletions, into the files. --force makes the files
  mirror the database, discarding file changes that were not pushed.

baddiecore push [directory] [--force] [--dry-run]
  Copy file changes, including deletions, into the database in one transaction. --force
  makes the database mirror the files, discarding database changes that were not pulled.
  --dry-run validates and rolls back.

baddiecore reset-admin <username>
  Make <username> an administrator, adding the member if needed, and set its password.
  Prompts for the password, or reads one line from stdin when it is not a terminal.
  Use it to create the first account or to recover access.

Both stop when an item changed on both sides, until you choose a side with --force.
Content pages are not synced; move them with packages in the admin UI. Directory defaults
to baddiecore-content. DATABASE_URL must identify a directly accessible MySQL database."
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
