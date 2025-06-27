//! # Data Processing Module
//!
//! Data filtering, sorting, and aggregation utilities for usage statistics
//!
//! ## Key Components
//! - [`parse_date_filter`] - Parse YYYYMMDD date strings
//! - [`filter_daily_stats_by_date`] - Filter daily statistics by date range
//! - [`sort_daily_stats`] - Sort daily statistics by date
//! - [`MonthlyStats`] - Monthly aggregated statistics
//! - [`SessionStats`] - Session-level statistics

use anyhow::{Context, Result};
use chrono::NaiveDate;
use serde::Serialize;
use std::collections::HashMap;

use crate::cli::SortOrder;
use crate::table_display::DailyStats;
use crate::jsonl_parser::SessionData;
use crate::pricing::calculate_session_cost;

#[derive(Debug, Serialize)]
pub struct MonthlyStats {
    pub month: String,
    pub models: Vec<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub total_tokens: u64,
    pub cost_usd: f64,
}

#[derive(Debug, Serialize)]
pub struct SessionStats {
    pub session_id: String,
    pub start_time: String,
    pub models: Vec<String>,
    pub total_tokens: u64,
    pub cost_usd: f64,
}

/// Parse date in YYYYMMDD format
pub fn parse_date_filter(date_str: &str) -> Result<NaiveDate> {
    if date_str.len() != 8 {
        anyhow::bail!("Date must be in YYYYMMDD format, got: {}", date_str);
    }
    
    let year = date_str[0..4].parse::<i32>()
        .context("Invalid year in date")?;
    let month = date_str[4..6].parse::<u32>()
        .context("Invalid month in date")?;
    let day = date_str[6..8].parse::<u32>()
        .context("Invalid day in date")?;
    
    NaiveDate::from_ymd_opt(year, month, day)
        .context("Invalid date")
}

/// Filter daily statistics by date range
pub fn filter_daily_stats_by_date(
    daily_stats: Vec<DailyStats>,
    since: Option<&str>,
    until: Option<&str>,
) -> Result<Vec<DailyStats>> {
    let since_date = if let Some(since_str) = since {
        Some(parse_date_filter(since_str)?)
    } else {
        None
    };
    
    let until_date = if let Some(until_str) = until {
        Some(parse_date_filter(until_str)?)
    } else {
        None
    };
    
    let filtered: Vec<DailyStats> = daily_stats
        .into_iter()
        .filter(|stat| {
            if let Ok(date) = NaiveDate::parse_from_str(&stat.date, "%Y-%m-%d") {
                let after_since = since_date.is_none_or(|since| date >= since);
                let before_until = until_date.is_none_or(|until| date <= until);
                after_since && before_until
            } else {
                false
            }
        })
        .collect();
    
    Ok(filtered)
}

/// Sort daily statistics by date
pub fn sort_daily_stats(mut daily_stats: Vec<DailyStats>, order: SortOrder) -> Vec<DailyStats> {
    daily_stats.sort_by(|a, b| {
        match order {
            SortOrder::Asc => a.date.cmp(&b.date),
            SortOrder::Desc => b.date.cmp(&a.date),
        }
    });
    daily_stats
}

/// Aggregate daily statistics into monthly summaries
pub fn aggregate_monthly_stats(daily_stats: &[DailyStats]) -> Result<Vec<MonthlyStats>> {
    let mut monthly_map: HashMap<String, MonthlyStats> = HashMap::new();
    
    for daily_stat in daily_stats {
        // Extract year-month from date (YYYY-MM-DD -> YYYY-MM)
        let month_key = daily_stat.date.chars().take(7).collect::<String>();
        
        let monthly_stat = monthly_map.entry(month_key.clone()).or_insert_with(|| MonthlyStats {
            month: month_key,
            models: Vec::new(),
            input_tokens: 0,
            output_tokens: 0,
            cache_creation_tokens: 0,
            cache_read_tokens: 0,
            total_tokens: 0,
            cost_usd: 0.0,
        });
        
        // Aggregate models (ensure uniqueness)
        for model in &daily_stat.models {
            if !monthly_stat.models.contains(model) {
                monthly_stat.models.push(model.clone());
            }
        }
        
        // Aggregate token counts
        monthly_stat.input_tokens += daily_stat.input_tokens;
        monthly_stat.output_tokens += daily_stat.output_tokens;
        monthly_stat.cache_creation_tokens += daily_stat.cache_creation_tokens;
        monthly_stat.cache_read_tokens += daily_stat.cache_read_tokens;
        monthly_stat.total_tokens += daily_stat.total_tokens;
        monthly_stat.cost_usd += daily_stat.cost_usd;
    }
    
    // Convert to sorted vector
    let mut monthly_stats: Vec<MonthlyStats> = monthly_map.into_values().collect();
    monthly_stats.sort_by(|a, b| a.month.cmp(&b.month));
    
    Ok(monthly_stats)
}

/// Sort monthly statistics
pub fn sort_monthly_stats(mut stats: Vec<MonthlyStats>, order: SortOrder) -> Vec<MonthlyStats> {
    stats.sort_by(|a, b| {
        match order {
            SortOrder::Asc => a.month.cmp(&b.month),
            SortOrder::Desc => b.month.cmp(&a.month),
        }
    });
    stats
}

/// Filter sessions by date range
pub fn filter_sessions_by_date(
    sessions: Vec<SessionData>,
    since: Option<&str>,
    until: Option<&str>,
) -> Result<Vec<SessionData>> {
    let since_date = if let Some(since_str) = since {
        Some(parse_date_filter(since_str)?)
    } else {
        None
    };
    
    let until_date = if let Some(until_str) = until {
        Some(parse_date_filter(until_str)?)
    } else {
        None
    };
    
    let filtered: Vec<SessionData> = sessions
        .into_iter()
        .filter(|session| {
            let session_date = session.start_time.date_naive();
            let after_since = since_date.is_none_or(|since| session_date >= since);
            let before_until = until_date.is_none_or(|until| session_date <= until);
            after_since && before_until
        })
        .collect();
    
    Ok(filtered)
}

/// Sort sessions by cost (highest first) or date
pub fn sort_sessions(mut sessions: Vec<SessionData>, order: SortOrder) -> Vec<SessionData> {
    sessions.sort_by(|a, b| {
        let cost_a = calculate_session_cost(&a.model_usage);
        let cost_b = calculate_session_cost(&b.model_usage);
        
        match order {
            SortOrder::Desc => cost_b.partial_cmp(&cost_a).unwrap_or(std::cmp::Ordering::Equal),
            SortOrder::Asc => cost_a.partial_cmp(&cost_b).unwrap_or(std::cmp::Ordering::Equal),
        }
    });
    sessions
}

/// Apply recent filtering to daily stats (keep last N days)
pub fn apply_recent_filter_daily(mut daily_stats: Vec<DailyStats>, recent_days: Option<usize>) -> Vec<DailyStats> {
    if let Some(days) = recent_days {
        // Sort by date descending first to get most recent
        daily_stats.sort_by(|a, b| b.date.cmp(&a.date));
        daily_stats.truncate(days);
        // Re-sort according to original order
        daily_stats.sort_by(|a, b| a.date.cmp(&b.date));
    }
    daily_stats
}

/// Apply recent filtering to sessions (keep last N sessions)
pub fn apply_recent_filter_sessions(mut sessions: Vec<SessionData>, recent_count: Option<usize>) -> Vec<SessionData> {
    if let Some(count) = recent_count {
        // Sort by start time descending to get most recent
        sessions.sort_by(|a, b| b.start_time.cmp(&a.start_time));
        sessions.truncate(count);
    }
    sessions
}