use crate::config::AppState;
use crate::config::{parse_keyphrase_matching_strategy, parse_punctuation_handling};
use crate::keyphrase::{KeyphraseAction, KeyphraseProcessingOptions};
use crate::meta_processor::LastProcessedMap;
use log::{debug, error, info};
use notify::{
    Config, EventKind, RecommendedWatcher, RecursiveMode, Result as NotifyResult, Watcher,
    event::{AccessKind, AccessMode},
};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use std::collections::HashMap;

/// Start watching a directory for meta.json files
pub fn start_watcher(app_state: AppState) -> NotifyResult<()> {
    // Get the watch path from configuration
    let watch_path = match &app_state.config.watch_dir {
        Some(dir) => PathBuf::from(dir),
        None => {
            error!("No watch directory specified");
            return Err(notify::Error::generic("No watch directory specified"));
        }
    };

    info!("Watching for meta.json in {:?}", watch_path);
    info!("Any LLM results will be copied to your clipboard");

    // Display result field preference
    if let Some(pref) = &app_state.config.result_field_preference {
        info!("Using result field preference: {}", pref);
    } else {
        info!("Using default result field preference: auto");
    }

    // Create shared state
    let app_state = Arc::new(app_state);
    let last_processed = Arc::new(Mutex::new(HashMap::new()));

    // Parse keyphrases and keyphrase options
    let (keyphrases, keyphrase_options) = parse_keyphrases_from_config(&app_state);
    let keyphrases = Arc::new(keyphrases);
    let keyphrase_options = Arc::new(keyphrase_options);

    // Clone references for the watcher closure
    let last_processed_clone = last_processed.clone();
    let keyphrases_clone = keyphrases.clone();
    let keyphrase_options_clone = keyphrase_options.clone();
    let app_state_clone = app_state.clone();

    // Create and configure the file watcher
    let mut watcher: RecommendedWatcher = RecommendedWatcher::new(
        move |res: NotifyResult<notify::Event>| match res {
            Ok(e) => handle_event(
                &e,
                &last_processed_clone,
                &keyphrases_clone,
                &keyphrase_options_clone,
                &app_state_clone,
            ),
            Err(e) => error!("Watch error: {:?}", e),
        },
        Config::default(),
    )?;

    // Start watching the directory
    watcher.watch(&watch_path, RecursiveMode::Recursive)?;
    info!("File watcher initialized successfully");

    // Keep alive and clean up old entries periodically
    loop {
        thread::sleep(Duration::from_secs(60));

        // Cleanup old entries from the debounce map
        let mut map = last_processed.lock().unwrap();
        let now = Instant::now();
        let old_len = map.len();
        map.retain(|_, timestamp| now.duration_since(*timestamp) < Duration::from_secs(600)); // Remove entries older than 10 minutes
        let new_len = map.len();
        if old_len != new_len {
            debug!(
                "Cleaned up {} old entries from debounce map",
                old_len - new_len
            );
        }
    }
}

/// Parse keyphrases and keyphrase processing options from the application configuration
fn parse_keyphrases_from_config(app_state: &Arc<AppState>) -> (Vec<KeyphraseAction>, KeyphraseProcessingOptions) {
    use crate::keyphrase::parse_keyphrases;
    let keyphrases = parse_keyphrases(&app_state.config);

    // Log keyphrase configuration
    if app_state.config.detect_keyphrases.unwrap_or(false) {
        info!("Keyphrase detection enabled");
        debug!("Configured {} keyphrases", keyphrases.len());
        for ka in &keyphrases {
            debug!("Keyphrase: \"{}\"", ka.keyphrase);
        }
    } else {
        debug!("Keyphrase detection disabled");
    }

    // Parse keyphrase processing options
    let mut options = KeyphraseProcessingOptions::default();
    
    if let Some(keyphrase_settings) = &app_state.config.keyphrase_settings {
        if let Some(strategy) = &keyphrase_settings.matching_strategy {
            options.matching_strategy = parse_keyphrase_matching_strategy(strategy);
            debug!("Using keyphrase matching strategy: {:?}", options.matching_strategy);
        }
        
        if let Some(handling) = &keyphrase_settings.punctuation_handling {
            options.punctuation_handling = parse_punctuation_handling(handling);
            debug!("Using punctuation handling: {:?}", options.punctuation_handling);
        }
    }

    (keyphrases, options)
}

/// Handle file system events
fn handle_event(
    evt: &notify::Event,
    last_processed: &LastProcessedMap,
    keyphrases: &[KeyphraseAction],
    keyphrase_options: &KeyphraseProcessingOptions,
    app_state: &Arc<AppState>,
) {
    // Track if we should process any files in this event
    let mut process_files = false;
    let mut paths_to_process = Vec::new();
    
    // Check event type
    match &evt.kind {
        // Process on file creation (might be premature, but keeping for backward compatibility)
        EventKind::Create(_) => {
            debug!("Create event detected for {:?}", evt.paths);
            process_files = true;
            paths_to_process.extend(evt.paths.iter().cloned());
        },
        
        // Process when a file has been closed after writing
        EventKind::Access(AccessKind::Close(AccessMode::Write)) => {
            debug!("Close(Write) event detected for {:?}", evt.paths);
            process_files = true;
            paths_to_process.extend(evt.paths.iter().cloned());
        },
        
        // Ignore other event types
        _ => {}
    }
    
    // Process any identified files
    if process_files {
        for path in &paths_to_process {
            if is_meta_json_file(path) {
                debug!("Processing meta.json file after write completion: {:?}", path);
                crate::meta_processor::process_meta_file(path, last_processed, keyphrases, keyphrase_options, app_state);
            }
        }
    }
}

/// Check if a path is a meta.json file
fn is_meta_json_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|s| s.to_str())
        .map_or(false, |s| s == "meta.json")
}