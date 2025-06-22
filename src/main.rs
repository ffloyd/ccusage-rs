//! # CC Usage Monitor
//!
//! Real-time token usage monitoring for Claude
//!
//! ## Key Components
//! - [`run_ccusage`] - Executes ccusage command and parses JSON
//! - [`monitor`] - Main monitoring loop
//! - [`ProgressBar`] - Progress bar display utilities

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Local, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use clap::{Parser, ValueEnum};
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    execute,
    terminal::{Clear, ClearType},
};
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::process::Command;
use std::time::Duration as StdDuration;
use tokio::{signal, time::sleep};

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
}

fn run_ccusage() -> Result<CcUsageData> {
    let output = match Command::new("ccusage").args(&["blocks", "--json"]).output() {
        Ok(output) => output,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                anyhow::bail!("ccusage command not found. Please ensure ccusage is installed and in your PATH");
            } else {
                return Err(e).context("Failed to execute ccusage command");
            }
        }
    };

    if !output.status.success() {
        anyhow::bail!(
            "ccusage command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // Debug: print raw output
    let stdout_str = String::from_utf8_lossy(&output.stdout);
    if stdout_str.trim().is_empty() {
        anyhow::bail!("ccusage returned empty output");
    }

    // Parse JSON with better error handling
    let data: CcUsageData = match serde_json::from_str(&stdout_str) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("JSON parse error: {}", e);
            eprintln!("Raw output from ccusage:");
            eprintln!("{}", stdout_str);
            return Err(e).context("Failed to parse JSON from ccusage output");
        }
    };

    Ok(data)
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

                if max_tokens > 0 {
                    max_tokens
                } else {
                    7000
                }
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

async fn run_monitor(args: Args) -> Result<()> {
    let mut stdout = io::stdout();

    let mut token_limit = if matches!(args.plan, Plan::CustomMax) {
        let initial_data = run_ccusage()?;
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

async fn monitor_iteration(
    args: &Args,
    token_limit: &mut u64,
    stdout: &mut io::Stdout,
) -> Result<()> {
    execute!(stdout, MoveTo(0, 0))?;

    let data = match run_ccusage() {
        Ok(data) => data,
        Err(e) => {
            println!("Error running ccusage: {}", e);
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
    let tokens_used = active_block.total_tokens;

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
    let reset_time = get_next_reset_time(current_time, args.reset_hour.clone(), &args.timezone)?;
    let minutes_to_reset = (reset_time - current_time).num_minutes() as f64;

    // Use projection data from JSON
    let predicted_end_time = if let Some(projection) = &active_block.projection {
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
    println!("🎯 \x1b[97mTokens:\x1b[0m         \x1b[97m{}\x1b[0m / \x1b[90m~{}\x1b[0m (\x1b[96m{} left\x1b[0m)",
        format_number(tokens_used), format_number(*token_limit), format_number(tokens_left));
    let projected_cost = active_block
        .projection
        .as_ref()
        .map(|p| p.total_cost)
        .unwrap_or(active_block.cost_usd);
    println!("💰 \x1b[97mCost:\x1b[0m           \x1b[93m${:.2}\x1b[0m → \x1b[91m${:.2}\x1b[0m \x1b[90m(projected)\x1b[0m", 
        active_block.cost_usd, projected_cost);
    let cost_per_hour = active_block
        .burn_rate
        .as_ref()
        .map(|br| br.cost_per_hour)
        .unwrap_or(0.0);
    println!("🔥 \x1b[97mBurn Rate:\x1b[0m      \x1b[93m{:.1}\x1b[0m \x1b[90mtokens/min\x1b[0m | \x1b[93m${:.2}/hr\x1b[0m", 
        burn_rate, cost_per_hour);
    println!(
        "📊 \x1b[97mToken Types:\x1b[0m    \x1b[90mIn: {}, Out: {}, Cache: {}\x1b[0m",
        format_number(active_block.token_counts.input_tokens),
        format_number(active_block.token_counts.output_tokens),
        format_number(
            active_block.token_counts.cache_creation_input_tokens
                + active_block.token_counts.cache_read_input_tokens
        )
    );
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

    // Status line
    let current_time_str = Local::now().format("%H:%M:%S");
    print!("⏰ \x1b[90m{}\x1b[0m 📝 \x1b[96mSmooth sailing...\x1b[0m | \x1b[90mCtrl+C to exit\x1b[0m 🟨", 
        current_time_str);

    // Clear remaining lines
    execute!(stdout, Clear(ClearType::FromCursorDown))?;
    stdout.flush()?;

    sleep(StdDuration::from_secs(3)).await;
    Ok(())
}
