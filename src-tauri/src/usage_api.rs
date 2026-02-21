use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum UsageError {
    #[error("HTTP request failed: {0}")]
    RequestError(String),
    #[error("Unauthorized - run `claude` to refresh your session")]
    Unauthorized,
    #[error("Failed to parse response: {0}")]
    ParseError(String),
}

#[derive(Debug, Deserialize)]
struct UsageBucket {
    utilization: Option<f64>,
    resets_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiResponse {
    five_hour: Option<UsageBucket>,
    seven_day: Option<UsageBucket>,
    seven_day_opus: Option<UsageBucket>,
    seven_day_sonnet: Option<UsageBucket>,
    extra_usage: Option<ExtraUsage>,
    tier: Option<String>,
    plan: Option<String>,
    membership: Option<Membership>,
}

#[derive(Debug, Deserialize)]
struct Membership {
    tier: Option<String>,
    plan_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExtraUsage {
    is_enabled: Option<bool>,
    monthly_limit: Option<f64>,
    used_credits: Option<f64>,
    utilization: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageLimit {
    pub label: String,
    pub usage_pct: f64,
    pub reset_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExtraUsageInfo {
    pub enabled: bool,
    pub monthly_limit: f64,
    pub used_credits: f64,
    pub utilization: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsageData {
    pub limits: Vec<UsageLimit>,
    pub extra_usage: Option<ExtraUsageInfo>,
    pub plan: Option<String>,
}

pub async fn fetch_usage(token: &str) -> Result<UsageData, UsageError> {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.anthropic.com/api/oauth/usage")
        .bearer_auth(token)
        .header("anthropic-beta", "oauth-2025-04-20")
        .send()
        .await
        .map_err(|e| UsageError::RequestError(e.to_string()))?;

    if resp.status() == 401 || resp.status() == 403 {
        return Err(UsageError::Unauthorized);
    }

    if !resp.status().is_success() {
        return Err(UsageError::RequestError(format!("HTTP {}", resp.status())));
    }

    let raw: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| UsageError::ParseError(e.to_string()))?;

    // Log raw API keys for debugging plan detection
    if let Some(obj) = raw.as_object() {
        let keys: Vec<&String> = obj.keys().collect();
        crate::log(&format!("usage API keys: {:?}", keys));
    }

    let body: ApiResponse = serde_json::from_value(raw)
        .map_err(|e| UsageError::ParseError(e.to_string()))?;

    let mut limits = Vec::new();

    fn push_bucket(limits: &mut Vec<UsageLimit>, bucket: &Option<UsageBucket>, label: &str) {
        if let Some(b) = bucket {
            if let Some(util) = b.utilization {
                limits.push(UsageLimit {
                    label: label.into(),
                    usage_pct: util / 100.0,
                    reset_at: b.resets_at.clone(),
                });
            }
        }
    }

    push_bucket(&mut limits, &body.five_hour, "Current session");
    push_bucket(&mut limits, &body.seven_day, "Current week (all models)");
    push_bucket(&mut limits, &body.seven_day_sonnet, "Current week (Sonnet only)");
    push_bucket(&mut limits, &body.seven_day_opus, "Current week (Opus only)");

    let extra_usage = body.extra_usage.and_then(|eu| {
        if eu.is_enabled.unwrap_or(false) {
            Some(ExtraUsageInfo {
                enabled: true,
                monthly_limit: eu.monthly_limit.unwrap_or(0.0) / 100.0,
                used_credits: eu.used_credits.unwrap_or(0.0) / 100.0,
                utilization: eu.utilization.unwrap_or(0.0) / 100.0,
            })
        } else {
            None
        }
    });

    // Try to detect plan from response
    let plan = body.plan
        .or(body.tier)
        .or(body.membership.as_ref().and_then(|m| m.plan_name.clone()))
        .or(body.membership.as_ref().and_then(|m| m.tier.clone()));

    Ok(UsageData { limits, extra_usage, plan })
}
