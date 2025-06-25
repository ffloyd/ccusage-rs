//! # Table Display Module
//!
//! Provides daily usage statistics table display similar to ccusage
//!
//! ## Key Components
//! - [`DailyStats`] - Daily aggregated statistics
//! - [`format_table`] - Main table formatting function
//! - [`format_number_compact`] - Compact number formatting for table cells

use anyhow::Result;
use chrono::{DateTime, Utc, Datelike};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::jsonl_parser::SessionData;
use crate::pricing::calculate_session_cost;

/// JSON structures matching ccusage format exactly
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonModelBreakdown {
    pub model_name: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub cost: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonDailyEntry {
    pub date: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub total_tokens: u64,
    pub total_cost: f64,
    pub models_used: Vec<String>,
    pub model_breakdowns: Vec<JsonModelBreakdown>,
}

#[derive(Debug, Serialize)]
pub struct JsonOutput {
    pub daily: Vec<JsonDailyEntry>,
}

#[derive(Debug, Default, Serialize)]
pub struct DailyStats {
    pub date: String,
    pub models: Vec<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub total_tokens: u64,
    pub cost_usd: f64,
}

impl DailyStats {
    fn new(date: String) -> Self {
        Self {
            date,
            models: Vec::new(),
            input_tokens: 0,
            output_tokens: 0,
            cache_creation_tokens: 0,
            cache_read_tokens: 0,
            total_tokens: 0,
            cost_usd: 0.0,
        }
    }

    fn add_session(&mut self, session: &SessionData) {
        for (model_name, usage) in &session.model_usage {
            // Add model to list if not already present
            let simplified_model = simplify_model_name(model_name);
            if !self.models.contains(&simplified_model) {
                self.models.push(simplified_model);
            }

            // Track token aggregation for table display

            // Add token counts
            self.input_tokens += usage.total_input;
            self.output_tokens += usage.total_output;
            self.cache_creation_tokens += usage.total_cache_write;
            self.cache_read_tokens += usage.total_cache_read;
        }

        // Update totals
        self.total_tokens = self.input_tokens + self.output_tokens + self.cache_creation_tokens + self.cache_read_tokens;
        let session_cost = calculate_session_cost(&session.model_usage);
        self.cost_usd += session_cost;
    }
}

pub fn aggregate_daily_stats(sessions: &[SessionData]) -> Result<Vec<DailyStats>> {
    let mut daily_map: HashMap<String, DailyStats> = HashMap::new();

    // Debug: Track what dates we're processing
    if std::env::var("CC_USAGE_DETAILED_DEBUG").is_ok() {
        println!("DEBUG: Processing {} total sessions", sessions.len());
    }

    for session in sessions {
        let date_key = session.start_time.format("%Y-%m-%d").to_string();
        
        // Debug: Show date processing
        if std::env::var("CC_USAGE_DETAILED_DEBUG").is_ok() {
            if !daily_map.contains_key(&date_key) {
                println!("DEBUG: First session for date {}: session {}", date_key, session.session_id);
            }
        }
        
        let daily_stat = daily_map.entry(date_key.clone())
            .or_insert_with(|| DailyStats::new(date_key));
        
        daily_stat.add_session(session);
    }

    // Convert to sorted vector
    let mut daily_stats: Vec<DailyStats> = daily_map.into_values().collect();
    daily_stats.sort_by(|a, b| a.date.cmp(&b.date));

    Ok(daily_stats)
}

fn simplify_model_name(model: &str) -> String {
    if model.contains("opus") {
        "opus-4".to_string()
    } else if model.contains("sonnet") {
        "sonnet-4".to_string()
    } else if model.contains("haiku") {
        "haiku".to_string()
    } else {
        // Extract first two parts separated by dashes
        model.split('-').take(2).collect::<Vec<_>>().join("-")
    }
}

fn format_number_compact(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.0}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn format_models_list(models: &[String]) -> String {
    let mut result = String::new();
    for (i, model) in models.iter().enumerate() {
        if i > 0 {
            result.push('\n');
        }
        result.push_str(&format!("- {}", model));
    }
    result
}

pub fn format_table(daily_stats: &[DailyStats]) -> String {
    let mut output = String::new();
    
    // Header
    output.push_str("\n");
    output.push_str(" ╭──────────────────────────────────────────╮\n");
    output.push_str(" │                                          │\n");
    output.push_str(" │  Claude Code Token Usage Report - Daily  │\n");
    output.push_str(" │                                          │\n");
    output.push_str(" ╰──────────────────────────────────────────╯\n");
    output.push_str("\n");

    // Table header
    output.push_str(&format!(
        "{gray}┌──────────{reset}{gray}┬───────────────────────────────{reset}{gray}┬──────────{reset}{gray}┬──────────{reset}{gray}┬──────────{reset}{gray}┬──────────{reset}{gray}┬──────────{reset}{gray}┬──────────┐{reset}\n",
        gray = "\x1b[90m", reset = "\x1b[39m"
    ));
    
    output.push_str(&format!(
        "{gray}│{reset}{cyan} Date     {reset}{gray}│{reset}{cyan} Models                        {reset}{gray}│{reset}{cyan}    Input {reset}{gray}│{reset}{cyan}   Output {reset}{gray}│{reset}{cyan}    Cache {reset}{gray}│{reset}{cyan}    Cache {reset}{gray}│{reset}{cyan}    Total {reset}{gray}│{reset}{cyan}     Cost {reset}{gray}│{reset}\n",
        gray = "\x1b[90m", reset = "\x1b[39m", cyan = "\x1b[36m"
    ));
    
    output.push_str(&format!(
        "{gray}│{reset}{cyan}          {reset}{gray}│{reset}{cyan}                               {reset}{gray}│{reset}{cyan}          {reset}{gray}│{reset}{cyan}          {reset}{gray}│{reset}{cyan}   Create {reset}{gray}│{reset}{cyan}     Read {reset}{gray}│{reset}{cyan}   Tokens {reset}{gray}│{reset}{cyan}    (USD) {reset}{gray}│{reset}\n",
        gray = "\x1b[90m", reset = "\x1b[39m", cyan = "\x1b[36m"
    ));

    // Calculate totals
    let mut total_input = 0u64;
    let mut total_output = 0u64;
    let mut total_cache_create = 0u64;
    let mut total_cache_read = 0u64;
    let mut total_tokens = 0u64;
    let mut total_cost = 0.0;

    // Data rows
    for (i, stats) in daily_stats.iter().enumerate() {
        // Add separator
        output.push_str(&format!(
            "{gray}├──────────{reset}{gray}┼───────────────────────────────{reset}{gray}┼──────────{reset}{gray}┼──────────{reset}{gray}┼──────────{reset}{gray}┼──────────{reset}{gray}┼──────────{reset}{gray}┼──────────┤{reset}\n",
            gray = "\x1b[90m", reset = "\x1b[39m"
        ));

        // Format date (MM-DD format)
        let formatted_date = if let Ok(parsed_date) = chrono::NaiveDate::parse_from_str(&stats.date, "%Y-%m-%d") {
            format!("{:04}\n {:02}-{:02}", parsed_date.year(), parsed_date.month(), parsed_date.day())
        } else {
            stats.date.clone()
        };

        // Format models with proper spacing for multi-line
        let models_text = format_models_list(&stats.models);
        let model_lines: Vec<&str> = models_text.lines().collect();
        
        // First row with date and first model
        let first_model = model_lines.first().unwrap_or(&"");
        output.push_str(&format!(
            "{gray}│{reset} {:<8} {gray}│{reset} {:<29} {gray}│{reset} {:>8} {gray}│{reset} {:>8} {gray}│{reset} {:>8} {gray}│{reset} {:>8} {gray}│{reset} {:>8} {gray}│{reset} {:>8} {gray}│{reset}\n",
            formatted_date.lines().next().unwrap_or(&stats.date),
            first_model,
            format_number_compact(stats.input_tokens),
            format_number_compact(stats.output_tokens),
            format_number_compact(stats.cache_creation_tokens),
            format_number_compact(stats.cache_read_tokens),
            format_number_compact(stats.total_tokens),
            format!("${:.2}", stats.cost_usd),
            gray = "\x1b[90m", reset = "\x1b[39m"
        ));

        // Additional rows for remaining models and date continuation
        let date_lines: Vec<&str> = formatted_date.lines().collect();
        let max_lines = std::cmp::max(model_lines.len(), date_lines.len());
        
        for line_idx in 1..max_lines {
            let date_part = date_lines.get(line_idx).unwrap_or(&"");
            let model_part = model_lines.get(line_idx).unwrap_or(&"");
            
            output.push_str(&format!(
                "{gray}│{reset} {:<8} {gray}│{reset} {:<29} {gray}│{reset}          {gray}│{reset}          {gray}│{reset}          {gray}│{reset}          {gray}│{reset}          {gray}│{reset}          {gray}│{reset}\n",
                date_part,
                model_part,
                gray = "\x1b[90m", reset = "\x1b[39m"
            ));
        }

        // Add to totals
        total_input += stats.input_tokens;
        total_output += stats.output_tokens;
        total_cache_create += stats.cache_creation_tokens;
        total_cache_read += stats.cache_read_tokens;
        total_tokens += stats.total_tokens;
        total_cost += stats.cost_usd;
    }

    // Totals row
    output.push_str(&format!(
        "{gray}├──────────{reset}{gray}┼───────────────────────────────{reset}{gray}┼──────────{reset}{gray}┼──────────{reset}{gray}┼──────────{reset}{gray}┼──────────{reset}{gray}┼──────────{reset}{gray}┼──────────┤{reset}\n",
        gray = "\x1b[90m", reset = "\x1b[39m"
    ));

    output.push_str(&format!(
        "{gray}│{reset} Total    {gray}│{reset}                               {gray}│{reset} {:>8} {gray}│{reset} {:>8} {gray}│{reset} {:>8} {gray}│{reset} {:>8} {gray}│{reset} {:>8} {gray}│{reset} {:>8} {gray}│{reset}\n",
        format_number_compact(total_input),
        format_number_compact(total_output),
        format_number_compact(total_cache_create),
        format_number_compact(total_cache_read),
        format_number_compact(total_tokens),
        format!("${:.2}", total_cost),
        gray = "\x1b[90m", reset = "\x1b[39m"
    ));

    // Table footer
    output.push_str(&format!(
        "{gray}└──────────{reset}{gray}┴───────────────────────────────{reset}{gray}┴──────────{reset}{gray}┴──────────{reset}{gray}┴──────────{reset}{gray}┴──────────{reset}{gray}┴──────────{reset}{gray}┴──────────┘{reset}\n",
        gray = "\x1b[90m", reset = "\x1b[39m"
    ));

    output
}

/// Convert daily stats to JSON format matching ccusage
pub fn generate_json_output(daily_stats: &[DailyStats]) -> Result<JsonOutput> {
    let mut json_daily = Vec::new();
    
    for stats in daily_stats {
        // Create model breakdowns from the models list
        let mut model_breakdowns = Vec::new();
        
        // For now, we'll aggregate all tokens under the primary model
        // This is a simplification - ideally we'd track per-model usage separately
        if !stats.models.is_empty() {
            let primary_model = &stats.models[0];
            model_breakdowns.push(JsonModelBreakdown {
                model_name: primary_model.clone(),
                input_tokens: stats.input_tokens,
                output_tokens: stats.output_tokens,
                cache_creation_tokens: stats.cache_creation_tokens,
                cache_read_tokens: stats.cache_read_tokens,
                cost: stats.cost_usd,
            });
        }
        
        json_daily.push(JsonDailyEntry {
            date: stats.date.clone(),
            input_tokens: stats.input_tokens,
            output_tokens: stats.output_tokens,
            cache_creation_tokens: stats.cache_creation_tokens,
            cache_read_tokens: stats.cache_read_tokens,
            total_tokens: stats.total_tokens,
            total_cost: stats.cost_usd,
            models_used: stats.models.clone(),
            model_breakdowns,
        });
    }
    
    Ok(JsonOutput { daily: json_daily })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use crate::jsonl_parser::ModelUsage;

    fn create_test_session(date: &str, model: &str, input: u64, output: u64) -> SessionData {
        let start_time = DateTime::parse_from_rfc3339(&format!("{}T12:00:00Z", date))
            .unwrap()
            .with_timezone(&Utc);

        let mut model_usage = HashMap::new();
        model_usage.insert(model.to_string(), ModelUsage {
            model_name: model.to_string(),
            total_input: input,
            total_output: output,
            total_cache_write: 0,
            total_cache_read: 0,
            message_count: 1,
            weighted_tokens: input + output,
        });

        SessionData {
            session_id: "test".to_string(),
            start_time,
            end_time: Some(start_time),
            model_usage,
            total_weighted_tokens: input + output,
            has_limit_error: false,
            limit_type: None,
        }
    }

    #[test]
    fn test_daily_aggregation() {
        let sessions = vec![
            create_test_session("2025-06-24", "claude-opus-4-20250514", 100, 200),
            create_test_session("2025-06-24", "claude-sonnet-4-20250325", 150, 250),
            create_test_session("2025-06-25", "claude-opus-4-20250514", 300, 400),
        ];

        let daily_stats = aggregate_daily_stats(&sessions).unwrap();
        
        assert_eq!(daily_stats.len(), 2);
        assert_eq!(daily_stats[0].date, "2025-06-24");
        assert_eq!(daily_stats[0].input_tokens, 250); // 100 + 150
        assert_eq!(daily_stats[0].output_tokens, 450); // 200 + 250
        assert_eq!(daily_stats[0].models.len(), 2);
        
        assert_eq!(daily_stats[1].date, "2025-06-25");
        assert_eq!(daily_stats[1].input_tokens, 300);
        assert_eq!(daily_stats[1].output_tokens, 400);
    }

    #[test]
    fn test_model_name_simplification() {
        assert_eq!(simplify_model_name("claude-opus-4-20250514"), "opus-4");
        assert_eq!(simplify_model_name("claude-sonnet-4-20250325"), "sonnet-4");
        assert_eq!(simplify_model_name("claude-3-5-haiku-20241022"), "haiku");
    }

    #[test]
    fn test_number_formatting() {
        assert_eq!(format_number_compact(1234), "1K");
        assert_eq!(format_number_compact(1234567), "1.2M");
        assert_eq!(format_number_compact(999), "999");
    }
}