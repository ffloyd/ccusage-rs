//! # CC Usage Monitor
//!
//! Real-time token usage monitoring for Claude
//!
//! ## Key Components
//! - [`cli`] - Command-line interface definitions and argument parsing
//! - [`commands`] - Command handlers for daily, monthly, session operations  
//! - [`data_processing`] - Data filtering, sorting, and aggregation utilities
//! - [`monitor`] - Real-time monitoring functionality

mod block_builder;
mod cli;
mod commands;
mod data_processing;
mod entry_processor;
mod jsonl_parser;
mod models;
mod monitor;
mod pricing;
mod table_display;

use anyhow::Result;
use clap::Parser;
use log::debug;

use cli::{Args, Commands, SortOrder};
use commands::{handle_daily_command, handle_monthly_command, handle_session_command};
use monitor::handle_monitor_command;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Setup logging
    if args.debug {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Debug)
            .init();
        debug!("Debug logging enabled");
    } else {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Warn)
            .init();
    }

    // Handle test parser legacy option
    if args.test_parser {
        return test_parser_comparison();
    }

    // Route to appropriate command handler
    match args.command {
        Some(Commands::Daily { since, until, order, json, breakdown, recent }) => {
            handle_daily_command(since.as_deref(), until.as_deref(), order, json, breakdown, recent)
        }
        Some(Commands::Monthly { since, until, order, json, breakdown }) => {
            handle_monthly_command(since.as_deref(), until.as_deref(), order, json, breakdown)
        }
        Some(Commands::Session { since, until, order, json, breakdown, recent }) => {
            handle_session_command(since.as_deref(), until.as_deref(), order, json, breakdown, recent)
        }
        Some(Commands::Monitor { plan, reset_hour, timezone, active, recent, refresh_interval }) => {
            handle_monitor_command(plan, reset_hour, timezone, active, recent, refresh_interval).await
        }
        None => {
            // Default to daily command for backward compatibility
            handle_daily_command(None, None, SortOrder::Desc, false, false, None)
        }
    }
}

/// Test parser comparison (legacy functionality)
fn test_parser_comparison() -> Result<()> {
    println!("🧪 Testing JSONL parser compatibility...");
    
    // Get current working directory for project lookup
    let cwd = std::env::current_dir()?;
    let project_dirs = jsonl_parser::get_all_project_dirs(&cwd);

    if project_dirs.is_empty() {
        anyhow::bail!(
            "No Claude session data found. Make sure you're in a project directory that has been used with Claude Code."
        );
    }

    // Find all JSONL session files from all project directories
    let mut session_files = Vec::new();
    for project_dir in &project_dirs {
        let files = jsonl_parser::find_session_files(project_dir, None)?;
        session_files.extend(files);
    }

    if session_files.is_empty() {
        anyhow::bail!(
            "No JSONL session files found in project directories. This project may not have any Claude Code usage yet."
        );
    }

    println!("✅ Found {} session files", session_files.len());

    // Test entry processor
    let daily_stats = entry_processor::process_all_entries(&session_files)?;
    println!("✅ Processed {} daily statistics", daily_stats.len());

    // Test session parser
    let mut total_sessions = 0;
    for file in &session_files {
        let _sessions = jsonl_parser::parse_session_file(file)?;
        total_sessions += 1; // parse_session_file returns a single SessionData, not Vec
    }
    println!("✅ Parsed {} sessions", total_sessions);

    println!("🎉 Parser test completed successfully!");
    
    Ok(())
}