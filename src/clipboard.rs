use copypasta::{ClipboardContext, ClipboardProvider};
use log::{debug, warn};
use serde::Deserialize;
use std::io::{Error, ErrorKind};
use std::time::Duration;
use std::thread;

/// Clipboard format options
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ClipboardFormat {
    PlainText,
    RichText,
    Markdown,
}

/// Parse a string into a ClipboardFormat
pub fn parse_clipboard_format(format: &str) -> ClipboardFormat {
    match format.to_lowercase().as_str() {
        "richtext" => ClipboardFormat::RichText,
        "markdown" => ClipboardFormat::Markdown,
        _ => ClipboardFormat::PlainText,
    }
}

/// Get the current clipboard content
pub fn get_clipboard_content() -> std::io::Result<String> {
    let mut ctx = ClipboardContext::new().map_err(|e| {
        Error::new(
            ErrorKind::Other,
            format!("Failed to access clipboard: {}", e),
        )
    })?;
    
    ctx.get_contents().map_err(|e| {
        Error::new(
            ErrorKind::Other,
            format!("Failed to get clipboard contents: {}", e),
        )
    })
}

/// Normalize text for comparison by trimming whitespace and normalizing newlines
fn normalize_for_comparison(text: &str) -> String {
    text.trim().replace("\r\n", "\n").replace('\r', "\n")
}

/// Copy text to system clipboard with format support
pub fn copy_to_clipboard_with_format(text: &str, format: &ClipboardFormat) -> std::io::Result<()> {
    let mut ctx = ClipboardContext::new().map_err(|e| {
        Error::new(
            ErrorKind::Other,
            format!("Failed to access clipboard: {}", e),
        )
    })?;

    match format {
        ClipboardFormat::PlainText => ctx.set_contents(text.to_owned()).map_err(|e| {
            Error::new(
                ErrorKind::Other,
                format!("Failed to set clipboard contents: {}", e),
            )
        }),
        ClipboardFormat::RichText => {
            #[cfg(target_os = "windows")]
            {
                debug!("Rich text clipboard format requested - platform support limited");
                let html_content = format!(
                    "<div style=\"font-family: system-ui;\">{}</div>",
                    text.replace("\n", "<br>")
                );

                // TODO: Implement proper HTML clipboard support on Windows
                // For now, fallback to plain text
                debug!(
                    "Using fallback to plain text (HTML: {})",
                    truncate(&html_content, 50)
                );

                ctx.set_contents(text.to_owned()).map_err(|e| {
                    Error::new(
                        ErrorKind::Other,
                        format!("Failed to set clipboard contents: {}", e),
                    )
                })
            }
            #[cfg(not(target_os = "windows"))]
            {
                debug!("Rich text clipboard format requested - platform support limited");
                ctx.set_contents(text.to_owned()).map_err(|e| {
                    Error::new(
                        ErrorKind::Other,
                        format!("Failed to set clipboard contents: {}", e),
                    )
                })
            }
        }
        ClipboardFormat::Markdown => {
            debug!("Markdown clipboard format requested");

            // On most platforms, we'll just put the plain text,
            // but applications that understand markdown will interpret it correctly
            ctx.set_contents(text.to_owned()).map_err(|e| {
                Error::new(
                    ErrorKind::Other,
                    format!("Failed to set clipboard contents: {}", e),
                )
            })
        }
    }
}

/// Ensure our content is in the clipboard by monitoring for changes
pub fn ensure_clipboard_content_with_monitoring(text: &str, format: &ClipboardFormat) -> std::io::Result<()> {
    // Normalize the input text for comparison
    let normalized_text = normalize_for_comparison(text);
    
    // First set our content
    copy_to_clipboard_with_format(text, format)?;
    debug!("Initial clipboard set with our processed content");
    
    // Give superwhisper some time to potentially change the clipboard
    thread::sleep(Duration::from_millis(200));
    
    // Check if the clipboard changed
    match get_clipboard_content() {
        Ok(current_content) => {
            let normalized_current = normalize_for_comparison(&current_content);
            
            // If the clipboard content is different from what we set, it likely means
            // superwhisper changed it, so we set our content again
            if normalized_current != normalized_text {
                debug!("Detected clipboard change (likely from superwhisper). Setting our content again.");
                copy_to_clipboard_with_format(text, format)?;
                
                // Add one more check after a short delay to catch any potential follow-up changes
                thread::sleep(Duration::from_millis(100));
                if let Ok(latest_content) = get_clipboard_content() {
                    let normalized_latest = normalize_for_comparison(&latest_content);
                    if normalized_latest != normalized_text {
                        debug!("Clipboard changed again. Final set of our content.");
                        copy_to_clipboard_with_format(text, format)?;
                    }
                }
            } else {
                debug!("Clipboard content unchanged - our content is already in clipboard");
            }
        },
        Err(e) => {
            // If we can't read the clipboard, log the error and set our content again
            warn!("Failed to read clipboard: {}. Setting our content again.", e);
            copy_to_clipboard_with_format(text, format)?;
        }
    }
    
    Ok(())
}

/// Helper to display a truncated string preview
pub fn truncate(s: &str, max_chars: usize) -> String {
    if s.len() <= max_chars {
        s.to_string()
    } else {
        format!("{}...", &s[..max_chars])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_clipboard_format() {
        assert_eq!(
            parse_clipboard_format("plaintext"),
            ClipboardFormat::PlainText
        );
        assert_eq!(
            parse_clipboard_format("richtext"),
            ClipboardFormat::RichText
        );
        assert_eq!(
            parse_clipboard_format("markdown"),
            ClipboardFormat::Markdown
        );
        assert_eq!(
            parse_clipboard_format("unknown"),
            ClipboardFormat::PlainText
        );
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("This is a long string", 7), "This is...");
    }
    
    #[test]
    fn test_normalize_for_comparison() {
        assert_eq!(normalize_for_comparison("  test  "), "test");
        assert_eq!(normalize_for_comparison("test\r\n"), "test");
        assert_eq!(normalize_for_comparison("test\r"), "test");
        assert_eq!(normalize_for_comparison("test\n"), "test");
    }
}
