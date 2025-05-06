use crate::config::AppConfig;
use log::warn;
use regex::Regex;

/// Apply text cleaning operations based on configuration
pub fn apply_text_cleaning(text: &str, config: &AppConfig) -> String {
    let text_cleaning = match &config.text_cleaning {
        Some(options) => options,
        None => return text.to_string(),
    };

    let mut result = text.to_string();

    // Trim whitespace if configured
    if text_cleaning.trim_whitespace {
        result = result.trim().to_string();
    }

    // Normalize newlines if configured
    if text_cleaning.normalize_newlines {
        // Replace \r\n with \n
        result = result.replace("\r\n", "\n");
    }

    // Remove extra spaces if configured
    if text_cleaning.remove_extra_spaces {
        if let Ok(re) = Regex::new(r"\s+") {
            result = re.replace_all(&result, " ").to_string();
        } else {
            warn!("Failed to compile regex for removing extra spaces");
        }
    }

    // Capitalize sentences if configured
    if text_cleaning.capitalize_sentences {
        result = capitalize_sentences(&result);
    }

    result
}

/// Capitalize the first letter of each sentence
fn capitalize_sentences(text: &str) -> String {
    match Regex::new(r"(?:^|[.!?]\s+)([a-z])") {
        Ok(re) => re
            .replace_all(text, |caps: &regex::Captures| {
                let matched = caps.get(1).unwrap().as_str();
                format!(
                    "{}{}",
                    &caps[0][0..caps[0].len() - 1],
                    matched.to_uppercase()
                )
            })
            .to_string(),
        Err(e) => {
            warn!("Failed to compile regex for capitalizing sentences: {}", e);
            text.to_string()
        }
    }
}

/// Trim leading and trailing whitespace
pub fn trim_whitespace(text: &str) -> String {
    text.trim().to_string()
}

/// Remove consecutive whitespace characters
pub fn normalize_whitespace(text: &str) -> String {
    match Regex::new(r"\s+") {
        Ok(re) => re.replace_all(text, " ").to_string(),
        Err(_) => {
            // Fallback implementation if regex fails
            let mut result = String::with_capacity(text.len());
            let mut last_was_whitespace = false;

            for c in text.chars() {
                if c.is_whitespace() {
                    if !last_was_whitespace {
                        result.push(' ');
                        last_was_whitespace = true;
                    }
                } else {
                    result.push(c);
                    last_was_whitespace = false;
                }
            }

            result
        }
    }
}

/// Convert Windows-style line endings to Unix-style
pub fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// Process text segments, handling each one appropriately
pub fn process_text_segments(segments: &[crate::keyphrase::TextSegment]) -> String {
    let mut result = String::new();

    for segment in segments {
        result.push_str(&segment.text);
    }

    // Normalize whitespace in the result
    normalize_whitespace(&result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TextCleaningOptions;

    #[test]
    fn test_trim_whitespace() {
        assert_eq!(trim_whitespace("  hello  "), "hello");
        assert_eq!(trim_whitespace("\n\thello\n\t"), "hello");
    }

    #[test]
    fn test_normalize_whitespace() {
        assert_eq!(normalize_whitespace("hello  world"), "hello world");
        assert_eq!(normalize_whitespace("hello\t\nworld"), "hello world");
    }

    #[test]
    fn test_normalize_newlines() {
        assert_eq!(normalize_newlines("hello\r\nworld"), "hello\nworld");
    }

    #[test]
    fn test_capitalize_sentences() {
        assert_eq!(
            capitalize_sentences("hello. this is a test. another sentence!"),
            "hello. This is a test. Another sentence!"
        );
    }

    #[test]
    #[test]
    fn test_apply_text_cleaning() {
        // Create test config with all options enabled
        let options = TextCleaningOptions {
            trim_whitespace: true,
            normalize_newlines: true,
            remove_extra_spaces: true,
            capitalize_sentences: true,
        };

        let config = AppConfig {
            watch_dir: None,
            log_file: None,
            log_level: None,
            echo_to_stdout: None,
            detect_keyphrases: None,
            keyphrases: None,
            dry_run: None,
            clipboard_format: None,
            text_cleaning: Some(options),
            disable_logs: None,
            keyphrase_settings: None,
            disable_clipboard: None,
            mode_name: None,
            result_field_preference: None,
        };

        let input = "  hello  world.\r\n  this is a test.  ";
        let expected = "Hello world. This is a test.";

        assert_eq!(apply_text_cleaning(input, &config), expected);
    }

    #[test]
    fn test_process_text_segments() {
        use crate::keyphrase::TextSegment;

        let segments = vec![
            TextSegment {
                text: "Hello ".to_string(),
                follows_keyphrase: None,
                precedes_keyphrase: Some("open notes".to_string()),
            },
            TextSegment {
                text: " my notes are here.".to_string(),
                follows_keyphrase: Some("open notes".to_string()),
                precedes_keyphrase: None,
            },
        ];

        assert_eq!(process_text_segments(&segments), "Hello my notes are here.");
    }
}
