//! The main entry point for the sw-catcher application.
use log::{error, info};
use std::error::Error;
use std::process;
use sw_catcher::{
    config::{create_default_config_file, load_config, print_usage_guide},
    logging::{log_startup_info, setup_logging},
    watcher::start_watcher,
    AUTHOR, VERSION,
};

fn main() {
    // Display startup banner
    println!("sw-catcher v{} - by {}", VERSION, AUTHOR);
    println!("Watching for LLM results in meta.json files");

    // Try to create default config file (will be ignored if already exists)
    if let Err(e) = create_default_config_file("config.toml") {
        eprintln!("Warning: Failed to create default config file: {}", e);
        // Continue execution, as this is not a critical error
    }

    // Run the main application
    if let Err(e) = run() {
        error!("Application error: {}", e);
        eprintln!("Error: {}", e);
        print_usage_guide();
        process::exit(1);
    }
}

/// Run the application
fn run() -> Result<(), Box<dyn Error>> {
    // Load configuration
    let app_state = load_config()?;

    // Set up logging
    setup_logging(&app_state)?;

    // Log startup information
    log_startup_info(&app_state);

    // Display startup message
    info!("sw-catcher started successfully");

    // Start the file watcher (this blocks until application termination)
    start_watcher(app_state)?;

    // This point is reached only on clean shutdown
    info!("sw-catcher shutting down");

    Ok(())
}