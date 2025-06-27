//! # Commands Module
//!
//! Command handlers for daily, monthly, session, and monitor operations
//!
//! ## Key Components
//! - [`handle_daily_command`] - Process daily usage reports
//! - [`handle_monthly_command`] - Process monthly usage aggregates
//! - [`handle_session_command`] - Process individual session reports
//! - [`handle_monitor_command`] - Real-time monitoring functionality

use anyhow::{Context, Result};

use crate::cli::{SortOrder};
use crate::data_processing::{
    filter_daily_stats_by_date, sort_daily_stats, aggregate_monthly_stats, sort_monthly_stats,
    filter_sessions_by_date, sort_sessions, apply_recent_filter_daily, apply_recent_filter_sessions, MonthlyStats
};
use crate::table_display::{format_table_with_breakdown, generate_json_output};

/// Helper function to format numbers with thousands separators
fn format_number(n: u64) -> String {
    let mut result = String::new();
    let s = n.to_string();
    let chars: Vec<char> = s.chars().collect();
    
    for (i, c) in chars.iter().enumerate() {
        if i > 0 && (chars.len() - i) % 3 == 0 {
            result.push(',');
        }
        result.push(*c);
    }
    
    result
}
use crate::{entry_processor, jsonl_parser};

/// Handle daily usage reports command
pub fn handle_daily_command(
    since: Option<&str>,
    until: Option<&str>,
    order: SortOrder,
    json: bool,
    breakdown: bool,
    recent: Option<usize>,
) -> Result<()> {
    // Get current working directory for project lookup
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let project_dirs = jsonl_parser::get_all_project_dirs(&cwd);

    if project_dirs.is_empty() {
        anyhow::bail!(
            "No Claude session data found. Make sure you're in a project directory that has been used with Claude Code."
        );
    }

    // Find all JSONL session files from all project directories
    let mut session_files = Vec::new();
    for project_dir in &project_dirs {
        let files = jsonl_parser::find_session_files(project_dir, None)
            .context("Failed to find session files")?;
        session_files.extend(files);
    }

    if session_files.is_empty() {
        anyhow::bail!(
            "No JSONL session files found in project directories. This project may not have any Claude Code usage yet."
        );
    }

    // Process all entries with global entry-level deduplication
    let daily_stats = entry_processor::process_all_entries(&session_files)
        .context("Failed to process entries and aggregate daily statistics")?;

    if daily_stats.is_empty() {
        anyhow::bail!(
            "No valid usage data found. The JSONL files may be corrupted or in an unexpected format."
        );
    }

    // Apply date filtering
    let filtered_stats = filter_daily_stats_by_date(daily_stats, since, until)
        .context("Failed to filter daily stats by date range")?;
    
    if filtered_stats.is_empty() {
        println!("No data found for the specified date range.");
        return Ok(());
    }
    
    // Apply recent filtering
    let recent_filtered_stats = apply_recent_filter_daily(filtered_stats, recent);
    
    // Apply sorting
    let sorted_stats = sort_daily_stats(recent_filtered_stats, order);

    if json {
        // Output in JSON format
        let json_output = generate_json_output(&sorted_stats)
            .context("Failed to generate JSON output")?;
        println!("{}", serde_json::to_string_pretty(&json_output)?);
    } else {
        // Display the table
        let table_output = format_table_with_breakdown(&sorted_stats, breakdown);
        println!("{}", table_output);
    }

    Ok(())
}

/// Handle monthly usage aggregates command
pub fn handle_monthly_command(
    since: Option<&str>,
    until: Option<&str>,
    order: SortOrder,
    json: bool,
    breakdown: bool,
) -> Result<()> {
    // Get current working directory for project lookup
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let project_dirs = jsonl_parser::get_all_project_dirs(&cwd);

    if project_dirs.is_empty() {
        anyhow::bail!(
            "No Claude session data found. Make sure you're in a project directory that has been used with Claude Code."
        );
    }

    // Find all JSONL session files from all project directories
    let mut session_files = Vec::new();
    for project_dir in &project_dirs {
        let files = jsonl_parser::find_session_files(project_dir, None)
            .context("Failed to find session files")?;
        session_files.extend(files);
    }

    if session_files.is_empty() {
        anyhow::bail!(
            "No JSONL session files found in project directories. This project may not have any Claude Code usage yet."
        );
    }

    // Process all entries to get daily stats first
    let daily_stats = entry_processor::process_all_entries(&session_files)
        .context("Failed to process entries and aggregate daily statistics")?;

    if daily_stats.is_empty() {
        anyhow::bail!(
            "No valid usage data found. The JSONL files may be corrupted or in an unexpected format."
        );
    }

    // Apply date filtering to daily stats first
    let filtered_daily_stats = filter_daily_stats_by_date(daily_stats, since, until)
        .context("Failed to filter daily stats by date range")?;
    
    if filtered_daily_stats.is_empty() {
        println!("No data found for the specified date range.");
        return Ok(());
    }

    // Aggregate into monthly stats
    let monthly_stats = aggregate_monthly_stats(&filtered_daily_stats)
        .context("Failed to aggregate monthly statistics")?;
    
    if monthly_stats.is_empty() {
        println!("No monthly data found for the specified date range.");
        return Ok(());
    }
    
    // Apply sorting
    let sorted_monthly = sort_monthly_stats(monthly_stats, order);

    if json {
        // Output in JSON format
        let json_output = generate_monthly_json_output(&sorted_monthly)
            .context("Failed to generate JSON output")?;
        println!("{}", serde_json::to_string_pretty(&json_output)?);
    } else {
        // Display the table
        let table_output = format_monthly_table_with_breakdown(&sorted_monthly, breakdown);
        println!("{}", table_output);
    }

    Ok(())
}

/// Handle individual session reports command
pub fn handle_session_command(
    since: Option<&str>,
    until: Option<&str>,
    order: SortOrder,
    json: bool,
    breakdown: bool,
    recent: Option<usize>,
) -> Result<()> {
    // Get current working directory for project lookup
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let project_dirs = jsonl_parser::get_all_project_dirs(&cwd);

    if project_dirs.is_empty() {
        anyhow::bail!(
            "No Claude session data found. Make sure you're in a project directory that has been used with Claude Code."
        );
    }

    // Find all JSONL session files from all project directories
    let mut session_files = Vec::new();
    for project_dir in &project_dirs {
        let files = jsonl_parser::find_session_files(project_dir, None)
            .context("Failed to find session files")?;
        session_files.extend(files);
    }

    if session_files.is_empty() {
        anyhow::bail!(
            "No JSONL session files found in project directories. This project may not have any Claude Code usage yet."
        );
    }

    // Parse all session files to get sessions
    let mut all_sessions = Vec::new();
    for file in &session_files {
        let session_data = jsonl_parser::parse_session_file(file)
            .context("Failed to parse session file")?;
        all_sessions.push(session_data);
    }

    if all_sessions.is_empty() {
        anyhow::bail!(
            "No valid session data found. The JSONL files may be corrupted or in an unexpected format."
        );
    }

    // Apply date filtering
    let filtered_sessions = filter_sessions_by_date(all_sessions, since, until)
        .context("Failed to filter sessions by date range")?;
    
    if filtered_sessions.is_empty() {
        println!("No sessions found for the specified date range.");
        return Ok(());
    }
    
    // Apply recent filtering
    let recent_filtered_sessions = apply_recent_filter_sessions(filtered_sessions, recent);
    
    // Apply sorting
    let sorted_sessions = sort_sessions(recent_filtered_sessions, order);

    if json {
        // Output in JSON format
        let json_output = generate_session_json_output(&sorted_sessions)
            .context("Failed to generate JSON output")?;
        println!("{}", serde_json::to_string_pretty(&json_output)?);
    } else {
        // Display the table
        let table_output = format_session_table_with_breakdown(&sorted_sessions, breakdown);
        println!("{}", table_output);
    }

    Ok(())
}

/// Generate JSON output for monthly statistics
pub fn generate_monthly_json_output(stats: &[MonthlyStats]) -> Result<serde_json::Value> {
    let json_obj = serde_json::json!({
        "monthly": stats.iter().map(|stat| {
            serde_json::json!({
                "month": stat.month,
                "models": stat.models,
                "input_tokens": stat.input_tokens,
                "output_tokens": stat.output_tokens,
                "cache_creation_tokens": stat.cache_creation_tokens,
                "cache_read_tokens": stat.cache_read_tokens,
                "total_tokens": stat.total_tokens,
                "cost_usd": stat.cost_usd
            })
        }).collect::<Vec<_>>()
    });
    
    Ok(json_obj)
}

/// Generate JSON output for session data
pub fn generate_session_json_output(sessions: &[crate::jsonl_parser::SessionData]) -> Result<serde_json::Value> {
    let session_data: Vec<serde_json::Value> = sessions.iter().map(|session| {
        serde_json::json!({
            "session_id": session.session_id,
            "start_time": session.start_time,
            "end_time": session.end_time,
            "model_usage": session.model_usage,
            "total_weighted_tokens": session.total_weighted_tokens
        })
    }).collect();
    
    let json_obj = serde_json::json!({
        "sessions": session_data
    });
    
    Ok(json_obj)
}

/// Format monthly table with optional breakdown
pub fn format_monthly_table_with_breakdown(stats: &[MonthlyStats], breakdown: bool) -> String {
    if breakdown {
        // TODO: Implement monthly breakdown view
        format_monthly_table_standard(stats)
    } else {
        format_monthly_table_standard(stats)
    }
}

/// Format standard monthly table
pub fn format_monthly_table_standard(stats: &[MonthlyStats]) -> String {
    // Simple table formatting using format! - will be moved from main.rs later
    let mut output = String::new();
    
    // Header
    output.push_str("┌─────────┬─────────────┬──────────────┬───────────────┬──────────────┬─────────────┬──────────────┬─────────────┐\n");
    output.push_str("│ Month   │ Models      │ Input Tokens │ Output Tokens │ Cache Create │ Cache Read  │ Total Tokens │ Cost (USD)  │\n");
    output.push_str("├─────────┼─────────────┼──────────────┼───────────────┼──────────────┼─────────────┼──────────────┼─────────────┤\n");
    
    // Data rows
    for stat in stats {
        output.push_str(&format!(
            "│ {:<7} │ {:<11} │ {:>12} │ {:>13} │ {:>12} │ {:>11} │ {:>12} │ {:>11.2} │\n",
            stat.month,
            stat.models.join(", "),
            format_number(stat.input_tokens),
            format_number(stat.output_tokens),
            format_number(stat.cache_creation_tokens),
            format_number(stat.cache_read_tokens),
            format_number(stat.total_tokens),
            stat.cost_usd
        ));
    }
    
    output.push_str("└─────────┴─────────────┴──────────────┴───────────────┴──────────────┴─────────────┴──────────────┴─────────────┘\n");
    
    // Calculate totals
    let total_tokens: u64 = stats.iter().map(|s| s.total_tokens).sum();
    let total_cost: f64 = stats.iter().map(|s| s.cost_usd).sum();
    
    output.push_str(&format!("\nTotal Usage: {} tokens | Total Cost: ${:.2}", format_number(total_tokens), total_cost));
    
    output
}

/// Format session table with optional breakdown
pub fn format_session_table_with_breakdown(sessions: &[crate::jsonl_parser::SessionData], breakdown: bool) -> String {
    if breakdown {
        // TODO: Implement session breakdown view
        format_session_table_standard(sessions)
    } else {
        format_session_table_standard(sessions)
    }
}

/// Format standard session table
pub fn format_session_table_standard(sessions: &[crate::jsonl_parser::SessionData]) -> String {
    use crate::pricing::calculate_session_cost;
    use crate::table_display::simplify_model_name;
    
    let mut output = String::new();
    
    // Header
    output.push_str("┌──────────────┬─────────────────────┬─────────────┬──────────────┬─────────────┐\n");
    output.push_str("│ Session ID   │ Start Time          │ Models      │ Total Tokens │ Cost (USD)  │\n");
    output.push_str("├──────────────┼─────────────────────┼─────────────┼──────────────┼─────────────┤\n");
    
    // Data rows
    for session in sessions {
        let models: Vec<String> = session.model_usage.keys()
            .map(|model| simplify_model_name(model))
            .collect();
        
        let total_tokens: u64 = session.model_usage.values()
            .map(|usage| usage.total_input + usage.total_output + usage.total_cache_write + usage.total_cache_read)
            .sum();
        
        let cost = calculate_session_cost(&session.model_usage);
        
        let short_session_id = if session.session_id.len() > 12 {
            format!("{}...", &session.session_id[0..12])
        } else {
            session.session_id.clone()
        };
        
        output.push_str(&format!(
            "│ {:<12} │ {:<19} │ {:<11} │ {:>12} │ {:>11.2} │\n",
            short_session_id,
            session.start_time.format("%Y-%m-%d %H:%M").to_string(),
            models.join(", "),
            format_number(total_tokens),
            cost
        ));
    }
    
    output.push_str("└──────────────┴─────────────────────┴─────────────┴──────────────┴─────────────┘\n");
    
    // Calculate totals
    let total_tokens: u64 = sessions.iter().map(|s| {
        s.model_usage.values()
            .map(|usage| usage.total_input + usage.total_output + usage.total_cache_write + usage.total_cache_read)
            .sum::<u64>()
    }).sum();
    
    let total_cost: f64 = sessions.iter()
        .map(|s| calculate_session_cost(&s.model_usage))
        .sum();
    
    output.push_str(&format!("\nTotal Usage: {} tokens | Total Cost: ${:.2}", format_number(total_tokens), total_cost));
    
    output
}