//! # CC Usage Monitor
//!
//! Real-time token usage monitoring for Claude
//!
//! ## Key Components
//! - [`run_native_analysis`] - Analyzes Claude session JSONL files directly
//! - [`monitor`] - Main monitoring loop
//! - [`ProgressBar`] - Progress bar display utilities

mod analytics;
mod block_builder;
mod entry_processor;
mod jsonl_parser;
mod models;
mod plan_detector;
mod predictor;
mod pricing;
mod session;
mod table_display;

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Local, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use clap::{Parser, ValueEnum};
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    execute,
    terminal::{Clear, ClearType},
};
use log::debug;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, Write};
use std::time::Duration as StdDuration;
use tokio::{signal, time::sleep};

use crate::block_builder::{Block as NativeBlock, build_blocks_from_sessions};
use crate::models::get_model_config;
use crate::table_display::{aggregate_daily_stats, format_table};

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct TokenCounts {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct BurnRate {
    #[serde(default)]
    tokens_per_minute: f64,
    #[serde(default)]
    cost_per_hour: f64,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Projection {
    #[serde(default)]
    total_tokens: u64,
    #[serde(default)]
    total_cost: f64,
    #[serde(default)]
    remaining_minutes: f64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Block {
    #[serde(default)]
    id: String,
    start_time: String,
    #[serde(default)]
    end_time: String,
    #[serde(default)]
    actual_end_time: Option<String>,
    #[serde(default)]
    is_active: bool,
    #[serde(default)]
    is_gap: bool,
    #[serde(default)]
    entries: u64,
    #[serde(default)]
    token_counts: TokenCounts,
    #[serde(default)]
    total_tokens: u64,
    #[serde(rename = "costUSD", default)]
    cost_usd: f64,
    #[serde(default)]
    models: Vec<String>,
    burn_rate: Option<BurnRate>,
    projection: Option<Projection>,
    // New fields for enhanced tracking
    #[serde(skip_serializing_if = "Option::is_none")]
    model_breakdown: Option<HashMap<String, TokenCounts>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    weighted_total_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    context_consumption_rate: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum CcUsageData {
    BlocksObject { blocks: Vec<Block> },
    BlocksArray(Vec<Block>),
}

impl CcUsageData {
    fn blocks(self) -> Vec<Block> {
        match self {
            CcUsageData::BlocksObject { blocks } => blocks,
            CcUsageData::BlocksArray(blocks) => blocks,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Plan {
    Pro,
    Max5,
    Max20,
    CustomMax,
}

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Claude Token Monitor - Real-time token usage monitoring"
)]
struct Args {
    /// Claude plan type
    #[arg(long, default_value = "pro", value_enum)]
    plan: Plan,

    /// Change the reset hour (0-23) for daily limits
    #[arg(long)]
    reset_hour: Option<u32>,

    /// Timezone for reset times
    #[arg(long, default_value = "Europe/Warsaw")]
    timezone: String,

    /// Enable debug logging
    #[arg(long)]
    debug: bool,

    /// Show table view instead of real-time monitoring
    #[arg(long)]
    table: bool,

    /// Test new JSONL parser
    #[arg(long)]
    test_parser: bool,

    /// Output in JSON format
    #[arg(long)]
    json: bool,
}

fn run_native_analysis() -> Result<CcUsageData> {
    // Get current working directory for project lookup
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let project_dir = jsonl_parser::get_project_dir(&cwd);

    if !project_dir.exists() {
        anyhow::bail!(
            "No Claude session data found at {}. Make sure you're in a project directory that has been used with Claude Code.",
            project_dir.display()
        );
    }

    debug!("Looking for session files in: {}", project_dir.display());

    // Find all JSONL session files
    let session_files = jsonl_parser::find_session_files(&project_dir, None)
        .context("Failed to find session files")?;

    if session_files.is_empty() {
        anyhow::bail!(
            "No JSONL session files found in {}. This project may not have any Claude Code usage yet.",
            project_dir.display()
        );
    }

    debug!("Found {} session files", session_files.len());

    // Parse all sessions
    let mut all_sessions = Vec::new();
    for file in session_files {
        match jsonl_parser::parse_session_file(&file) {
            Ok(session) => {
                debug!(
                    "Parsed session {} with {} tokens",
                    session.session_id, session.total_weighted_tokens
                );
                all_sessions.push(session);
            }
            Err(e) => {
                debug!("Failed to parse session file {}: {}", file.display(), e);
                continue;
            }
        }
    }

    if all_sessions.is_empty() {
        anyhow::bail!(
            "No valid session data found. The JSONL files may be corrupted or in an unexpected format."
        );
    }

    debug!("Successfully parsed {} sessions", all_sessions.len());

    // Convert sessions to blocks using our native implementation
    let native_blocks = build_blocks_from_sessions(&all_sessions)
        .context("Failed to build blocks from sessions")?;

    debug!("Built {} blocks from sessions", native_blocks.len());

    // Convert native blocks to the expected Block format
    let blocks: Vec<Block> = native_blocks
        .into_iter()
        .map(|nb| convert_native_block(nb))
        .collect();

    Ok(CcUsageData::BlocksArray(blocks))
}

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
        projection: native_block.projection.map(|proj| Projection {
            total_tokens: proj.total_tokens,
            total_cost: proj.total_cost,
            remaining_minutes: proj.remaining_minutes,
        }),
        model_breakdown: native_block.model_breakdown.map(|breakdown| {
            breakdown
                .into_iter()
                .map(|(model, counts)| {
                    (
                        model,
                        TokenCounts {
                            input_tokens: counts.input_tokens,
                            output_tokens: counts.output_tokens,
                            cache_creation_input_tokens: counts.cache_creation_input_tokens,
                            cache_read_input_tokens: counts.cache_read_input_tokens,
                        },
                    )
                })
                .collect()
        }),
        weighted_total_tokens: native_block.weighted_total_tokens,
        context_consumption_rate: native_block.context_consumption_rate,
    }
}

fn format_time(minutes: f64) -> String {
    if minutes < 60.0 {
        format!("{}m", minutes as i32)
    } else {
        let hours = (minutes / 60.0) as i32;
        let mins = (minutes % 60.0) as i32;
        if mins == 0 {
            format!("{}h", hours)
        } else {
            format!("{}h {}m", hours, mins)
        }
    }
}

fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

fn create_token_progress_bar(percentage: f64, width: usize) -> String {
    let filled = ((width as f64) * percentage / 100.0) as usize;
    let empty = width.saturating_sub(filled);

    let green_bar = "█".repeat(filled);
    let red_bar = "░".repeat(empty);

    format!(
        "🟢 [{}{}{}{}{}] {:.1}%",
        "\x1b[92m", green_bar, "\x1b[91m", red_bar, "\x1b[0m", percentage
    )
}

fn create_time_progress_bar(elapsed_minutes: f64, total_minutes: f64, width: usize) -> String {
    let percentage = if total_minutes <= 0.0 {
        0.0
    } else {
        (elapsed_minutes / total_minutes * 100.0).min(100.0)
    };

    let filled = ((width as f64) * percentage / 100.0) as usize;
    let empty = width.saturating_sub(filled);

    let blue_bar = "█".repeat(filled);
    let red_bar = "░".repeat(empty);

    let remaining_time = format_time((total_minutes - elapsed_minutes).max(0.0));

    format!(
        "⏰ [{}{}{}{}{}] {}",
        "\x1b[94m", blue_bar, "\x1b[91m", red_bar, "\x1b[0m", remaining_time
    )
}

fn print_header() {
    let cyan = "\x1b[96m";
    let blue = "\x1b[94m";
    let reset = "\x1b[0m";

    let sparkles = format!("{}✦ ✧ ✦ ✧ {}", cyan, reset);

    println!(
        "{}{}CLAUDE TOKEN MONITOR{} {}",
        sparkles, cyan, reset, sparkles
    );
    println!("{}{}{}", blue, "=".repeat(60), reset);
    println!();
}

fn get_next_reset_time(
    current_time: DateTime<Utc>,
    custom_reset_hour: Option<u32>,
    timezone_str: &str,
) -> Result<DateTime<Utc>> {
    let target_tz: Tz = timezone_str
        .parse()
        .unwrap_or_else(|_| "Europe/Warsaw".parse().unwrap());

    let target_time = current_time.with_timezone(&target_tz);

    let reset_hours = if let Some(hour) = custom_reset_hour {
        vec![hour]
    } else {
        vec![4, 9, 14, 18, 23]
    };

    let current_hour = target_time.hour();
    let current_minute = target_time.minute();

    let mut next_reset_hour = None;
    for &hour in &reset_hours {
        if current_hour < hour || (current_hour == hour && current_minute == 0) {
            next_reset_hour = Some(hour);
            break;
        }
    }

    let (next_reset_hour, next_reset_date) = if let Some(hour) = next_reset_hour {
        (hour, target_time.date_naive())
    } else {
        (reset_hours[0], target_time.date_naive() + Duration::days(1))
    };

    let next_reset_naive = next_reset_date
        .and_hms_opt(next_reset_hour, 0, 0)
        .context("Failed to create reset time")?;

    let next_reset = target_tz
        .from_local_datetime(&next_reset_naive)
        .single()
        .context("Failed to convert reset time to timezone")?;

    Ok(next_reset.with_timezone(&Utc))
}

fn get_token_limit(plan: Plan, blocks: Option<&[Block]>) -> u64 {
    match plan {
        Plan::CustomMax => {
            if let Some(blocks) = blocks {
                let max_tokens = blocks
                    .iter()
                    .filter(|b| !b.is_gap && !b.is_active)
                    .map(|b| b.total_tokens)
                    .max()
                    .unwrap_or(0);

                if max_tokens > 0 { max_tokens } else { 7000 }
            } else {
                7000
            }
        }
        Plan::Pro => 7000,
        Plan::Max5 => 35000,
        Plan::Max20 => 140000,
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logger based on debug flag
    if args.debug {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    }

    // Validate configuration
    validate_config(&args)?;

    if args.test_parser {
        // Test new parser and exit
        test_parser_comparison(&args)
    } else if args.table || args.json {
        // Show table view or JSON output and exit
        show_table_view(&args)
    } else {
        // Setup terminal - don't use raw mode as it interferes with output
        let mut stdout = io::stdout();

        // Initial screen clear and hide cursor
        execute!(stdout, Clear(ClearType::All), Hide)?;

        // Ensure we restore terminal on exit
        let result = run_monitor(args).await;

        // Restore terminal
        execute!(stdout, Show)?;

        if result.is_ok() {
            println!("\n\x1b[96mMonitoring stopped.\x1b[0m");
            execute!(stdout, Clear(ClearType::All))?;
        }

        result
    }
}

fn show_table_view(args: &Args) -> Result<()> {
    // Get current working directory for project lookup
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let project_dirs = jsonl_parser::get_all_project_dirs(&cwd);

    if project_dirs.is_empty() {
        anyhow::bail!(
            "No Claude session data found. Make sure you're in a project directory that has been used with Claude Code."
        );
    }

    // Found {} project directories

    // Find all JSONL session files from all project directories
    let mut session_files = Vec::new();
    for project_dir in &project_dirs {
        let files = jsonl_parser::find_session_files(project_dir, None)
            .context("Failed to find session files")?;
        
        if std::env::var("CC_USAGE_DETAILED_DEBUG").is_ok() {
            println!("DEBUG: Found {} files in project dir: {}", files.len(), project_dir.display());
            for file in &files {
                println!("DEBUG: Session file: {}", file.display());
            }
        }
        
        session_files.extend(files);
    }

    if session_files.is_empty() {
        anyhow::bail!(
            "No JSONL session files found in project directories. This project may not have any Claude Code usage yet."
        );
    }

    // Process all entries with global entry-level deduplication (matching ccusage exactly)
    let daily_stats = entry_processor::process_all_entries(&session_files)
        .context("Failed to process entries and aggregate daily statistics")?;

    if daily_stats.is_empty() {
        anyhow::bail!(
            "No valid usage data found. The JSONL files may be corrupted or in an unexpected format."
        );
    }

    if args.json {
        // Output in JSON format
        let json_output = table_display::generate_json_output(&daily_stats)
            .context("Failed to generate JSON output")?;
        println!("{}", serde_json::to_string_pretty(&json_output)?);
    } else {
        // Display the table
        let table_output = format_table(&daily_stats);
        println!("{}", table_output);
    }

    Ok(())
}

fn test_parser_comparison(_args: &Args) -> Result<()> {
    // Get current working directory for project lookup
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let project_dir = jsonl_parser::get_project_dir(&cwd);

    if !project_dir.exists() {
        anyhow::bail!("No Claude session data found at {}.", project_dir.display());
    }

    // Find all JSONL session files
    let session_files = jsonl_parser::find_session_files(&project_dir, None)
        .context("Failed to find session files")?;

    if session_files.is_empty() {
        anyhow::bail!("No JSONL session files found.");
    }

    println!("=== PARSER COMPARISON TEST ===");
    println!("Found {} session files", session_files.len());
    println!();

    let mut old_total_tokens = 0u64;
    let mut new_total_tokens = 0u64;
    let mut old_total_cost = 0.0;
    let mut new_total_cost = 0.0;

    for file in session_files.iter().take(5) {
        // Test first 5 files
        println!(
            "Testing file: {}",
            file.file_name().unwrap_or_default().to_string_lossy()
        );

        // Parse with old method
        match jsonl_parser::parse_session_file(file) {
            Ok(old_session) => {
                let old_cost = crate::pricing::calculate_session_cost(&old_session.model_usage);
                old_total_tokens += old_session.total_weighted_tokens;
                old_total_cost += old_cost;
                println!(
                    "  OLD: {} tokens, ${:.2} cost, {} models",
                    old_session.total_weighted_tokens,
                    old_cost,
                    old_session.model_usage.len()
                );
            }
            Err(e) => println!("  OLD: Failed - {}", e),
        }

        // Parse with deduplication method
        let mut temp_hashes = std::collections::HashSet::new();
        match jsonl_parser::parse_session_file_with_deduplication(file, &mut temp_hashes) {
            Ok(new_session) => {
                let new_cost = crate::pricing::calculate_session_cost(&new_session.model_usage);
                new_total_tokens += new_session.total_weighted_tokens;
                new_total_cost += new_cost;
                println!(
                    "  NEW: {} tokens, ${:.2} cost, {} models",
                    new_session.total_weighted_tokens,
                    new_cost,
                    new_session.model_usage.len()
                );
            }
            Err(e) => println!("  NEW: Failed - {}", e),
        }
        println!();
    }

    println!("=== SUMMARY ===");
    println!(
        "Old parser total: {} tokens, ${:.2} cost",
        old_total_tokens, old_total_cost
    );
    println!(
        "New parser total: {} tokens, ${:.2} cost",
        new_total_tokens, new_total_cost
    );
    println!(
        "Difference: {} tokens ({:.1}%), ${:.2} cost ({:.1}%)",
        new_total_tokens as i64 - old_total_tokens as i64,
        if old_total_tokens > 0 {
            ((new_total_tokens as f64 - old_total_tokens as f64) / old_total_tokens as f64) * 100.0
        } else {
            0.0
        },
        new_total_cost - old_total_cost,
        if old_total_cost > 0.0 {
            ((new_total_cost - old_total_cost) / old_total_cost) * 100.0
        } else {
            0.0
        }
    );

    Ok(())
}

async fn run_monitor(args: Args) -> Result<()> {
    let mut stdout = io::stdout();

    let mut token_limit = if matches!(args.plan, Plan::CustomMax) {
        let initial_data = run_native_analysis()?;
        let blocks = initial_data.blocks();
        get_token_limit(args.plan, Some(&blocks))
    } else {
        get_token_limit(args.plan, None)
    };

    loop {
        // Handle Ctrl+C
        tokio::select! {
            _ = signal::ctrl_c() => {
                break;
            }
            _ = monitor_iteration(&args, &mut token_limit, &mut stdout) => {
                // Continue looping
            }
        }
    }

    Ok(())
}

fn validate_config(args: &Args) -> Result<()> {
    // Validate timezone
    if args.timezone.parse::<Tz>().is_err() {
        anyhow::bail!(
            "Invalid timezone '{}'. Use format like 'Europe/Warsaw' or 'America/New_York'",
            args.timezone
        );
    }

    // Validate reset hour
    if let Some(hour) = args.reset_hour {
        if hour >= 24 {
            anyhow::bail!("Invalid reset hour '{}'. Must be 0-23", hour);
        }
    }

    Ok(())
}

async fn monitor_iteration(
    args: &Args,
    token_limit: &mut u64,
    stdout: &mut io::Stdout,
) -> Result<()> {
    execute!(stdout, MoveTo(0, 0))?;

    let data = match run_native_analysis() {
        Ok(data) => data,
        Err(e) => {
            println!("Error analyzing usage data: {}", e);
            sleep(StdDuration::from_secs(3)).await;
            return Ok(());
        }
    };

    let blocks = data.blocks();
    let active_block = blocks.iter().find(|b| b.is_active);

    if active_block.is_none() {
        println!("No active session found");
        println!("Total blocks found: {}", blocks.len());
        sleep(StdDuration::from_secs(3)).await;
        return Ok(());
    }

    let active_block = active_block.unwrap();

    // Use weighted tokens if available, otherwise estimate from raw tokens and models
    let tokens_used = active_block.weighted_total_tokens.unwrap_or_else(|| {
        // Estimate weighted tokens based on model mix and raw tokens
        let raw_tokens = active_block.total_tokens;
        if !active_block.models.is_empty() {
            // Calculate average multiplier for the models in use
            let total_multiplier: f64 = active_block
                .models
                .iter()
                .map(|model| {
                    get_model_config(model)
                        .map(|c| c.consumption_multiplier)
                        .unwrap_or(1.0)
                })
                .sum();
            let avg_multiplier = total_multiplier / active_block.models.len() as f64;
            (raw_tokens as f64 * avg_multiplier) as u64
        } else {
            raw_tokens
        }
    });
    let raw_tokens = active_block.total_tokens;

    // Auto-switch to custom_max if needed
    if tokens_used > *token_limit && matches!(args.plan, Plan::Pro) {
        let new_limit = get_token_limit(Plan::CustomMax, Some(&blocks));
        if new_limit > *token_limit {
            *token_limit = new_limit;
        }
    }

    let usage_percentage = if *token_limit > 0 {
        (tokens_used as f64 / *token_limit as f64) * 100.0
    } else {
        0.0
    };
    let tokens_left = token_limit.saturating_sub(tokens_used);

    // Time calculations
    let current_time = Utc::now();

    // Use burn rate from JSON data
    let burn_rate = active_block
        .burn_rate
        .as_ref()
        .map(|br| br.tokens_per_minute)
        .unwrap_or(0.0);

    // Reset time calculation
    let reset_time = get_next_reset_time(current_time, args.reset_hour, &args.timezone)?;
    let minutes_to_reset = (reset_time - current_time).num_minutes() as f64;

    // Use enhanced predictor if we have model breakdown
    let predicted_end_time = if let Some(model_breakdown) = &active_block.model_breakdown {
        // Create model usage map for predictor
        let model_tokens: HashMap<String, u64> = model_breakdown
            .iter()
            .map(|(model, counts)| (model.clone(), counts.input_tokens + counts.output_tokens))
            .collect();

        let mut predictor =
            predictor::ContextPredictor::new(tokens_used, *token_limit, &model_tokens);
        predictor.set_burn_rate(burn_rate);

        let prediction = predictor.predict_exhaustion(reset_time);
        prediction.predicted_exhaustion_time
    } else if let Some(projection) = &active_block.projection {
        current_time + Duration::minutes(projection.remaining_minutes as i64)
    } else {
        reset_time
    };

    // Display
    print_header();

    // Token Usage
    println!(
        "📊 \x1b[97mToken Usage:\x1b[0m    {}",
        create_token_progress_bar(usage_percentage, 50)
    );
    println!();

    // Time to Reset
    let time_since_reset = (300.0 - minutes_to_reset).max(0.0);
    println!(
        "⏳ \x1b[97mTime to Reset:\x1b[0m  {}",
        create_time_progress_bar(time_since_reset, 300.0, 50)
    );
    println!();

    // Detailed stats
    println!(
        "🎯 \x1b[97mTokens:\x1b[0m         \x1b[97m{}\x1b[0m / \x1b[90m~{}\x1b[0m (\x1b[96m{} left\x1b[0m)",
        format_number(tokens_used),
        format_number(*token_limit),
        format_number(tokens_left)
    );

    // Show weighted vs raw if different
    if let Some(weighted) = active_block.weighted_total_tokens {
        if weighted != raw_tokens {
            let multiplier = weighted as f64 / raw_tokens as f64;
            println!(
                "⚖️  \x1b[97mWeighted:\x1b[0m       \x1b[95m{}\x1b[0m \x1b[90m(raw: {} × {:.1}x)\x1b[0m",
                format_number(weighted),
                format_number(raw_tokens),
                multiplier
            );
        }
    }

    let projected_cost = active_block
        .projection
        .as_ref()
        .map(|p| p.total_cost)
        .unwrap_or(active_block.cost_usd);
    println!(
        "💰 \x1b[97mCost:\x1b[0m           \x1b[93m${:.2}\x1b[0m → \x1b[91m${:.2}\x1b[0m \x1b[90m(projected)\x1b[0m",
        active_block.cost_usd, projected_cost
    );
    let cost_per_hour = active_block
        .burn_rate
        .as_ref()
        .map(|br| br.cost_per_hour)
        .unwrap_or(0.0);
    println!(
        "🔥 \x1b[97mBurn Rate:\x1b[0m      \x1b[93m{:.1}\x1b[0m \x1b[90mtokens/min\x1b[0m | \x1b[93m${:.2}/hr\x1b[0m",
        burn_rate, cost_per_hour
    );

    // Show model breakdown if available
    if let Some(model_breakdown) = &active_block.model_breakdown {
        println!();
        println!("🤖 \x1b[97mModel Breakdown:\x1b[0m");
        for (model, counts) in model_breakdown {
            let model_total = counts.input_tokens + counts.output_tokens;
            let model_name = model.split('-').take(2).collect::<Vec<_>>().join("-");
            let multiplier = models::get_model_config(model)
                .map(|c| c.consumption_multiplier)
                .unwrap_or(1.0);
            println!(
                "   \x1b[94m{:<15}\x1b[0m \x1b[97m{:>8}\x1b[0m tokens \x1b[90m(×{:.1})\x1b[0m",
                model_name,
                format_number(model_total),
                multiplier
            );
        }
    } else {
        println!(
            "📊 \x1b[97mToken Types:\x1b[0m    \x1b[90mIn: {}, Out: {}, Cache: {}\x1b[0m",
            format_number(active_block.token_counts.input_tokens),
            format_number(active_block.token_counts.output_tokens),
            format_number(
                active_block.token_counts.cache_creation_input_tokens
                    + active_block.token_counts.cache_read_input_tokens
            )
        );
    }
    println!();

    // Predictions
    let target_tz: Tz = args
        .timezone
        .parse()
        .unwrap_or_else(|_| "Europe/Warsaw".parse().unwrap());

    let predicted_end_local = predicted_end_time.with_timezone(&target_tz);
    let reset_time_local = reset_time.with_timezone(&target_tz);

    println!(
        "🏁 \x1b[97mPredicted End:\x1b[0m {}",
        predicted_end_local.format("%H:%M")
    );
    println!(
        "🔄 \x1b[97mToken Reset:\x1b[0m   {}",
        reset_time_local.format("%H:%M")
    );

    // Show model info
    let models_str = active_block.models.join(", ");
    println!(
        "🤖 \x1b[97mModel:\x1b[0m          \x1b[90m{}\x1b[0m",
        models_str
    );
    println!();

    // Notifications
    if tokens_used > 7000 && matches!(args.plan, Plan::Pro) && *token_limit > 7000 {
        println!(
            "🔄 \x1b[93mTokens exceeded Pro limit - switched to custom_max ({})\x1b[0m",
            format_number(*token_limit)
        );
        println!();
    }

    if tokens_used > *token_limit {
        println!(
            "🚨 \x1b[91mTOKENS EXCEEDED MAX LIMIT! ({} > {})\x1b[0m",
            format_number(tokens_used),
            format_number(*token_limit)
        );
        println!();
    }

    if predicted_end_time < reset_time {
        println!("⚠️  \x1b[91mTokens will run out BEFORE reset!\x1b[0m");
        println!();
    }

    // Warn about Opus usage on max5 plan
    if matches!(args.plan, Plan::Max5) {
        if let Some(model_breakdown) = &active_block.model_breakdown {
            let opus_tokens: u64 = model_breakdown
                .iter()
                .filter(|(model, _)| model.contains("opus"))
                .map(|(_, counts)| counts.input_tokens + counts.output_tokens)
                .sum();
            let total_model_tokens: u64 = model_breakdown
                .iter()
                .map(|(_, counts)| counts.input_tokens + counts.output_tokens)
                .sum();

            if total_model_tokens > 0 {
                let opus_percentage = opus_tokens as f64 / total_model_tokens as f64;
                if opus_percentage > 0.2 {
                    println!(
                        "⚠️  \x1b[93mHigh Opus usage ({:.0}%) may trigger early limit on Max5 plan!\x1b[0m",
                        opus_percentage * 100.0
                    );
                    println!();
                }
            }
        }
    }

    // Status line
    let current_time_str = Local::now().format("%H:%M:%S");
    print!(
        "⏰ \x1b[90m{}\x1b[0m 📝 \x1b[96mSmooth sailing...\x1b[0m | \x1b[90mCtrl+C to exit\x1b[0m 🟨",
        current_time_str
    );

    // Clear remaining lines
    execute!(stdout, Clear(ClearType::FromCursorDown))?;
    stdout.flush()?;

    sleep(StdDuration::from_secs(3)).await;
    Ok(())
}

fn deduplicate_sessions(
    sessions: Vec<jsonl_parser::SessionData>,
) -> Vec<jsonl_parser::SessionData> {
    let mut session_map: HashMap<String, jsonl_parser::SessionData> = HashMap::new();


    for session in sessions {
        let session_id = session.session_id.clone();

        if let Some(existing_session) = session_map.get_mut(&session_id) {
            // Merge this session with the existing one

            // Use the earliest start time and latest end time
            if session.start_time < existing_session.start_time {
                existing_session.start_time = session.start_time;
            }
            if let Some(session_end) = session.end_time {
                if existing_session.end_time.is_none()
                    || session_end > existing_session.end_time.unwrap()
                {
                    existing_session.end_time = Some(session_end);
                }
            }

            // Merge model usage data
            for (model_name, usage) in session.model_usage {
                let existing_usage = existing_session
                    .model_usage
                    .entry(model_name.clone())
                    .or_insert_with(|| jsonl_parser::ModelUsage {
                        model_name: model_name.clone(),
                        total_input: 0,
                        total_output: 0,
                        total_cache_write: 0,
                        total_cache_read: 0,
                        message_count: 0,
                        weighted_tokens: 0,
                    });

                // Merge usage data
                existing_usage.total_input += usage.total_input;
                existing_usage.total_output += usage.total_output;
                existing_usage.total_cache_write += usage.total_cache_write;
                existing_usage.total_cache_read += usage.total_cache_read;
                existing_usage.message_count += usage.message_count;
                existing_usage.weighted_tokens += usage.weighted_tokens;
            }

            // Merge other flags
            existing_session.has_limit_error |= session.has_limit_error;
            if session.limit_type.is_some() {
                existing_session.limit_type = session.limit_type;
            }
        } else {
            // First time seeing this session ID
            session_map.insert(session_id, session);
        }
    }

    // Recalculate totals for each deduplicated session
    for session in session_map.values_mut() {
        session.calculate_totals();
    }

    session_map.into_values().collect()
}
