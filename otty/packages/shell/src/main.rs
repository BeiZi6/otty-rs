//! `otty-app` — the window.
//!
//! Everything of substance is in `otty_shell::run`; this binary exists so the CLI
//! can find an app to launch beside itself.

fn main() {
    if let Err(e) = otty_shell::run(otty_config::Config::load()) {
        eprintln!("otty: {e}");
        std::process::exit(3);
    }
}

