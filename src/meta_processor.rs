use crate::clipboard::ensure_clipboard_content_with_monitoring;
use crate::clipboard::truncate;
use crate::config::AppState;
use crate::extract_text_by_preference;
use crate::keyphrase::{process_keyphrases_enhanced, KeyphraseAction, KeyphraseProcessingOptions};
use crate::text_processing::apply_text_cleaning;
use log::{debug, error, info};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::collections::HashMap;
use std::thread::sleep;

// Type alias for the map tracking recently processed files
pub type LastProcessedMap = Arc<Mutex<HashMap<PathBuf, Instant>>>;

/// Process a meta.json file
pub fn process_meta_file(
    path: &Path,
    last_processed: &LastProcessedMap,
    keyphrases: &[KeyphraseAction],
    keyphrase_options: &KeyphraseProcessingOptions,
    app_state: &Arc<AppState>,
) {
    // Debounce: Check if we've recently processed this file
    let now = Instant::now();
    {
        let mut map = last_processed.lock().unwrap();
        if let Some(last_time) = map.get(path) {
            if now.duration_since(*last_time) < Duration::from_secs(1) {
                debug!("Skipping recently processed file: {}", path.display());
                return;
            }
        }
        // Update the last processed time
        map.insert(path.to_path_buf(), now);
    }

    info!("Found new meta.json at {}", path.display());

    // Retry configuration
    let max_retries = 5;
    let retry_delay = Duration::from_millis(500); // 500ms delay between retries

    for attempt in 1..=max_retries {
        // Read the file content
        let txt = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                error!("Couldn't read {} (attempt {}/{}): {}", path.display(), attempt, max_retries, e);
                if attempt < max_retries {
                    debug!("Retrying in {:?}...", retry_delay);
                    sleep(retry_delay);
                    continue;
                }
                return;
            }
        };

        // Parse the JSON
        match serde_json::from_str::<crate::Meta>(&txt) {
            Ok(meta) => {
                // Get the preference from config
                let preference = app_state.config.result_field_preference.as_deref().unwrap_or("auto");
                
                // Extract text based on preference
                if let Some(text) = extract_text_by_preference(&meta, preference) {
                    // Log which field was used
                    match preference {
                        "llm" => debug!("Using LLM result field"),
                        "raw" => debug!("Using raw result field"),
                        "intermediate" => debug!("Using intermediate result field"),
                        _ => {
                            if meta.llm_result.is_some() {
                                debug!("Auto mode: Using LLM result field");
                            } else if meta.result.is_some() {
                                debug!("Auto mode: Using intermediate result field");
                            } else if meta.raw_result.is_some() {
                                debug!("Auto mode: Using raw result field");
                            }
                        }
                    }
                    
                    // Process keyphrases and get cleaned text
                    let cleaned_text = if !keyphrases.is_empty() {
                        process_keyphrases_enhanced(&text, keyphrases, app_state.dry_run, keyphrase_options)
                    } else {
                        text.clone()
                    };

                    // Apply text cleaning if configured
                    let final_text = apply_text_cleaning(&cleaned_text, &app_state.config);

                    // Copy to clipboard with monitoring for changes
                    match ensure_clipboard_content_with_monitoring(&final_text, &app_state.clipboard_format) {
                        Ok(_) => {
                            info!("Copied to clipboard: {}", truncate(&final_text, 60));
                        }
                        Err(e) => error!("Clipboard error: {}", e),
                    }
                    return; // Success! Exit function
                } else {
                    if attempt < max_retries {
                        debug!("No text found in the specified field '{}' (attempt {}/{}). Retrying in {:?}...", 
                              preference, attempt, max_retries, retry_delay);
                        sleep(retry_delay);
                        continue;
                    } else {
                        error!("No text found in the specified field: {}", preference);
                        log_unknown_json_structure(&txt);
                    }
                }
            }
            Err(e) => {
                if attempt < max_retries {
                    debug!("JSON parse error in {} (attempt {}/{}): {}. Retrying in {:?}...", 
                          path.display(), attempt, max_retries, e, retry_delay);
                    sleep(retry_delay);
                    continue;
                } else {
                    error!("JSON parse error in {}: {}", path.display(), e);
                    log_unknown_json_structure(&txt);
                }
            }
        }
    }
}

/// Log details about an unknown JSON structure
pub fn log_unknown_json_structure(json_text: &str) {
    error!("Unknown JSON structure in meta file");
    if let Ok(value) = serde_json::from_str::<Value>(json_text) {
        // Log available top-level keys to help debugging
        if let Some(obj) = value.as_object() {
            let keys = obj.keys().collect::<Vec<_>>();
            error!("Available keys in JSON: {:?}", keys);
            
            // Log a sample of each field's content
            for key in keys {
                if let Some(value) = obj.get(key) {
                    let value_str = match value {
                        Value::String(s) => truncate(s, 30),
                        _ => truncate(&value.to_string(), 30)
                    };
                    info!("Field '{}' contains: {}", key, value_str);
                }
            }
        }
    } else {
        // JSON is invalid
        error!("JSON content is not valid: {}", truncate(json_text, 100));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Tests can be added here later
}