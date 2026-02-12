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

    let body: ApiResponse = resp
        .json()
        .await
        .map_err(|e| UsageError::ParseError(e.to_string()))?;

    let mut limits = Vec::new();

    if let Some(bucket) = &body.five_hour {
        if let Some(util) = bucket.utilization {
            limits.push(UsageLimit {
                label: "Session (5hr rolling)".into(),
                usage_pct: util / 100.0,
                reset_at: bucket.resets_at.clone(),
            });
        }
    }

    if let Some(bucket) = &body.seven_day {
        if let Some(util) = bucket.utilization {
            limits.push(UsageLimit {
                label: "Weekly All Models".into(),
                usage_pct: util / 100.0,
                reset_at: bucket.resets_at.clone(),
            });
        }
    }

    if let Some(bucket) = &body.seven_day_sonnet {
        if let Some(util) = bucket.utilization {
            limits.push(UsageLimit {
                label: "Weekly Sonnet".into(),
                usage_pct: util / 100.0,
                reset_at: bucket.resets_at.clone(),
            });
        }
    }

    if let Some(bucket) = &body.seven_day_opus {
        if let Some(util) = bucket.utilization {
            limits.push(UsageLimit {
                label: "Weekly Opus".into(),
                usage_pct: util / 100.0,
                reset_at: bucket.resets_at.clone(),
            });
        }
    }

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

    Ok(UsageData { limits, extra_usage })
}
