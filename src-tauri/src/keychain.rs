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
    let raw = get_raw_credentials()?;
    parse_oauth_token(&raw)
}

#[cfg(target_os = "macos")]
fn get_raw_credentials() -> Result<String, KeychainError> {
    let output = std::process::Command::new("security")
        .args(["find-generic-password", "-s", "Claude Code-credentials", "-w"])
        .output()
        .map_err(|e| KeychainError::CommandError(e.to_string()))?;

    if !output.status.success() {
        return Err(KeychainError::NotFound);
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(not(target_os = "macos"))]
fn get_raw_credentials() -> Result<String, KeychainError> {
    let entry = keyring::Entry::new("Claude Code-credentials", "default")
        .map_err(|e| KeychainError::CommandError(e.to_string()))?;

    entry
        .get_password()
        .map_err(|e| match e {
            keyring::Error::NoEntry => KeychainError::NotFound,
            _ => KeychainError::CommandError(e.to_string()),
        })
}

fn parse_oauth_token(raw: &str) -> Result<String, KeychainError> {
    let creds: serde_json::Value = serde_json::from_str(raw)
        .map_err(|e| KeychainError::ParseError(e.to_string()))?;

    let token = creds
        .get("claudeAiOauth")
        .and_then(|v| v.get("accessToken"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| KeychainError::ParseError("Missing claudeAiOauth.accessToken".into()))?;

    Ok(token.to_string())
}
