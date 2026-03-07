//! Main application entry point
//!
//! This file just forwards to the main application logic in main_app.rs

// Include the main application logic
#[path = "app.rs"]
mod app;

fn main() {
    if let Err(e) = app::main() {
        eprintln!("Application error: {}", e);
        std::process::exit(1);
    }
}
