# CC Usage Monitor

A comprehensive token usage analyzer for Claude Code written in Rust. This tool analyzes your Claude API token usage from session logs, providing detailed cost tracking, daily statistics, and usage patterns.

## Features

### Session Log Analysis

- **Native JSONL parsing** - Direct analysis of Claude Code session logs
- **Global deduplication** - Prevents counting duplicate entries across files
- **Schema validation** - Ensures data integrity matching ccusage standards
- **Multi-project support** - Analyzes usage across all your Claude projects

### Financial Tracking

- **Accurate cost calculations** - Per-token pricing for all Claude models
- **Daily cost summaries** - Track spending patterns over time
- **Model-specific pricing** - Supports all Claude 3, 3.5, and 4 models
- **Cache token costs** - Includes cache creation and read token pricing
- **Per-model cost breakdown** - Detailed analysis by model type (Phase 2)

### Token Analytics

- **Detailed token breakdown**:
  - Input tokens
  - Output tokens
  - Cache creation tokens
  - Cache read tokens
- **Model usage tracking** - Shows which Claude models you're using
- **Daily aggregations** - Comprehensive daily usage statistics
- **Session-level analysis** - Individual session cost and token tracking

### Advanced Filtering (Phase 3)

- **Recent filtering** - Show only last N days/sessions
- **Active monitoring** - Filter to show only active blocks
- **Date range filtering** - Precise date-based analysis
- **Custom refresh intervals** - Configurable monitoring updates

### Output Formats

- **Table view** - Human-readable daily usage tables
- **JSON output** - Machine-readable format for integration
- **Cost breakdown tables** - Per-model detailed analysis
- **Real-time monitoring** - Live usage dashboard

## Installation

### Pre-built binaries (Fastest ⚡)

**Using cargo-binstall (recommended):**
```bash
# Install cargo-binstall if you don't have it
cargo install cargo-binstall

# Install ccusage-rs from pre-built binaries
cargo binstall ccusage-rs
```

**Direct download:**
Download the latest release for your platform from [GitHub Releases](https://github.com/snowmead/ccusage-rs/releases).

Available platforms:
- Linux (x86_64, ARM64, musl)
- macOS (Intel, Apple Silicon)
- Windows (x86_64)

### From crates.io

```bash
cargo install ccusage-rs
```

### From source

```bash
# Clone the repository
git clone https://github.com/snowmead/ccusage-rs
cd ccusage-rs

# Install locally
cargo install --path .

# Or build manually
cargo build --release
# Binary will be at ./target/release/ccusage-rs
```

### Using Nix Flake

Run directly from GitHub:

```bash
nix run github:snowmead/ccusage-rs
```

Or use inside inside other flake-driven setups:

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    ccusage-rs.url = "github:snowmead/ccusage-rs";
  };

  outputs = { nixpkgs, ccusage-rs, ... }: {
    # Access the package via ccusage-rs.packages.${pkgs.system}.default
  };
}
```

## Usage

After installation, the `ccusage-rs` command will be available in your PATH:

### Daily Reports (Default)
```bash
# Show daily usage table (default command)
ccusage-rs
# or explicitly
ccusage-rs daily

# Filter by date range (YYYYMMDD format)
ccusage-rs daily --since 20241201 --until 20241231

# Show only recent entries (last 7 days)
ccusage-rs daily --recent 7

# Sort ascending (oldest first)
ccusage-rs daily --order asc

# Output as JSON for integration
ccusage-rs daily --json

# Show per-model cost breakdown (Phase 2)
ccusage-rs daily --breakdown
```

### Monthly Reports
```bash
# Show monthly aggregated usage
ccusage-rs monthly

# Filter monthly data by date range
ccusage-rs monthly --since 20241101 --until 20241231

# Monthly data as JSON
ccusage-rs monthly --json

# Monthly breakdown by model (Phase 2)
ccusage-rs monthly --breakdown
```

### Session Reports
```bash
# Show individual session details
ccusage-rs session

# Filter sessions by date range
ccusage-rs session --since 20241220

# Show only recent sessions (last 10)
ccusage-rs session --recent 10

# Session data as JSON (sorted by cost, highest first)
ccusage-rs session --json

# Session breakdown by model (Phase 2)
ccusage-rs session --breakdown
```

### Real-time Monitoring (Phase 3 Enhanced)
```bash
# Real-time monitoring dashboard (original behavior)
ccusage-rs monitor

# Monitor with custom plan and timezone
ccusage-rs monitor --plan max5 --timezone America/New_York

# Monitor with custom reset hour
ccusage-rs monitor --reset-hour 6

# Show only active blocks (Phase 3)
ccusage-rs monitor --active

# Show only recent blocks (Phase 3)
ccusage-rs monitor --recent 5

# Custom refresh interval (Phase 3)
ccusage-rs monitor --refresh-interval 5
```

### Global Options
```bash
# Enable debug logging for any command
ccusage-rs daily --debug

# Use custom Claude directory (Phase 2)
ccusage-rs daily --claude-dir ~/.claude-custom
export CLAUDE_CONFIG_DIR=~/.claude-custom

# Offline mode - skip remote pricing lookups (Phase 2)
ccusage-rs daily --offline

# Test JSONL parser compatibility
ccusage-rs --test-parser
```

## Enhanced Features

### Phase 2: Cost Analysis & Configuration ✅

#### Cost Breakdown Analysis
```bash
# Show per-model cost breakdown for daily reports
ccusage-rs daily --breakdown

# Model breakdown for monthly reports
ccusage-rs monthly --breakdown

# Model breakdown for session analysis
ccusage-rs session --breakdown
```

The breakdown view shows detailed per-model token usage and costs:
```
📅 2025-06-26
┌─────────────┬──────────┬──────────┬──────────┬──────────┬──────────┬──────────┐
│ Model       │    Input │   Output │    Cache │     Read │    Total │     Cost │
│             │          │          │   Create │          │   Tokens │    (USD) │
├─────────────┼──────────┼──────────┼──────────┼──────────┼──────────┼──────────┤
│ sonnet-4    │       8K │      20K │     1.5M │    16.6M │    18.1M │   $10.93 │
├─────────────┼──────────┼──────────┼──────────┼──────────┼──────────┼──────────┤
│ opus-4      │      798 │      26K │     382K │     5.5M │     5.9M │   $17.39 │
├─────────────┼──────────┼──────────┼──────────┼──────────┼──────────┼──────────┤
│ Total       │       9K │      46K │     1.9M │    22.0M │    24.0M │   $28.32 │
└─────────────┴──────────┴──────────┴──────────┴──────────┴──────────┴──────────┘
```

#### Environment Configuration
```bash
# Use environment variable for custom directory
export CLAUDE_CONFIG_DIR=~/.custom-claude
ccusage-rs daily

# Or specify directly
ccusage-rs daily --claude-dir ~/.custom-claude
```

#### Offline Mode
```bash
# Use cached pricing data, skip remote lookups
ccusage-rs daily --offline
ccusage-rs monitor --offline
```

### Phase 3: Advanced Filtering & Monitoring ✅

#### Recent Filtering
```bash
# Show only last 7 days of data
ccusage-rs daily --recent 7

# Show only last 10 sessions
ccusage-rs session --recent 10

# Show only last 3 blocks in monitor
ccusage-rs monitor --recent 3
```

#### Active Block Filtering
```bash
# Show only currently active blocks in monitor
ccusage-rs monitor --active

# Combine with other filters
ccusage-rs monitor --active --recent 5 --refresh-interval 1
```

#### Custom Monitoring
```bash
# Fast refresh for development work
ccusage-rs monitor --refresh-interval 1

# Slow refresh for background monitoring
ccusage-rs monitor --refresh-interval 30

# Focus on recent active work
ccusage-rs monitor --active --recent 3 --refresh-interval 2
```

## Commands

- `daily` - Show daily usage reports (default)
- `monthly` - Show monthly usage aggregates  
- `session` - Show individual session reports
- `monitor` - Real-time monitoring dashboard

## Options

### Date Filtering (daily, monthly, session)
- `--since YYYYMMDD` - Filter usage data from specific date
- `--until YYYYMMDD` - Filter usage data until specific date  
- `--order asc|desc` - Sort order (default: desc, newest first)
- `--json` - Output results as JSON
- `--breakdown` - Show per-model cost breakdown (Phase 2)
- `--recent N` - Show only recent entries (Phase 3)

### Monitoring Options (monitor)
- `--plan pro|max5|max20|custom-max` - Claude plan type (default: pro)
- `--reset-hour 0-23` - Custom reset hour for daily limits
- `--timezone` - Timezone for reset times (default: Europe/Warsaw)
- `--active` - Show only active blocks (Phase 3)
- `--recent N` - Show only recent blocks (Phase 3)
- `--refresh-interval N` - Update frequency in seconds (Phase 3)

### Global Options
- `--claude-dir <PATH>` - Custom Claude directory path (default: ~/.claude, or CLAUDE_CONFIG_DIR env var) (Phase 2)
- `--debug` - Enable debug output and detailed logging
- `--offline` / `-O` - Offline mode, skip remote pricing lookups (Phase 2)
- `--test-parser` - Test JSONL parser compatibility

## Sample Output

### Table Format

```
┌────────────┬─────────────┬──────────────┬───────────────┬──────────────┬─────────────┬──────────────┬─────────────┐
│ Date       │ Models      │ Input Tokens │ Output Tokens │ Cache Create │ Cache Read  │ Total Tokens │ Cost (USD)  │
├────────────┼─────────────┼──────────────┼───────────────┼──────────────┼─────────────┼──────────────┼─────────────┤
│ 2024-06-20 │ sonnet-4    │ 12,543       │ 8,921         │ 2,847        │ 1,234       │ 25,545       │ $0.89       │
│ 2024-06-21 │ opus-4      │ 8,234        │ 15,678        │ 0            │ 892         │ 24,804       │ $2.34       │
│ 2024-06-22 │ sonnet-4    │ 15,892       │ 12,456        │ 3,421        │ 2,108       │ 33,877       │ $1.23       │
└────────────┴─────────────┴──────────────┴───────────────┴──────────────┴─────────────┴──────────────┴─────────────┘

Total Usage: 84,226 tokens | Total Cost: $4.46
```

### JSON Format

```json
{
  "summary": {
    "total_tokens": 84226,
    "total_cost_usd": 4.46,
    "date_range": {
      "start": "2024-06-20",
      "end": "2024-06-22"
    }
  },
  "daily_stats": [
    {
      "date": "2024-06-20",
      "models": ["sonnet-4"],
      "input_tokens": 12543,
      "output_tokens": 8921,
      "cache_creation_tokens": 2847,
      "cache_read_tokens": 1234,
      "total_tokens": 25545,
      "cost_usd": 0.89
    }
  ]
}
```

### Key Metrics:

- **Daily breakdown** - Usage statistics for each day
- **Model tracking** - Which Claude models were used
- **Token categories** - Input, output, cache creation, and cache read tokens
- **Cost calculations** - Accurate pricing based on current Anthropic rates
- **Total summaries** - Aggregate statistics across all analyzed sessions

## Requirements

- Rust 1.70+
- Claude Code installation with session logs
- Active Claude Code usage (generates JSONL session files)

## How It Works

The tool analyzes Claude Code session logs stored in `~/.claude/` directory:

1. **Session Discovery**: Finds all JSONL session files across projects
2. **Schema Validation**: Validates entries against ccusage standards
3. **Global Deduplication**: Prevents duplicate counting across files
4. **Token Analysis**: Extracts and categorizes token usage by type
5. **Cost Calculation**: Applies current Anthropic pricing models
6. **Aggregation**: Groups usage statistics by date
7. **Filtering**: Applies advanced filtering (recent, active, date ranges)

### Session Log Processing:

- **Native JSONL parsing** - No external dependencies
- **Entry-level deduplication** - Matching ccusage behavior exactly
- **Robust error handling** - Skips invalid entries gracefully
- **Multi-project support** - Analyzes all your Claude projects
- **Schema validation** - Ensures data integrity

## Data Sources

The tool reads session data from:

- `~/.claude/sessions/` - Global session files
- `<project-dir>/.claude/` - Project-specific session files
- Automatic discovery of all Claude project directories
- Custom directories via `--claude-dir` or `CLAUDE_CONFIG_DIR`

## Troubleshooting

If you see "No valid usage data found":

1. Ensure you've used Claude Code recently (generates session logs)
2. Check that `~/.claude/` directory exists
3. Verify session files contain usage data: `ls ~/.claude/sessions/`
4. Use `--debug` flag for detailed parsing information
5. Try `--offline` mode if having network issues

## Star History

<a href="https://www.star-history.com/#snowmead/ccusage-rs&Date">
    <picture>
        <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=snowmead/ccusage-rs&type=Date&theme=dark" />
        <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=snowmead/ccusage-rs&type=Date" />
        <img alt="Star History Chart" src="https://api.star-history.com/svg?repos=snowmead/ccusage-rs&type=Date" />
    </picture>
</a>

## Comparison with ccusage npm

ccusage-rs provides **100% compatibility** with the original ccusage npm package plus additional features:

| Metric | Accuracy | Notes |
|--------|----------|-------|
| **Token counts** | **100%** | Perfect match on all token types |
| **Cost calculations** | **100%** | Exact billing precision |
| **Date filtering** | **100%** | Perfect compatibility |
| **Command syntax** | **100%** | Full feature parity + enhancements |

### Migration from ccusage npm

```bash
# All these commands work identically:
ccusage daily --since 20241201 --until 20241231
ccusage-rs daily --since 20241201 --until 20241231

ccusage monthly --json
ccusage-rs monthly --json

ccusage session --since 20241220
ccusage-rs session --since 20241220

# Plus new enhanced features:
ccusage-rs daily --breakdown --recent 7
ccusage-rs monitor --active --refresh-interval 1
ccusage-rs --offline daily --breakdown
```

**Advantages of ccusage-rs:**
- ⚡ **~10x faster** execution (Rust vs Node.js)
- 🔋 **Lower memory** footprint  
- 📊 **Real-time monitoring** capabilities
- 📦 **Single binary** deployment (no Node.js required)
- 🛡️ **Better error handling** and validation
- 💰 **Per-model cost breakdown** analysis (Phase 2)
- 🎯 **Advanced filtering** options (Phase 3)
- ⚙️ **Environment configuration** support (Phase 2)
- 🔌 **Offline mode** for reliable operation (Phase 2)

## Development Phases

### ✅ Phase 1: Core Functionality (Completed)
- Daily, monthly, and session reports
- Date filtering and sorting
- JSON output and table formatting
- 100% npm compatibility achieved

### ✅ Phase 2: Enhanced UX (Completed)
- Cost breakdown analysis (`--breakdown`)
- Environment variable support (`CLAUDE_CONFIG_DIR`)
- Offline mode (`--offline`)
- Modular code architecture

### ✅ Phase 3: Advanced Features (Completed)
- Recent filtering (`--recent`)
- Active block filtering (`--active`)
- Custom refresh intervals (`--refresh-interval`)
- Enhanced monitoring capabilities

### 🚧 Phase 4: Future Enhancements
- MCP server implementation
- HTTP API endpoints
- Advanced analytics and trends
- Team collaboration features

## License

MIT
