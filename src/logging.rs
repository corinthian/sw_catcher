use crate::config::{parse_log_level, AppState, get_default_log_directory};
use log::{debug, LevelFilter, warn};
use notify::Error as NotifyError;
use notify::Result as NotifyResult;
use simplelog::{CombinedLogger, ConfigBuilder, SharedLogger, TermLogger, WriteLogger};
use std::fs::{File, create_dir_all};
use std::path::Path;

/// Set up logging with file and optionally terminal output
pub fn setup_logging(app_state: &AppState) -> NotifyResult<()> {
    // Check if logging is completely disabled
    if app_state.disable_logs {
        debug!("Logging has been disabled completely");
        return setup_null_logging();
    }

    // First determine the log file path as a String
    let log_file_path = if let Some(path) = &app_state.config.log_file {
        // Use config path if specified
        path.to_string()
    } else {
        // Use platform-specific default path
        let default_dir = get_default_log_directory();
        let default_path = default_dir.join("sw-catcher.log");
        default_path.to_string_lossy().to_string()
    };

    // Handle special case for /dev/null
    if log_file_path == "/dev/null" {
        return setup_null_logging();
    }

    let log_level = parse_log_level(app_state.config.log_level.as_deref().unwrap_or("info"));
    let echo_to_stdout = app_state.config.echo_to_stdout.unwrap_or(false);

    // Ensure the directory exists if needed
    if let Some(parent) = Path::new(&log_file_path).parent() {
        if !parent.exists() {
            if let Err(e) = create_dir_all(parent) {
                warn!("Could not create log directory {}: {}", parent.display(), e);
                // Continue anyway, we'll get an error when trying to create the log file
            }
        }
    }

    setup_logging_with_params(&log_file_path, log_level, echo_to_stdout)
}

/// Set up null logging (discard all file logs)
fn setup_null_logging() -> NotifyResult<()> {
    let log_config = ConfigBuilder::new()
        .add_filter_allow_str("sw_catcher")
        .set_time_format_rfc3339()
        .set_target_level(LevelFilter::Error)
        .build();

    let mut loggers: Vec<Box<dyn SharedLogger>> = Vec::new();

    // Only add terminal logger if needed for development
    #[cfg(debug_assertions)]
    {
        loggers.push(TermLogger::new(
            LevelFilter::Debug,
            log_config,
            simplelog::TerminalMode::Mixed,
            simplelog::ColorChoice::Auto,
        ));
    }

    if loggers.is_empty() {
        // If no loggers (production), just use a null logger (logs nowhere)
        return Ok(());
    }

    CombinedLogger::init(loggers)
        .map_err(|e| NotifyError::generic(&format!("Failed to initialize logger: {}", e)))?;

    debug!("Null logging initialized (file logging disabled)");
    
    Ok(())
}

/// Set up logging with specific parameters
pub fn setup_logging_with_params(
    log_file: &str,
    level: LevelFilter,
    echo_to_stdout: bool,
) -> NotifyResult<()> {
    let log_config = ConfigBuilder::new()
        .add_filter_allow_str("sw_catcher")
        .set_time_format_rfc3339()
        .set_target_level(LevelFilter::Error) // Don't show target in logs
        .build();

    let mut loggers: Vec<Box<dyn SharedLogger>> = Vec::new();

    // Add file logger
    let file = File::create(log_file).map_err(|e| {
        NotifyError::generic(&format!("Failed to create log file {}: {}", log_file, e))
    })?;

    loggers.push(WriteLogger::new(level, log_config.clone(), file));
    debug!("File logging enabled to {}", log_file);

    // Add terminal logger if requested
    if echo_to_stdout {
        loggers.push(TermLogger::new(
            level,
            log_config,
            simplelog::TerminalMode::Mixed,
            simplelog::ColorChoice::Auto,
        ));
        debug!("Terminal logging enabled");
    }

    CombinedLogger::init(loggers)
        .map_err(|e| NotifyError::generic(&format!("Failed to initialize logger: {}", e)))?;

    debug!("Logging initialized at level {}", level);

    Ok(())
}

/// Log application startup information
pub fn log_startup_info(app_state: &AppState) {
    debug!("sw-catcher starting up");

    if app_state.dry_run {
        debug!("Running in DRY-RUN mode - actions will be logged but not executed");
    }

    if app_state.disable_notifications {
        debug!("Desktop notifications are disabled");
    }
    
    if app_state.disable_logs {
        debug!("Logging to file is disabled");
    }

    debug!("Using clipboard format: {:?}", app_state.clipboard_format);

    if let Some(watch_dir) = &app_state.config.watch_dir {
        debug!("Watching for meta.json files in: {}", watch_dir);
    }
}

/// Log a separator line for visual grouping in logs
pub fn log_separator() {
    debug!("----------------------------------------");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn test_setup_logging_with_params() {
        // Create a temporary directory for the log file
        let temp_dir = tempdir().unwrap();
        let log_path = temp_dir.path().join("test.log");
        let log_file = log_path.to_str().unwrap();

        // Test setup with only file logging
        let result = setup_logging_with_params(log_file, LevelFilter::Info, false);
        assert!(result.is_ok());

        // Verify log file was created
        assert!(log_path.exists());
    }
    
    #[test]
    fn test_setup_null_logging() {
        // This shouldn't create any files
        let result = setup_null_logging();
        assert!(result.is_ok());
    }
}
