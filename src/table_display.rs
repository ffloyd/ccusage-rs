//! # Table Display Module
//!
//! Provides daily usage statistics table display similar to ccusage
//!
//! ## Key Components
//! - [`DailyStats`] - Daily aggregated statistics
//! - [`format_table`] - Main table formatting function
//! - [`format_number_compact`] - Compact number formatting for table cells

use anyhow::Result;
use chrono::Datelike;
use serde::Serialize;

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

#[derive(Debug, Default, Serialize, Clone)]
pub struct ModelBreakdown {
    pub model_name: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub total_tokens: u64,
    pub cost_usd: f64,
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
    pub model_breakdowns: Vec<ModelBreakdown>,
}



pub fn simplify_model_name(model: &str) -> String {
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


pub fn format_table_with_breakdown(daily_stats: &[DailyStats], breakdown: bool) -> String {
    if breakdown {
        format_breakdown_table(daily_stats)
    } else {
        format_standard_table(daily_stats)
    }
}

fn format_breakdown_table(daily_stats: &[DailyStats]) -> String {
    let mut output = String::new();
    
    // Header
    output.push('\n');
    output.push_str(" ╭──────────────────────────────────────────────────╮\n");
    output.push_str(" │                                                  │\n");
    output.push_str(" │  Claude Code Token Usage - Daily Model Breakdown │\n");
    output.push_str(" │                                                  │\n");
    output.push_str(" ╰──────────────────────────────────────────────────╯\n");
    output.push('\n');

    let gray = "\x1b[90m";
    let reset = "\x1b[39m";
    let cyan = "\x1b[36m";
    let green = "\x1b[32m";

    // Calculate totals
    let mut grand_total_tokens = 0u64;
    let mut grand_total_cost = 0.0;

    for stats in daily_stats {
        output.push_str(&format!("\n{green}📅 {}{reset}\n", stats.date));
        
        if stats.model_breakdowns.is_empty() {
            output.push_str("   No model usage data available\n");
            continue;
        }

        // Table header for this date
        output.push_str(&format!(
            "{gray}┌─────────────{reset}{gray}┬──────────{reset}{gray}┬──────────{reset}{gray}┬──────────{reset}{gray}┬──────────{reset}{gray}┬──────────{reset}{gray}┬──────────┐{reset}\n"
        ));
        
        output.push_str(&format!(
            "{gray}│{reset}{cyan} Model       {reset}{gray}│{reset}{cyan}    Input {reset}{gray}│{reset}{cyan}   Output {reset}{gray}│{reset}{cyan}    Cache {reset}{gray}│{reset}{cyan}     Read {reset}{gray}│{reset}{cyan}    Total {reset}{gray}│{reset}{cyan}     Cost {reset}{gray}│{reset}\n"
        ));
        
        output.push_str(&format!(
            "{gray}│{reset}{cyan}             {reset}{gray}│{reset}{cyan}          {reset}{gray}│{reset}{cyan}          {reset}{gray}│{reset}{cyan}   Create {reset}{gray}│{reset}{cyan}          {reset}{gray}│{reset}{cyan}   Tokens {reset}{gray}│{reset}{cyan}    (USD) {reset}{gray}│{reset}\n"
        ));

        // Data rows for each model
        for breakdown in &stats.model_breakdowns {
            output.push_str(&format!(
                "{gray}├─────────────{reset}{gray}┼──────────{reset}{gray}┼──────────{reset}{gray}┼──────────{reset}{gray}┼──────────{reset}{gray}┼──────────{reset}{gray}┼──────────┤{reset}\n"
            ));

            output.push_str(&format!(
                "{gray}│{reset} {:<11} {gray}│{reset} {:>8} {gray}│{reset} {:>8} {gray}│{reset} {:>8} {gray}│{reset} {:>8} {gray}│{reset} {:>8} {gray}│{reset} {:>8} {gray}│{reset}\n",
                breakdown.model_name,
                format_number_compact(breakdown.input_tokens),
                format_number_compact(breakdown.output_tokens),
                format_number_compact(breakdown.cache_creation_tokens),
                format_number_compact(breakdown.cache_read_tokens),
                format_number_compact(breakdown.total_tokens),
                format!("${:.2}", breakdown.cost_usd)
            ));
        }

        // Totals row for this date
        output.push_str(&format!(
            "{gray}├─────────────{reset}{gray}┼──────────{reset}{gray}┼──────────{reset}{gray}┼──────────{reset}{gray}┼──────────{reset}{gray}┼──────────{reset}{gray}┼──────────┤{reset}\n"
        ));

        output.push_str(&format!(
            "{gray}│{reset} {green}Total{reset}       {gray}│{reset} {:>8} {gray}│{reset} {:>8} {gray}│{reset} {:>8} {gray}│{reset} {:>8} {gray}│{reset} {:>8} {gray}│{reset} {:>8} {gray}│{reset}\n",
            format_number_compact(stats.input_tokens),
            format_number_compact(stats.output_tokens),
            format_number_compact(stats.cache_creation_tokens),
            format_number_compact(stats.cache_read_tokens),
            format_number_compact(stats.total_tokens),
            format!("${:.2}", stats.cost_usd)
        ));

        output.push_str(&format!(
            "{gray}└─────────────{reset}{gray}┴──────────{reset}{gray}┴──────────{reset}{gray}┴──────────{reset}{gray}┴──────────{reset}{gray}┴──────────{reset}{gray}┴──────────┘{reset}\n"
        ));

        grand_total_tokens += stats.total_tokens;
        grand_total_cost += stats.cost_usd;
    }

    // Grand totals
    output.push_str(&format!("\n{green}📊 Grand Total: {} tokens | ${:.2}{reset}\n", 
        format_number_compact(grand_total_tokens), grand_total_cost));

    output
}

fn format_standard_table(daily_stats: &[DailyStats]) -> String {
    let mut output = String::new();
    
    // Header
    output.push('\n');
    output.push_str(" ╭──────────────────────────────────────────╮\n");
    output.push_str(" │                                          │\n");
    output.push_str(" │  Claude Code Token Usage Report - Daily  │\n");
    output.push_str(" │                                          │\n");
    output.push_str(" ╰──────────────────────────────────────────╯\n");
    output.push('\n');

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
    for stats in daily_stats.iter() {
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