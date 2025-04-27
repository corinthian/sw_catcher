use crate::actions::{execute_action, parse_action, ActionType};
use crate::config::AppConfig;
use log::{info, warn, debug};
use regex::Regex;

/// Keyphrase with associated action
#[derive(Debug, Clone)]
pub struct KeyphraseAction {
    pub keyphrase: String,
    pub action: ActionType,
}

/// Strategy for matching keyphrases in text
#[derive(Debug, Clone, PartialEq)]
pub enum KeyphraseMatchingStrategy {
    /// Simple substring match (case-insensitive)
    Simple,
    /// Match only whole words (case-insensitive)
    WholeWord,
    /// Exact case-sensitive match
    Exact,
}

/// How to handle punctuation after keyphrases
#[derive(Debug, Clone, PartialEq)]
pub enum PunctuationHandling {
    /// Don't remove any punctuation
    IgnorePunctuation,
    /// Remove sentence-ending punctuation (.!?)
    RemoveSentenceEnding,
    /// Remove all punctuation
    RemoveAllPunctuation,
}

/// Options for keyphrase matching and processing
#[derive(Debug, Clone)]
pub struct KeyphraseProcessingOptions {
    pub matching_strategy: KeyphraseMatchingStrategy,
    pub punctuation_handling: PunctuationHandling,
}

impl Default for KeyphraseProcessingOptions {
    fn default() -> Self {
        Self {
            matching_strategy: KeyphraseMatchingStrategy::Simple,
            punctuation_handling: PunctuationHandling::RemoveSentenceEnding,
        }
    }
}

/// A matched keyphrase with position information
#[derive(Debug, Clone)]
pub struct KeyphraseMatch {
    pub keyphrase: String,
    pub action: ActionType,
    pub start_pos: usize,
    pub end_pos: usize,
}

/// A segment of text between keyphrases
#[derive(Debug, Clone)]
pub struct TextSegment {
    pub text: String,
    pub follows_keyphrase: Option<String>, // The keyphrase that came before this segment
    pub precedes_keyphrase: Option<String>, // The keyphrase that comes after this segment
}

/// Extract keyphrase actions from configuration
pub fn parse_keyphrases(config: &AppConfig) -> Vec<KeyphraseAction> {
    let mut keyphrases = Vec::new();

    if let Some(true) = config.detect_keyphrases {
        if let Some(kp_map) = &config.keyphrases {
            for (phrase, action_str) in kp_map {
                let action = parse_action(action_str);
                keyphrases.push(KeyphraseAction {
                    keyphrase: phrase.clone(),
                    action,
                });
            }
        }
    }

    keyphrases
}

/// Process text to detect and act on keyphrases
///
/// Returns the modified text with keyphrases removed
pub fn process_keyphrases(text: &str, keyphrases: &[KeyphraseAction], dry_run: bool) -> String {
    process_keyphrases_enhanced(text, keyphrases, dry_run, &KeyphraseProcessingOptions::default())
}

/// Enhanced keyphrase processing with configurable options
pub fn process_keyphrases_enhanced(
    text: &str, 
    keyphrases: &[KeyphraseAction], 
    dry_run: bool,
    options: &KeyphraseProcessingOptions,
) -> String {
    // Detect all keyphrases in the text
    let matches = detect_all_keyphrases(text, keyphrases, options);
    
    // If no keyphrases found, return the original text
    if matches.is_empty() {
        return text.to_string();
    }
    
    // Process the chained actions
    process_chained_actions(text, &matches, dry_run)
}

/// Find a keyphrase in text based on matching strategy
fn find_keyphrase(text: &str, keyphrase: &str, options: &KeyphraseProcessingOptions) -> Option<usize> {
    match options.matching_strategy {
        KeyphraseMatchingStrategy::Simple => {
            text.to_lowercase().find(&keyphrase.to_lowercase())
        },
        KeyphraseMatchingStrategy::WholeWord => {
            // Pattern that matches the phrase as whole words
            if let Ok(pattern) = Regex::new(&format!(
                "(?i)\\b{}\\b", 
                regex::escape(keyphrase)
            )) {
                pattern.find(text).map(|m| m.start())
            } else {
                None
            }
        },
        KeyphraseMatchingStrategy::Exact => {
            // Exact case-sensitive match
            text.find(keyphrase)
        },
    }
}

/// Detect all keyphrases in a text along with their positions
pub fn detect_all_keyphrases(
    text: &str,
    keyphrases: &[KeyphraseAction],
    options: &KeyphraseProcessingOptions,
) -> Vec<KeyphraseMatch> {
    let mut matches = Vec::new();
    
    for ka in keyphrases {
        // Find all instances of this keyphrase in the text
        let mut start = 0;
        while let Some(pos) = find_keyphrase(&text[start..], &ka.keyphrase, options) {
            let absolute_pos = start + pos;
            matches.push(KeyphraseMatch {
                keyphrase: ka.keyphrase.clone(),
                action: ka.action.clone(),
                start_pos: absolute_pos,
                end_pos: absolute_pos + ka.keyphrase.len(),
            });
            start = absolute_pos + ka.keyphrase.len(); // Move past this match
        }
    }
    
    // Sort matches by position to ensure correct order of execution
    matches.sort_by_key(|m| m.start_pos);
    
    // Log the detected keyphrases in order
    if !matches.is_empty() {
        debug!("Detected {} keyphrases in order:", matches.len());
        for (i, m) in matches.iter().enumerate() {
            debug!("  {}. \"{}\" at position {}", i+1, m.keyphrase, m.start_pos);
        }
    }
    
    matches
}

/// Split text into segments between keyphrases
pub fn segment_text(
    text: &str,
    keyphrase_matches: &[KeyphraseMatch],
) -> Vec<TextSegment> {
    let mut segments = Vec::new();
    let mut current_pos = 0;
    
    for (i, km) in keyphrase_matches.iter().enumerate() {
        // Text before this keyphrase
        if km.start_pos > current_pos {
            let segment_text = &text[current_pos..km.start_pos];
            segments.push(TextSegment {
                text: segment_text.to_string(),
                follows_keyphrase: if i > 0 {
                    Some(keyphrase_matches[i-1].keyphrase.clone())
                } else {
                    None
                },
                precedes_keyphrase: Some(km.keyphrase.clone()),
            });
        }
        
        current_pos = km.end_pos;
    }
    
    // Add final segment after last keyphrase
    if current_pos < text.len() {
        segments.push(TextSegment {
            text: text[current_pos..].to_string(),
            follows_keyphrase: keyphrase_matches.last().map(|km| km.keyphrase.clone()),
            precedes_keyphrase: None,
        });
    }
    
    segments
}

/// Process and execute chained actions in the order they appear in text
pub fn process_chained_actions(
    text: &str,
    matches: &[KeyphraseMatch],
    dry_run: bool,
) -> String {
    // Split text into segments
    
    // Log the execution sequence
    if !matches.is_empty() {
        info!("Executing {} keyphrase actions in sequence:", matches.len());
        for (i, km) in matches.iter().enumerate() {
            info!("  {}. Will execute \"{}\"", i+1, km.keyphrase);
        }
    }
    
    // Execute actions in sequence
    for (i, km) in matches.iter().enumerate() {
        if dry_run {
            info!(
                "DRY-RUN: Would execute action #{} for keyphrase: \"{}\"",
                i+1, km.keyphrase
            );
        } else {
            info!("Executing action #{} for keyphrase: \"{}\"", i+1, km.keyphrase);
            
            // Execute the action
            match execute_action(&km.action) {
                Ok(_) => {
                    info!(
                        "Successfully executed action for keyphrase: \"{}\"",
                        km.keyphrase
                    );
                }
                Err(e) => {
                    warn!(
                        "Failed to execute action for keyphrase \"{}\": {}",
                        km.keyphrase, e
                    );
                }
            }
        }
    }
    
    // Construct the cleaned text (without keyphrases)
    let mut result = String::new();
    
    // List of common punctuation characters to check for
    let punctuation_chars = vec![',', ';', ':', '.', '!', '?', '\'', '"', ')', '}', ']'];
    
    // We need to reconstruct the original text without the keyphrases
    let mut last_end = 0;
    for km in matches {
        // Add text from last end to current start, handling punctuation
        if km.start_pos > last_end {
            let mut pre_text = text[last_end..km.start_pos].to_string();
            
            // Check if pre_text ends with punctuation
            if let Some(last_char) = pre_text.chars().last() {
                if punctuation_chars.contains(&last_char) {
                    // Remove the trailing punctuation
                    pre_text.pop();
                }
            }
            
            result.push_str(&pre_text);
        }
        
        // Skip the keyphrase
        last_end = km.end_pos;
        
        // Skip any punctuation immediately after the keyphrase
        if last_end < text.len() {
            if let Some(next_char) = text[last_end..].chars().next() {
                if punctuation_chars.contains(&next_char) {
                    last_end += next_char.len_utf8();
                }
            }
        }
    }
    
    // Add any remaining text
    if last_end < text.len() {
        result.push_str(&text[last_end..]);
    }
    
    // Clean up any remaining issues
    
    // Remove leading punctuation
    while !result.is_empty() {
        if let Some(first_char) = result.chars().next() {
            if punctuation_chars.contains(&first_char) {
                result.remove(0);
            } else {
                break;
            }
        } else {
            break;
        }
    }
    
    // Normalize whitespace by replacing multiple spaces with a single space
    let mut normalized = String::with_capacity(result.len());
    let mut last_was_whitespace = false;
    
    for c in result.chars() {
        if c.is_whitespace() {
            if !last_was_whitespace {
                normalized.push(' ');
                last_was_whitespace = true;
            }
        } else {
            normalized.push(c);
            last_was_whitespace = false;
        }
    }
    
    normalized.trim().to_string()
}

/// Get list of keyphrases only (for display/logging purposes)
pub fn get_keyphrase_list(keyphrases: &[KeyphraseAction]) -> Vec<String> {
    keyphrases.iter().map(|ka| ka.keyphrase.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_parse_keyphrases() {
        let mut keyphrases_map = HashMap::new();
        keyphrases_map.insert(
            String::from("open browser"),
            String::from("https://example.com")
        );

        let config = AppConfig {
            detect_keyphrases: Some(true),
            keyphrases: Some(keyphrases_map),
            watch_dir: None,
            log_file: None,
            log_level: None,
            echo_to_stdout: None,
            disable_notifications: None,
            dry_run: None,
            clipboard_format: None,
            text_cleaning: None,
            disable_logs: None,
            keyphrase_settings: None,
        };

        let keyphrases = parse_keyphrases(&config);
        assert_eq!(keyphrases.len(), 1);

        // Verify the keyphrases were parsed correctly
        let phrases: Vec<String> = keyphrases.iter().map(|ka| ka.keyphrase.clone()).collect();
        assert!(phrases.contains(&String::from("open browser")));
    }

    #[test]
    fn test_detect_all_keyphrases() {
        let keyphrases = vec![
            KeyphraseAction {
                keyphrase: String::from("open notes"),
                action: ActionType::OpenApplication(String::from("Notes")),
            },
        ];
        
        let text = "I need to open notes for this meeting.";
        let options = KeyphraseProcessingOptions::default();
        
        let matches = detect_all_keyphrases(text, &keyphrases, &options);
        
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].keyphrase, "open notes");
        assert_eq!(matches[0].start_pos, 10);
    }
    
    #[test]
    fn test_segment_text() {
        let matches = vec![
            KeyphraseMatch {
                keyphrase: String::from("open notes"),
                action: ActionType::OpenApplication(String::from("Notes")),
                start_pos: 10,
                end_pos: 20,
            },
        ];
        
        let text = "I need to open notes for this meeting.";
        
        let segments = segment_text(text, &matches);
        
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].text, "I need to ");
        assert_eq!(segments[1].text, " for this meeting.");
    }
    
    #[test]
    fn test_process_chained_actions() {
        let matches = vec![
            KeyphraseMatch {
                keyphrase: String::from("open notes"),
                action: ActionType::None, // Use None for testing
                start_pos: 10,
                end_pos: 20,
            },
        ];
        
        let text = "I need to open notes for this meeting.";
        
        let result = process_chained_actions(text, &matches, true);
        
        // Expected: keyphrases removed
        assert_eq!(result, "I need to for this meeting.");
    }
    
    #[test]
    fn test_chained_actions_realistic_example() {
        let keyphrases = vec![
            KeyphraseAction {
                keyphrase: String::from("open notes"),
                action: ActionType::None, // Use None for testing
            },
            KeyphraseAction {
                keyphrase: String::from("create reminder"),
                action: ActionType::None, // Use None for testing
            },
        ];
        
        let text = "My meeting with the marketing team went well. Open notes I need to follow up with Sarah about the Q3 budget by Friday. We also discussed the new product launch. Create reminder Call John about partnership opportunity. Overall the project is on track for delivery.";
        
        let result = process_keyphrases_enhanced(text, &keyphrases, true, &KeyphraseProcessingOptions::default());
        
        // Expected: keyphrases removed but content preserved
        let expected = "My meeting with the marketing team went well. I need to follow up with Sarah about the Q3 budget by Friday. We also discussed the new product launch. Call John about partnership opportunity. Overall the project is on track for delivery.";
        
        assert_eq!(result, expected);
    }
}