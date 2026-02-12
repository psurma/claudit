use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum KeychainError {
    #[error("Keychain entry not found. Run `claude` first to authenticate.")]
    NotFound,
    #[error("Failed to parse keychain data: {0}")]
    ParseError(String),
    #[error("Command failed: {0}")]
    CommandError(String),
}

pub fn get_oauth_token() -> Result<String, KeychainError> {
    let output = Command::new("security")
        .args(["find-generic-password", "-s", "Claude Code-credentials", "-w"])
        .output()
        .map_err(|e| KeychainError::CommandError(e.to_string()))?;

    if !output.status.success() {
        return Err(KeychainError::NotFound);
    }

    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();

    let creds: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| KeychainError::ParseError(e.to_string()))?;

    let token = creds
        .get("claudeAiOauth")
        .and_then(|v| v.get("accessToken"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| KeychainError::ParseError("Missing claudeAiOauth.accessToken".into()))?;

    Ok(token.to_string())
}
