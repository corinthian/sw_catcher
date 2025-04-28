//! # sw-catcher
//!
//! Monitors a directory for meta.json files and copies LLM results to clipboard.
//! Supports keyphrase detection and various actions.
//!
//! ## Features
//!
//! - Monitors directories for new meta.json files
//! - Extracts LLM results from various json field formats
//! - Copies results to clipboard in different formats (plaintext, richtext, markdown)
//! - Detects keyphrases and executes associated actions
//! - Supports chained actions through natural dictation
//! - Configurable text cleaning options
//! - Supports dry-run mode for testing actions
//!
//! ## Example
//!
//! ```no_run
//! use sw_catcher::{
//!     config::{load_config, Opts},
//!     logging::setup_logging,
//!     watcher::start_watcher,
//! };
//!
//! fn main() {
//!     let app_state = load_config().expect("Failed to load configuration");
//!     setup_logging(&app_state).expect("Failed to set up logging");
//!     start_watcher(app_state).expect("Failed to start file watcher");
//! }
//! ```

// Define all modules
pub mod actions;
pub mod clipboard;
pub mod config;
pub mod keyphrase;
pub mod logging;
pub mod meta_processor;
pub mod text_processing;
pub mod watcher;

// Define the Meta type here to avoid circular dependencies
mod meta {
    use serde::Deserialize;

    /// Flexible Meta structure that looks for different possible keys
    #[derive(Debug, Deserialize)]
    pub struct Meta {
        #[serde(rename = "llmResult", default)]
        pub llm_result: Option<String>,
        #[serde(default)]
        pub result: Option<String>,
        #[serde(rename = "rawResult", default)]
        pub raw_result: Option<String>,
    }
}

// Re-export key types and functions
pub use actions::{
    execute_action, ActionType
};
pub use clipboard::{copy_to_clipboard_with_format, ensure_clipboard_content_with_monitoring, ClipboardFormat};
pub use config::{load_config, create_default_config_file, AppConfig, AppState, Opts};
pub use keyphrase::{
    detect_all_keyphrases, process_keyphrases, process_keyphrases_enhanced,
    KeyphraseAction, KeyphraseProcessingOptions, KeyphraseMatch, TextSegment,
};
pub use logging::setup_logging;
pub use meta::Meta;
pub use meta_processor::{process_meta_file, LastProcessedMap};
pub use text_processing::apply_text_cleaning;
pub use watcher::start_watcher;

// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const AUTHOR: &str = env!("CARGO_PKG_AUTHORS");

/// Extract text from meta.json based on user preference
pub fn extract_text_by_preference(meta: &Meta, preference: &str) -> Option<String> {
    match preference.to_lowercase().as_str() {
        "llm" => meta.llm_result.clone(),
        "raw" => meta.raw_result.clone(),
        "intermediate" => meta.result.clone(),
        _ => {
            // "auto" or any other value - try each field in order, with cloning to avoid ownership issues
            meta.llm_result.clone()
                .or_else(|| meta.result.clone())
                .or_else(|| meta.raw_result.clone())
        }
    }
}

/// Run the application
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let app_state = load_config()?;

    // Setup logging
    setup_logging(&app_state)?;

    // Start watching for files
    start_watcher(app_state)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_text_by_preference() {
        // Create a test Meta instance with all fields populated
        let meta = Meta {
            llm_result: Some("This is the LLM result".to_string()),
            result: Some("This is the intermediate result".to_string()),
            raw_result: Some("This is the raw result".to_string()),
        };

        // Test each preference
        assert_eq!(extract_text_by_preference(&meta, "llm"), Some("This is the LLM result".to_string()));
        assert_eq!(extract_text_by_preference(&meta, "raw"), Some("This is the raw result".to_string()));
        assert_eq!(extract_text_by_preference(&meta, "intermediate"), Some("This is the intermediate result".to_string()));
        assert_eq!(extract_text_by_preference(&meta, "auto"), Some("This is the LLM result".to_string()));
        assert_eq!(extract_text_by_preference(&meta, "invalid"), Some("This is the LLM result".to_string()));

        // Test with some fields missing
        let meta_partial = Meta {
            llm_result: None,
            result: Some("This is the intermediate result".to_string()),
            raw_result: Some("This is the raw result".to_string()),
        };

        assert_eq!(extract_text_by_preference(&meta_partial, "llm"), None);
        assert_eq!(extract_text_by_preference(&meta_partial, "auto"), Some("This is the intermediate result".to_string()));

        let meta_minimal = Meta {
            llm_result: None,
            result: None,
            raw_result: Some("This is the raw result".to_string()),
        };

        assert_eq!(extract_text_by_preference(&meta_minimal, "auto"), Some("This is the raw result".to_string()));
    }
}