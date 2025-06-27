//! # CLI Module
//!
//! Command-line interface definitions and argument parsing for ccusage-rs
//!
//! ## Key Components
//! - [`Args`] - Main CLI arguments structure
//! - [`Commands`] - Subcommand definitions
//! - [`Plan`] - Claude plan type enumeration
//! - [`SortOrder`] - Result sorting options

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Plan {
    Pro,
    Max5,
    Max20,
    CustomMax,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SortOrder {
    Asc,
    Desc,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Show daily usage reports (default)
    Daily {
        /// Filter usage data from date (YYYYMMDD format)
        #[arg(long)]
        since: Option<String>,
        
        /// Filter usage data until date (YYYYMMDD format)
        #[arg(long)]
        until: Option<String>,
        
        /// Sort order for results
        #[arg(long, default_value = "desc", value_enum)]
        order: SortOrder,
        
        /// Output in JSON format
        #[arg(long)]
        json: bool,
        
        /// Show per-model cost breakdown
        #[arg(long)]
        breakdown: bool,
        
        /// Show only recent entries (last N days)
        #[arg(long)]
        recent: Option<usize>,
    },
    /// Show monthly usage aggregates
    Monthly {
        /// Filter usage data from date (YYYYMMDD format)
        #[arg(long)]
        since: Option<String>,
        
        /// Filter usage data until date (YYYYMMDD format)
        #[arg(long)]
        until: Option<String>,
        
        /// Sort order for results
        #[arg(long, default_value = "desc", value_enum)]
        order: SortOrder,
        
        /// Output in JSON format
        #[arg(long)]
        json: bool,
        
        /// Show per-model cost breakdown
        #[arg(long)]
        breakdown: bool,
    },
    /// Show individual session reports
    Session {
        /// Filter usage data from date (YYYYMMDD format)
        #[arg(long)]
        since: Option<String>,
        
        /// Filter usage data until date (YYYYMMDD format)
        #[arg(long)]
        until: Option<String>,
        
        /// Sort order for results
        #[arg(long, default_value = "desc", value_enum)]
        order: SortOrder,
        
        /// Output in JSON format
        #[arg(long)]
        json: bool,
        
        /// Show per-model cost breakdown
        #[arg(long)]
        breakdown: bool,
        
        /// Show only recent entries (last N days)
        #[arg(long)]
        recent: Option<usize>,
    },
    /// Real-time monitoring (original behavior)
    Monitor {
        /// Claude plan type
        #[arg(long, default_value = "pro", value_enum)]
        plan: Plan,
        
        /// Change the reset hour (0-23) for daily limits
        #[arg(long)]
        reset_hour: Option<u32>,
        
        /// Timezone for reset times
        #[arg(long, default_value = "Europe/Warsaw")]
        timezone: String,
        
        /// Show only active blocks (hide completed ones)
        #[arg(long)]
        active: bool,
        
        /// Show only recent blocks (last N blocks)
        #[arg(long)]
        recent: Option<usize>,
        
        /// Update frequency in seconds (default: 2)
        #[arg(long, default_value = "2")]
        refresh_interval: u64,
    },
}

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Claude Token Monitor - Real-time token usage monitoring and analysis"
)]
pub struct Args {
    /// Enable debug logging
    #[arg(long, global = true)]
    pub debug: bool,
    
    /// Custom Claude directory path (can also use CLAUDE_CONFIG_DIR env var)
    #[arg(long, global = true)]
    pub claude_dir: Option<String>,
    
    /// Test new JSONL parser (legacy)
    #[arg(long, global = true)]
    pub test_parser: bool,
    
    /// Offline mode - use cached pricing and skip remote lookups
    #[arg(short = 'O', long, global = true)]
    pub offline: bool,
    
    #[command(subcommand)]
    pub command: Option<Commands>,
}