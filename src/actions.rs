use log::{debug, info};
use std::process::Command;

/// Action type for keyphrases
#[derive(Debug, Clone)]
pub enum ActionType {
    OpenApplication(String),
    OpenUrl(String),
    None,
}

/// Parse a string into an ActionType
pub fn parse_action(action_str: &str) -> ActionType {
    // Check for special action keywords first
    match action_str.to_lowercase().as_str() {
        "" => return ActionType::None,
        _ => {} // Continue with other checks
    }
    
    // URI detection - look for scheme:something or scheme://something pattern
    if action_str.contains(':') {
        // Check if it matches URI pattern with a scheme
        if let Some(scheme_end) = action_str.find(':') {
            let scheme = &action_str[..scheme_end];
            // Validate scheme format (letters, digits, +, -, .)
            if scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
            {
                debug!("Detected URI with scheme: {}", scheme);
                return ActionType::OpenUrl(action_str.to_string());
            }
        }
    }

    // Otherwise assume it's an application
    ActionType::OpenApplication(action_str.to_string())
}

/// Execute an action based on its type
pub fn execute_action(action: &ActionType) -> std::result::Result<(), Box<dyn std::error::Error>> {
    match action {
        ActionType::OpenApplication(app) => open_application(app),
        ActionType::OpenUrl(url) => open_url(url),
        ActionType::None => Ok(()),
    }
}

/// Open an application based on platform
pub fn open_application(app: &str) -> std::result::Result<(), Box<dyn std::error::Error>> {
    info!("Opening application: {}", app);

    // Platform-specific application launching
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(&["/C", "start", "", app])
            .spawn()?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open").args(&["-a", app]).spawn()?;
    }

    #[cfg(target_os = "linux")]
    {
        Command::new(app).spawn()?;
    }

    Ok(())
}

/// Open a URL using the system's default handler
pub fn open_url(url: &str) -> std::result::Result<(), Box<dyn std::error::Error>> {
    info!("Opening URL: {}", url);
    open::that(url)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_action_empty() {
        match parse_action("") {
            ActionType::None => {}
            _ => panic!("Expected None action for empty string"),
        }
    }

    #[test]
    fn test_parse_action_url() {
        match parse_action("https://example.com") {
            ActionType::OpenUrl(url) => {
                assert_eq!(url, "https://example.com");
            }
            _ => panic!("Expected OpenUrl for URL string"),
        }
    }

    #[test]
    fn test_parse_action_app() {
        match parse_action("notepad") {
            ActionType::OpenApplication(app) => {
                assert_eq!(app, "notepad");
            }
            _ => panic!("Expected OpenApplication for app name"),
        }
    }
}