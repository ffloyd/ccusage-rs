//! # Monitor Module
//!
//! Real-time monitoring functionality for token usage tracking
//!
//! ## Key Components
//! - [`handle_monitor_command`] - Main monitoring command handler
//! - [`run_monitor`] - Core monitoring loop
//! - [`validate_monitor_config`] - Configuration validation
//! - Display utilities for real-time updates

use anyhow::{Context, Result};
use chrono_tz::Tz;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    execute,
    terminal::{Clear, ClearType},
};
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::time::Duration as StdDuration;
use tokio::{signal, time::sleep};

use crate::cli::Plan;
use crate::block_builder::{Block as NativeBlock, build_blocks_from_sessions};

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

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TokenCounts {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
    #[serde(default)]
    pub cache_read_input_tokens: u64,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BurnRate {
    #[serde(default)]
    pub tokens_per_minute: f64,
    #[serde(default)]
    pub cost_per_hour: f64,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Projection {
    #[serde(default)]
    pub total_tokens: u64,
    #[serde(default)]
    pub total_cost: f64,
    #[serde(default)]
    pub remaining_minutes: f64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    #[serde(default)]
    pub id: String,
    pub start_time: String,
    #[serde(default)]
    pub end_time: String,
    #[serde(default)]
    pub actual_end_time: Option<String>,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub is_gap: bool,
    #[serde(default)]
    pub entries: u64,
    #[serde(default)]
    pub token_counts: TokenCounts,
    #[serde(default)]
    pub total_tokens: u64,
    #[serde(rename = "costUSD", default)]
    pub cost_usd: f64,
    #[serde(default)]
    pub models: Vec<String>,
    pub burn_rate: Option<BurnRate>,
    pub projection: Option<Projection>,
}

/// Handle monitor command with real-time updates
pub async fn handle_monitor_command(
    plan: Plan,
    reset_hour: Option<u32>,
    timezone: String,
    active_only: bool,
    recent_blocks: Option<usize>,
    refresh_interval: u64,
) -> Result<()> {
    // Validate monitor configuration
    validate_monitor_config(reset_hour, &timezone)?;
    
    // Setup terminal - don't use raw mode as it interferes with output
    let mut stdout = io::stdout();

    // Initial screen clear and hide cursor
    execute!(stdout, Clear(ClearType::All), Hide)?;

    // Ensure we restore terminal on exit
    let result = run_monitor(plan, reset_hour, timezone, active_only, recent_blocks, refresh_interval).await;

    // Restore terminal
    execute!(stdout, Show)?;

    if result.is_ok() {
        println!("\n\x1b[96mMonitoring stopped.\x1b[0m");
        execute!(stdout, Clear(ClearType::All))?;
    }

    result
}

/// Main monitoring loop
pub async fn run_monitor(plan: Plan, _reset_hour: Option<u32>, _timezone: String, active_only: bool, recent_blocks: Option<usize>, refresh_interval: u64) -> Result<()> {
    let mut stdout = io::stdout();
    
    loop {
        // Clear screen and move to top
        execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
        
        // Get current working directory for project lookup
        let cwd = std::env::current_dir().context("Failed to get current directory")?;
        let project_dirs = crate::jsonl_parser::get_all_project_dirs(&cwd);

        if project_dirs.is_empty() {
            println!("❌ No Claude session data found.");
            println!("   Make sure you're in a project directory that has been used with Claude Code.");
            tokio::select! {
                _ = sleep(StdDuration::from_secs(5)) => continue,
                _ = signal::ctrl_c() => break,
            }
        }

        // Find all JSONL session files from all project directories
        let mut session_files = Vec::new();
        for project_dir in &project_dirs {
            if let Ok(files) = crate::jsonl_parser::find_session_files(project_dir, None) {
                session_files.extend(files);
            }
        }

        if session_files.is_empty() {
            println!("❌ No JSONL session files found.");
            println!("   This project may not have any Claude Code usage yet.");
            tokio::select! {
                _ = sleep(StdDuration::from_secs(5)) => continue,
                _ = signal::ctrl_c() => break,
            }
        }

        // Parse all session files to get sessions
        let mut all_sessions = Vec::new();
        for file in &session_files {
            if let Ok(session_data) = crate::jsonl_parser::parse_session_file(file) {
                all_sessions.push(session_data);
            }
        }

        if all_sessions.is_empty() {
            println!("❌ No valid session data found.");
            println!("   The JSONL files may be corrupted or in an unexpected format.");
            tokio::select! {
                _ = sleep(StdDuration::from_secs(5)) => continue,
                _ = signal::ctrl_c() => break,
            }
        }

        // Build blocks from sessions
        if let Ok(native_blocks) = build_blocks_from_sessions(&all_sessions) {
            let mut blocks: Vec<Block> = native_blocks.into_iter().map(convert_native_block).collect();
            
            // Apply filtering
            if active_only {
                blocks.retain(|block| block.is_active);
            }
            
            if let Some(recent_count) = recent_blocks {
                // Sort by start time (most recent first) and keep only recent blocks
                blocks.sort_by(|a, b| b.start_time.cmp(&a.start_time));
                blocks.truncate(recent_count);
            }
            
            // Display monitoring interface
            print_header();
            
            let token_limit = get_token_limit(plan, Some(&blocks));
            display_blocks(&blocks, token_limit);
        } else {
            println!("❌ Failed to build blocks from sessions.");
        }
        
        stdout.flush()?;

        // Wait for update interval or Ctrl+C
        tokio::select! {
            _ = sleep(StdDuration::from_secs(refresh_interval)) => {},
            _ = signal::ctrl_c() => break,
        }
    }

    Ok(())
}

/// Validate monitor configuration
pub fn validate_monitor_config(reset_hour: Option<u32>, timezone: &str) -> Result<()> {
    // Validate reset hour
    if let Some(hour) = reset_hour {
        if hour > 23 {
            anyhow::bail!("Reset hour must be between 0 and 23, got: {}", hour);
        }
    }
    
    // Validate timezone
    timezone.parse::<Tz>()
        .with_context(|| format!("Invalid timezone: {}", timezone))?;
    
    Ok(())
}

/// Convert native block to monitor block
fn convert_native_block(native_block: NativeBlock) -> Block {
    Block {
        id: native_block.id,
        start_time: native_block.start_time,
        end_time: native_block.end_time,
        actual_end_time: native_block.actual_end_time,
        is_active: native_block.is_active,
        is_gap: native_block.is_gap,
        entries: native_block.entries,
        token_counts: TokenCounts {
            input_tokens: native_block.token_counts.input_tokens,
            output_tokens: native_block.token_counts.output_tokens,
            cache_creation_input_tokens: native_block.token_counts.cache_creation_input_tokens,
            cache_read_input_tokens: native_block.token_counts.cache_read_input_tokens,
        },
        total_tokens: native_block.total_tokens,
        cost_usd: native_block.cost_usd,
        models: native_block.models,
        burn_rate: native_block.burn_rate.map(|br| BurnRate {
            tokens_per_minute: br.tokens_per_minute,
            cost_per_hour: br.cost_per_hour,
        }),
        projection: native_block.projection.map(|p| Projection {
            total_tokens: p.total_tokens,
            total_cost: p.total_cost,
            remaining_minutes: p.remaining_minutes,
        }),
    }
}

/// Print monitoring header
fn print_header() {
    println!();
    println!("\x1b[96m╭──────────────────────────────────────────────────────╮\x1b[0m");
    println!("\x1b[96m│                                                      │\x1b[0m");
    println!("\x1b[96m│                 \x1b[1mClaude Token Monitor\x1b[0m\x1b[96m                 │\x1b[0m");
    println!("\x1b[96m│                                                      │\x1b[0m");
    println!("\x1b[96m│                   \x1b[33mPress Ctrl+C to exit\x1b[0m\x1b[96m                 │\x1b[0m");
    println!("\x1b[96m│                                                      │\x1b[0m");
    println!("\x1b[96m╰──────────────────────────────────────────────────────╯\x1b[0m");
    println!();
}

/// Get token limit based on plan
fn get_token_limit(plan: Plan, blocks: Option<&[Block]>) -> u64 {
    match plan {
        Plan::Pro => 150_000,
        Plan::Max5 => 300_000,
        Plan::Max20 => 2_000_000,
        Plan::CustomMax => {
            if let Some(blocks) = blocks {
                blocks.iter()
                    .map(|b| b.total_tokens)
                    .max()
                    .unwrap_or(2_000_000)
            } else {
                2_000_000
            }
        }
    }
}

/// Display monitoring blocks
fn display_blocks(blocks: &[Block], token_limit: u64) {
    if blocks.is_empty() {
        println!("📊 No usage blocks found yet...");
        return;
    }

    let total_tokens: u64 = blocks.iter().map(|b| b.total_tokens).sum();
    let total_cost: f64 = blocks.iter().map(|b| b.cost_usd).sum();
    
    println!("📊 \x1b[1mUsage Summary\x1b[0m");
    println!("   Total Tokens: \x1b[93m{}\x1b[0m", format_number(total_tokens));
    println!("   Total Cost: \x1b[92m${:.2}\x1b[0m", total_cost);
    println!("   Limit: \x1b[96m{}\x1b[0m", format_number(token_limit));
    
    let usage_percent = (total_tokens as f64 / token_limit as f64 * 100.0).min(100.0);
    println!("   Usage: \x1b[{}m{:.1}%\x1b[0m", 
        if usage_percent > 90.0 { "91" } else if usage_percent > 75.0 { "93" } else { "92" },
        usage_percent);
    
    // Progress bar
    let bar_width = 50;
    let _filled = ((usage_percent / 100.0) * bar_width as f64) as usize;
    let bar = create_token_progress_bar(usage_percent, bar_width);
    println!("   {}", bar);
    
    println!();
    
    // Display recent blocks
    println!("🕐 \x1b[1mRecent Activity\x1b[0m");
    
    let recent_blocks: Vec<_> = blocks.iter()
        .filter(|b| !b.is_gap)
        .rev()
        .take(5)
        .collect();
    
    if recent_blocks.is_empty() {
        println!("   No recent activity");
    } else {
        for block in recent_blocks {
            let status = if block.is_active { "🟢 Active" } else { "⚫ Complete" };
            println!("   {} - {} tokens - ${:.2}", 
                status, format_number(block.total_tokens), block.cost_usd);
        }
    }
}

/// Create token usage progress bar
fn create_token_progress_bar(percentage: f64, width: usize) -> String {
    let filled = ((percentage / 100.0) * width as f64) as usize;
    let empty = width - filled;
    
    let color = if percentage > 90.0 {
        "\x1b[91m" // Red
    } else if percentage > 75.0 {
        "\x1b[93m" // Yellow
    } else {
        "\x1b[92m" // Green
    };
    
    format!("{}[{}{}]\x1b[0m", 
        color,
        "█".repeat(filled),
        "░".repeat(empty))
}

