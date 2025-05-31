use regex::Regex;
use anyhow::{Result, bail};
use std::collections::HashSet;

pub struct InputValidator {
    max_text_length: usize,
    slack_user_id_regex: Regex,
    slack_team_id_regex: Regex,
    slack_channel_id_regex: Regex,
    allowed_commands: HashSet<String>,
}

impl Default for InputValidator {
    fn default() -> Self {
        let mut allowed_commands = HashSet::new();
        allowed_commands.insert("/meet".to_string());
        allowed_commands.insert("/meet-auth".to_string());
        allowed_commands.insert("/meet-help".to_string());

        Self {
            max_text_length: 2000,
            slack_user_id_regex: Regex::new(r"^U[A-Z0-9]{8,10}$").unwrap(),
            slack_team_id_regex: Regex::new(r"^T[A-Z0-9]{8,10}$").unwrap(),
            slack_channel_id_regex: Regex::new(r"^[CDG][A-Z0-9]{8,10}$").unwrap(),
            allowed_commands,
        }
    }
}

impl InputValidator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn validate_slack_user_id(&self, user_id: &str) -> Result<()> {
        if user_id.is_empty() {
            bail!("User ID cannot be empty");
        }
        
        if !self.slack_user_id_regex.is_match(user_id) {
            bail!("Invalid Slack user ID format: {}", user_id);
        }
        
        Ok(())
    }

    pub fn validate_slack_team_id(&self, team_id: &str) -> Result<()> {
        if team_id.is_empty() {
            bail!("Team ID cannot be empty");
        }
        
        if !self.slack_team_id_regex.is_match(team_id) {
            bail!("Invalid Slack team ID format: {}", team_id);
        }
        
        Ok(())
    }

    pub fn validate_slack_channel_id(&self, channel_id: &str) -> Result<()> {
        if channel_id.is_empty() {
            bail!("Channel ID cannot be empty");
        }
        
        if !self.slack_channel_id_regex.is_match(channel_id) {
            bail!("Invalid Slack channel ID format: {}", channel_id);
        }
        
        Ok(())
    }

    pub fn validate_text_input(&self, text: &str, field_name: &str) -> Result<String> {
        if text.len() > self.max_text_length {
            bail!("{} exceeds maximum length of {} characters", field_name, self.max_text_length);
        }

        let sanitized = text
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || " .,!?-_@#$%^&*()[]{}+=:;\"'<>/\\|`~".contains(*c))
            .collect::<String>();

        let lowercase = sanitized.to_lowercase();
        let dangerous_patterns = [
            "javascript:", "data:", "vbscript:", "<script", "</script",
            "onload=", "onerror=", "onclick=", "onmouseover=",
            "eval(", "document.cookie", "window.location",
            "alert(", "confirm(", "prompt(",
            "document.write", "innerHTML", "outerHTML"
        ];

        for pattern in &dangerous_patterns {
            if lowercase.contains(pattern) {
                bail!("{} contains potentially dangerous content: {}", field_name, pattern);
            }
        }

        Ok(sanitized)
    }

    pub fn validate_slack_command(&self, command: &str) -> Result<()> {
        if command.is_empty() {
            bail!("Command cannot be empty");
        }

        if !self.allowed_commands.contains(command) {
            bail!("Unknown command: {}", command);
        }

        Ok(())
    }

    pub fn validate_oauth_state(&self, state: &str) -> Result<()> {
        if state.is_empty() {
            bail!("OAuth state cannot be empty");
        }

        if state.len() < 32 {
            bail!("OAuth state too short (minimum 32 characters)");
        }

        if state.len() > 128 {
            bail!("OAuth state too long (maximum 128 characters)");
        }

        let valid_chars = state.chars().all(|c| {
            c.is_ascii_alphanumeric() || c == '-' || c == '_'
        });

        if !valid_chars {
            bail!("OAuth state contains invalid characters");
        }

        Ok(())
    }

    pub fn validate_oauth_code(&self, code: &str) -> Result<()> {
        if code.is_empty() {
            bail!("OAuth code cannot be empty");
        }

        if code.len() < 10 || code.len() > 200 {
            bail!("OAuth code has invalid length");
        }

        let valid_chars = code.chars().all(|c| {
            c.is_ascii_alphanumeric() || "._-/".contains(c)
        });

        if !valid_chars {
            bail!("OAuth code contains invalid characters");
        }

        Ok(())
    }

    pub fn validate_meeting_title(&self, title: &str) -> Result<String> {
        if title.is_empty() {
            bail!("Meeting title cannot be empty");
        }

        if title.len() > 200 {
            bail!("Meeting title too long (maximum 200 characters)");
        }

        self.validate_text_input(title, "Meeting title")
    }

    pub fn validate_url(&self, url: &str) -> Result<()> {
        if url.is_empty() {
            bail!("URL cannot be empty");
        }

        if !url.starts_with("https://") {
            bail!("URL must use HTTPS");
        }

        if url.contains("slack.com") && !url.starts_with("https://hooks.slack.com/") {
            bail!("Invalid Slack URL domain");
        }

        if url.len() > 2048 {
            bail!("URL too long");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_slack_user_id() {
        let validator = InputValidator::new();
        
        // Valid user IDs
        assert!(validator.validate_slack_user_id("U1234567890").is_ok());
        assert!(validator.validate_slack_user_id("UABCDEFGHIJ").is_ok());
        
        // Invalid user IDs
        assert!(validator.validate_slack_user_id("").is_err());
        assert!(validator.validate_slack_user_id("invalid").is_err());
        assert!(validator.validate_slack_user_id("T1234567890").is_err()); // Team ID format
    }

    #[test]
    fn test_validate_text_input() {
        let validator = InputValidator::new();
        
        // Valid input
        assert!(validator.validate_text_input("Hello world!", "test").is_ok());
        
        // XSS attempts
        assert!(validator.validate_text_input("<script>alert('xss')</script>", "test").is_err());
        assert!(validator.validate_text_input("javascript:alert(1)", "test").is_err());
        
        // Too long
        let long_text = "a".repeat(3000);
        assert!(validator.validate_text_input(&long_text, "test").is_err());
    }

    #[test]
    fn test_validate_oauth_state() {
        let validator = InputValidator::new();
        
        // Valid state
        assert!(validator.validate_oauth_state("abcdef1234567890abcdef1234567890ab").is_ok());
        
        // Too short
        assert!(validator.validate_oauth_state("short").is_err());
        
        // Invalid characters
        assert!(validator.validate_oauth_state("invalid+chars/here=").is_err());
    }
}
