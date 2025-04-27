use crate::clipboard::parse_clipboard_format;
use crate::clipboard::ClipboardFormat;
use crate::keyphrase::{KeyphraseMatchingStrategy, PunctuationHandling};
use clap::Parser;
use log::{debug, error, LevelFilter};
use notify::Error as NotifyError;
use notify::Result as NotifyResult;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::fs;
use std::io::Write;
use std::path::Path;

/// Text cleaning options
#[derive(Debug, Clone, Deserialize)]
pub struct TextCleaningOptions {
    #[serde(default)]
    pub trim_whitespace: bool,
    #[serde(default)]
    pub normalize_newlines: bool,
    #[serde(default)]
    pub remove_extra_spaces: bool,
    #[serde(default)]
    pub capitalize_sentences: bool,
}

/// Keyphrase configuration options
#[derive(Debug, Clone, Deserialize)]
pub struct KeyphraseConfig {
    #[serde(default)]
    pub matching_strategy: Option<String>,  // "simple", "wholeword", or "exact"
    #[serde(default)]
    pub punctuation_handling: Option<String>,  // "ignore", "sentence", or "all"
}

/// Configuration structure for the application
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub watch_dir: Option<String>,
    pub log_file: Option<String>,
    pub log_level: Option<String>,
    pub echo_to_stdout: Option<bool>,
    pub detect_keyphrases: Option<bool>,
    pub keyphrases: Option<HashMap<String, String>>,
    #[serde(default)]
    pub disable_notifications: Option<bool>,
    pub dry_run: Option<bool>,
    pub disable_logs: Option<bool>,
    pub disable_clipboard: Option<bool>,
    pub clipboard_format: Option<String>,
    pub result_field_preference: Option<String>, // "llm", "raw", "intermediate", or "auto"
    pub text_cleaning: Option<TextCleaningOptions>,
    pub keyphrase_settings: Option<KeyphraseConfig>,
}

/// sw-catcher: Monitors a directory for meta.json files and copies LLM results to clipboard
#[derive(Parser)]
#[command(name = "sw-catcher", about, long_about = None, version)]
pub struct Opts {
    /// Directory to watch for new meta.json files
    #[arg(short = 'w', long, value_name = "DIRECTORY")]
    pub watch_dir: Option<PathBuf>,

    /// Log file path
    #[arg(short = 'f', long, value_name = "FILE")]
    pub log_file: Option<PathBuf>,

    /// Log level (error, warn, info, debug, trace)
    #[arg(short = 'l', long, value_name = "LEVEL")]
    pub log_level: Option<String>,

    /// Echo logs to standard output
    #[arg(short = 'e', long)]
    pub echo_to_stdout: bool,

    /// Disable desktop notifications
    #[arg(short = 'n', long)]
    pub disable_notifications: bool,

    /// Run in dry-run mode (don't execute actions)
    #[arg(short = 'd', long)]
    pub dry_run: bool,

    /// Clipboard format (plaintext, richtext, markdown)
    #[arg(short = 'c', long, value_name = "FORMAT")]
    pub clipboard_format: Option<String>,
    
    /// Result field to use (llm, raw, intermediate, auto)
    #[arg(short = 'r', long, value_name = "FIELD")]
    pub result_field: Option<String>,
    
    /// Disable logging completely (equivalent to logging to /dev/null)
    #[arg(long)]
    pub disable_logs: bool,
}

/// Application state shared between components
#[derive(Debug)]
pub struct AppState {
    pub config: AppConfig,
    pub clipboard_format: ClipboardFormat,
    pub dry_run: bool,
    pub disable_notifications: bool,
    pub disable_logs: bool,
}

/// Create a default config.toml file if it doesn't exist
pub fn create_default_config_file(path: &str) -> std::io::Result<()> {
    if Path::new(path).exists() {
        debug!("Config file already exists at {}", path);
        return Ok(());
    }

    debug!("Creating default config file at {}", path);
    
    let default_config = r#"# sw-catcher configuration
# Uncomment and modify the options you want to change

# watch_dir = "/path/to/directory"
# log_file = "sw-catcher.log"
log_level = "info"                # error, warn, info, debug, trace
echo_to_stdout = true
detect_keyphrases = true          # enable keyphrase detection
# disable_notifications = false
# dry_run = false
# disable_logs = false            # Disable logging completely
clipboard_format = "plaintext"    # plaintext, richtext, markdown
result_field_preference = "auto"  # llm, raw, intermediate, auto
# disable_clipboard = false         # Disable copying to clipboard

[keyphrases]
# Application examples
# "open browser" = "Firefox"
# "start notepad" = "notepad"

# Web service examples
# "search google" = "https://www.google.com/search?q="
# "search wikipedia" = "https://en.wikipedia.org/wiki/Special:Search?search="

[keyphrase_settings]
matching_strategy = "simple"     # simple, wholeword, exact
punctuation_handling = "sentence" # ignore, sentence, all

[text_cleaning]
trim_whitespace = true
normalize_newlines = true
remove_extra_spaces = true
capitalize_sentences = false
"#;

    let mut file = fs::File::create(path)?;
    file.write_all(default_config.as_bytes())?;
    
    debug!("Default config file created successfully");
    Ok(())
}

/// Load configuration from file and command line arguments
pub fn load_config() -> NotifyResult<AppState> {
    let opts = Opts::parse();

    // Load configuration file
    let config_path = "config.toml";
    let file_config = if std::path::Path::new(config_path).exists() {
        let config_content = std::fs::read_to_string(config_path)
            .map_err(|e| NotifyError::generic(&format!("Failed to read {}: {}", config_path, e)))?;

        toml::from_str::<AppConfig>(&config_content)
            .map_err(|e| NotifyError::generic(&format!("Invalid TOML in {}: {}", config_path, e)))?
    } else {
        debug!("No config file found at {}, using defaults", config_path);
        // Create a default configuration with application keyphrases
        let mut keyphrases = HashMap::new();
        keyphrases.insert("open browser".to_string(), "firefox".to_string());
        keyphrases.insert("search google".to_string(), "https://www.google.com/search?q=".to_string());
        
        AppConfig {
            watch_dir: None,
            log_file: None,
            log_level: None,
            echo_to_stdout: None,
            detect_keyphrases: Some(true), // Enable keyphrases by default
            keyphrases: Some(keyphrases),  // Add default keyphrases
            disable_notifications: None,
            dry_run: None,
            clipboard_format: None,
            result_field_preference: None,
            text_cleaning: None,
            disable_logs: None,
            disable_clipboard: None,
            keyphrase_settings: None,
        }
    };

    // Set up app state by combining file config and command line options
    let disable_notifications =
        opts.disable_notifications || file_config.disable_notifications.unwrap_or(false);
    let dry_run = opts.dry_run || file_config.dry_run.unwrap_or(false);
    let disable_logs = opts.disable_logs || file_config.disable_logs.unwrap_or(false);
    let clipboard_format = parse_clipboard_format(
        opts.clipboard_format
            .or(file_config.clipboard_format.clone())
            .unwrap_or_else(|| "plaintext".to_string())
            .as_str(),
    );

    // Override result_field_preference from command line if specified
    let config = if opts.result_field.is_some() {
        let mut updated_config = file_config.clone();
        updated_config.result_field_preference = opts.result_field;
        updated_config
    } else {
        file_config
    };

    // Validate watch path
    if let Some(ref watch_path) = opts
        .watch_dir
        .clone()
        .or_else(|| config.watch_dir.as_ref().map(PathBuf::from))
    {
        if !watch_path.exists() || !watch_path.is_dir() {
            error!("Watch directory does not exist: {:?}", watch_path);
            return Err(NotifyError::generic(&format!(
                "Watch directory does not exist: {:?}",
                watch_path
            )));
        }

        // Check if directory is readable
        match std::fs::read_dir(watch_path) {
            Ok(_) => {
                debug!("Watch directory is readable: {:?}", watch_path);
            }
            Err(e) => {
                error!(
                    "Watch directory exists but cannot be read: {:?} ({})",
                    watch_path, e
                );
                return Err(NotifyError::generic(&format!(
                    "Watch directory exists but cannot be read: {:?} ({})",
                    watch_path, e
                )));
            }
        }
    } else {
        error!("No watch directory specified in command line or config file");
        return Err(NotifyError::generic(
            "No watch directory specified in command line or config file",
        ));
    }

    Ok(AppState {
        config,
        clipboard_format,
        dry_run,
        disable_notifications,
        disable_logs,
    })
}

/// Parse a string into a log level
pub fn parse_log_level(level: &str) -> LevelFilter {
    match level.to_lowercase().as_str() {
        "error" => LevelFilter::Error,
        "warn" => LevelFilter::Warn,
        "info" => LevelFilter::Info,
        "debug" => LevelFilter::Debug,
        "trace" => LevelFilter::Trace,
        _ => {
            debug!("Invalid log level '{}', defaulting to 'info'", level);
            LevelFilter::Info
        }
    }
}

/// Parse a string into a result field preference
pub fn parse_result_field_preference(preference: &str) -> &'static str {
    match preference.to_lowercase().as_str() {
        "llm" => "llm",
        "raw" => "raw",
        "intermediate" => "intermediate",
        "auto" => "auto",
        _ => {
            debug!("Invalid result field preference '{}', defaulting to 'auto'", preference);
            "auto"
        }
    }
}

/// Parse a string into a KeyphraseMatchingStrategy
pub fn parse_keyphrase_matching_strategy(strategy: &str) -> KeyphraseMatchingStrategy {
    match strategy.to_lowercase().as_str() {
        "wholeword" | "whole_word" | "whole-word" => KeyphraseMatchingStrategy::WholeWord,
        "exact" => KeyphraseMatchingStrategy::Exact,
        _ => KeyphraseMatchingStrategy::Simple,  // Default to simple matching
    }
}

/// Parse a string into a PunctuationHandling
pub fn parse_punctuation_handling(handling: &str) -> PunctuationHandling {
    match handling.to_lowercase().as_str() {
        "ignore" => PunctuationHandling::IgnorePunctuation,
        "all" | "allpunctuation" | "all_punctuation" => PunctuationHandling::RemoveAllPunctuation,
        _ => PunctuationHandling::RemoveSentenceEnding,  // Default to sentence-ending
    }
}

/// Get the default log directory based on platform
pub fn get_default_log_directory() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.join("Library").join("Logs").join("sw-catcher")
    }
    #[cfg(not(target_os = "macos"))]
    {
        PathBuf::from(".")
    }
}

/// Get the configured log file path
pub fn get_log_file_path(opts: &Opts, config: &AppConfig) -> String {
    // If logging is disabled, return /dev/null
    if opts.disable_logs || config.disable_logs.unwrap_or(false) {
        return "/dev/null".to_string();
    }
    
    // Check command line args first
    if let Some(ref path) = opts.log_file {
        return path.to_string_lossy().to_string();
    }
    
    // Check config file next
    if let Some(ref path) = config.log_file {
        return path.to_string();
    }
    
    // Use platform-specific default
    let default_dir = get_default_log_directory();
    
    // Create the directory if it doesn't exist
    if !default_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(&default_dir) {
            eprintln!("Warning: Could not create log directory: {}", e);
            return "sw-catcher.log".to_string();
        }
    }
    
    default_dir.join("sw-catcher.log").to_string_lossy().to_string()
}

/// Get the watch directory path
pub fn get_watch_path(opts: &Opts, config: &AppConfig) -> Option<PathBuf> {
    opts.watch_dir
        .clone()
        .or_else(|| config.watch_dir.as_ref().map(PathBuf::from))
}

/// Get the configured log level
pub fn get_log_level(opts: &Opts, config: &AppConfig) -> LevelFilter {
    parse_log_level(
        opts.log_level
            .as_deref()
            .or(config.log_level.as_deref())
            .unwrap_or("info"),
    )
}

/// Print usage guide to stderr
pub fn print_usage_guide() {
    eprintln!("\nUsage Guide:");
    eprintln!("  1. Specify a watch directory with --watch-dir");
    eprintln!("  2. OR create a config.toml with configuration options:");
    eprintln!("     Example config.toml:");
    eprintln!("     watch_dir = \"/path/to/directory\"");
    eprintln!("     log_file = \"sw-catcher.log\"");
    eprintln!("     log_level = \"info\"  # error, warn, info, debug, trace");
    eprintln!("     echo_to_stdout = true");
    eprintln!("     detect_keyphrases = true");
    eprintln!("     disable_notifications = false");
    eprintln!("     dry_run = false");
    eprintln!("     disable_logs = false  # Disable logging completely");
    eprintln!("     clipboard_format = \"plaintext\"  # plaintext, richtext, markdown");
    eprintln!("     result_field_preference = \"auto\"  # llm, raw, intermediate, auto");
    eprintln!("     [keyphrases]");
    eprintln!("     # Keyphrase examples:");
    eprintln!("     \"open browser\" = \"https://www.example.com\"");
    eprintln!("     \"send email\" = \"mailto:user@example.com\"");
    eprintln!("     \"start notepad\" = \"notepad\"");
    eprintln!("     \"important reminder\" = \"\"  # Empty action, just detect");
    eprintln!("     [keyphrase_settings]");
    eprintln!("     matching_strategy = \"simple\"  # simple, wholeword, exact");
    eprintln!("     punctuation_handling = \"sentence\"  # ignore, sentence, all");
    eprintln!("     [text_cleaning]");
    eprintln!("     trim_whitespace = true");
    eprintln!("     normalize_newlines = true");
    eprintln!("     remove_extra_spaces = true");
    eprintln!("     capitalize_sentences = false");
    eprintln!("\nRun with --help for more information.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_log_level() {
        assert_eq!(parse_log_level("error"), LevelFilter::Error);
        assert_eq!(parse_log_level("warn"), LevelFilter::Warn);
        assert_eq!(parse_log_level("info"), LevelFilter::Info);
        assert_eq!(parse_log_level("debug"), LevelFilter::Debug);
        assert_eq!(parse_log_level("trace"), LevelFilter::Trace);
        assert_eq!(parse_log_level("invalid"), LevelFilter::Info);
    }
    
    #[test]
    fn test_parse_keyphrase_matching_strategy() {
        assert_eq!(parse_keyphrase_matching_strategy("simple"), KeyphraseMatchingStrategy::Simple);
        assert_eq!(parse_keyphrase_matching_strategy("wholeword"), KeyphraseMatchingStrategy::WholeWord);
        assert_eq!(parse_keyphrase_matching_strategy("whole_word"), KeyphraseMatchingStrategy::WholeWord);
        assert_eq!(parse_keyphrase_matching_strategy("whole-word"), KeyphraseMatchingStrategy::WholeWord);
        assert_eq!(parse_keyphrase_matching_strategy("exact"), KeyphraseMatchingStrategy::Exact);
        assert_eq!(parse_keyphrase_matching_strategy("invalid"), KeyphraseMatchingStrategy::Simple);
    }
    
    #[test]
    fn test_parse_punctuation_handling() {
        assert_eq!(parse_punctuation_handling("ignore"), PunctuationHandling::IgnorePunctuation);
        assert_eq!(parse_punctuation_handling("sentence"), PunctuationHandling::RemoveSentenceEnding);
        assert_eq!(parse_punctuation_handling("all"), PunctuationHandling::RemoveAllPunctuation);
        assert_eq!(parse_punctuation_handling("allpunctuation"), PunctuationHandling::RemoveAllPunctuation);
        assert_eq!(parse_punctuation_handling("all_punctuation"), PunctuationHandling::RemoveAllPunctuation);
        assert_eq!(parse_punctuation_handling("invalid"), PunctuationHandling::RemoveSentenceEnding);
    }

    #[test]
    fn test_parse_result_field_preference() {
        assert_eq!(parse_result_field_preference("llm"), "llm");
        assert_eq!(parse_result_field_preference("LLM"), "llm");
        assert_eq!(parse_result_field_preference("raw"), "raw");
        assert_eq!(parse_result_field_preference("intermediate"), "intermediate");
        assert_eq!(parse_result_field_preference("auto"), "auto");
        assert_eq!(parse_result_field_preference("invalid"), "auto");
    }
}