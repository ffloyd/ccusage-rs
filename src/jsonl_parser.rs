//! # JSONL Parser Module
//!
//! Parses Claude session JSONL files to extract token usage data
//!
//! ## Key Components
//! - [`SessionEntry`] - Represents a single JSONL entry
//! - [`parse_session_file`] - Parse a complete session file
//! - [`extract_model_usage`] - Extract model-specific token counts

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::models::calculate_weighted_tokens;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEntry {
    #[serde(default)]
    pub parent_uuid: Option<String>,
    #[serde(default)]
    pub is_sidechain: bool,
    #[serde(default)]
    pub user_type: String,
    #[serde(default)]
    pub cwd: String,
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub version: String,
    #[serde(rename = "type", default)]
    pub entry_type: String,
    pub message: Option<Message>,
    #[serde(default)]
    pub uuid: String,
    pub timestamp: String,
    #[serde(default)]
    pub is_api_error_message: bool,
    #[serde(default)]
    pub request_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub role: String,
    #[serde(rename = "type", default)]
    pub message_type: Option<String>,
    pub usage: Option<Usage>,
    #[serde(default)]
    pub content: Option<serde_json::Value>,
    #[serde(default)]
    pub stop_reason: Option<String>,
    #[serde(default)]
    pub stop_sequence: Option<String>,
    #[serde(rename = "costUSD", default)]
    pub cost_usd: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
    #[serde(default)]
    pub cache_read_input_tokens: u64,
    #[serde(default)]
    pub service_tier: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct ModelUsage {
    pub model_name: String,
    pub total_input: u64,
    pub total_output: u64,
    pub total_cache_write: u64,
    pub total_cache_read: u64,
    pub message_count: u32,
    pub weighted_tokens: u64,
}

impl ModelUsage {
    pub fn add_usage(&mut self, usage: &Usage) {
        self.total_input += usage.input_tokens;
        self.total_output += usage.output_tokens;
        self.total_cache_write += usage.cache_creation_input_tokens;
        self.total_cache_read += usage.cache_read_input_tokens;
        self.message_count += 1;

        let raw_tokens = usage.input_tokens + usage.output_tokens;
        let weighted = calculate_weighted_tokens(&self.model_name, raw_tokens);
        self.weighted_tokens += weighted;
    }

}

#[derive(Debug)]
pub struct SessionData {
    pub session_id: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub model_usage: HashMap<String, ModelUsage>,
    pub total_weighted_tokens: u64,
    pub has_limit_error: bool,
    pub _limit_type: Option<String>, // "opus" or "general"
}

impl SessionData {
    pub fn new(session_id: String, start_time: DateTime<Utc>) -> Self {
        Self {
            session_id,
            start_time,
            end_time: None,
            model_usage: HashMap::new(),
            total_weighted_tokens: 0,
            has_limit_error: false,
            _limit_type: None,
        }
    }

    pub fn add_entry(&mut self, entry: &SessionEntry) -> Result<()> {
        if let Some(message) = &entry.message {
            // Check for limit reached errors
            if entry.is_api_error_message {
                if let Some(content) = &message.content {
                    if let Some(text) = content
                        .as_array()
                        .and_then(|arr| arr.first())
                        .and_then(|v| v.get("text"))
                        .and_then(|t| t.as_str())
                    {
                        if text.contains("Claude AI usage limit reached") {
                            self.has_limit_error = true;
                            // TODO: Parse limit type from error message
                        }
                    }
                }
            }

            // Track token usage by model (matching ccusage filtering exactly)
            if let (Some(model), Some(usage)) = (&message.model, &message.usage) {
                // Only filter synthetic models (matching ccusage aggregateByModel behavior)
                if model != "<synthetic>" {
                    let model_usage =
                        self.model_usage
                            .entry(model.clone())
                            .or_insert_with(|| ModelUsage {
                                model_name: model.clone(),
                                ..Default::default()
                            });
                    model_usage.add_usage(usage);
                }
            }
        }

        // Update session end time
        let timestamp = DateTime::parse_from_rfc3339(&entry.timestamp)
            .context("Failed to parse timestamp")?
            .with_timezone(&Utc);

        if self.end_time.is_none() || timestamp > self.end_time.unwrap() {
            self.end_time = Some(timestamp);
        }

        Ok(())
    }

    pub fn calculate_totals(&mut self) {
        self.total_weighted_tokens = self
            .model_usage
            .values()
            .map(|usage| usage.weighted_tokens)
            .sum();
    }
}





pub fn parse_session_file(path: &Path) -> Result<SessionData> {
    let file = File::open(path).context("Failed to open JSONL file")?;
    let reader = BufReader::new(file);

    let mut session_data: Option<SessionData> = None;

    for line in reader.lines() {
        let line = line.context("Failed to read line")?;
        if line.trim().is_empty() {
            continue;
        }

        // Check if this is a summary entry (skip it)
        if line.contains("\"type\":\"summary\"") {
            continue;
        }

        // Try to parse as SessionEntry
        match serde_json::from_str::<SessionEntry>(&line) {
            Ok(entry) => {
                // Initialize session data on first valid entry
                if session_data.is_none() {
                    if let Ok(timestamp) = DateTime::parse_from_rfc3339(&entry.timestamp) {
                        session_data = Some(SessionData::new(
                            entry.session_id.clone(),
                            timestamp.with_timezone(&Utc),
                        ));
                    }
                }

                if let Some(ref mut data) = session_data {
                    let _ = data.add_entry(&entry); // Ignore individual entry errors
                }
            }
            Err(_) => {
                // Skip entries that don't match our expected format
                continue;
            }
        }
    }

    if let Some(mut data) = session_data {
        data.calculate_totals();
        Ok(data)
    } else {
        anyhow::bail!("No valid session entries found in JSONL file")
    }
}

pub fn find_session_files(
    project_dir: &Path,
    since: Option<DateTime<Utc>>,
) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    if !project_dir.exists() {
        return Ok(files);
    }

    for entry in std::fs::read_dir(project_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
            if let Some(since_time) = since {
                let metadata = entry.metadata()?;
                let modified = metadata.modified()?;
                let modified_time = DateTime::<Utc>::from(modified);

                if modified_time < since_time {
                    continue;
                }
            }

            files.push(path);
        }
    }

    files.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());

    Ok(files)
}


pub fn get_all_project_dirs(_cwd: &Path) -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    let claude_dir = home.join(".claude").join("projects");

    if !claude_dir.exists() {
        return Vec::new();
    }

    let mut found_dirs = Vec::new();

    // Return ALL Claude project directories (matching ccusage behavior)
    if let Ok(entries) = std::fs::read_dir(&claude_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                found_dirs.push(path);
            }
        }
    }

    // Sort for consistent ordering
    found_dirs.sort();
    found_dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_usage_calculation() {
        let mut usage = ModelUsage {
            model_name: "claude-opus-4-20250514".to_string(),
            ..Default::default()
        };

        usage.add_usage(&Usage {
            input_tokens: 100,
            output_tokens: 200,
            cache_creation_input_tokens: 50,
            cache_read_input_tokens: 25,
            service_tier: None,
        });

        assert_eq!(usage.total_input + usage.total_output, 300);
        assert_eq!(usage.weighted_tokens, 1500); // 300 * 5.0 multiplier
    }
}
