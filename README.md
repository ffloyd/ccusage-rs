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

### Token Analytics

- **Detailed token breakdown**:
  - Input tokens
  - Output tokens
  - Cache creation tokens
  - Cache read tokens
- **Model usage tracking** - Shows which Claude models you're using
- **Daily aggregations** - Comprehensive daily usage statistics

### Output Formats

- **Table view** - Human-readable daily usage tables
- **JSON output** - Machine-readable format for integration
- **Flexible filtering** - Date ranges and project-specific analysis

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

## Usage

After installation, the `ccusage-rs` command will be available in your PATH:

```bash
# Show daily usage table
ccusage-rs --table

# Output as JSON for integration
ccusage-rs --json

# Analyze specific project directory
ccusage-rs --table --project-dir ~/my-claude-project

# Debug mode with detailed logging
ccusage-rs --table --debug

# Specify custom Claude directory
ccusage-rs --table --claude-dir ~/.claude-custom
```

## Options

- `--table` - Display daily usage in table format
- `--json` - Output results as JSON
- `--project-dir <PATH>` - Analyze specific project directory
- `--claude-dir <PATH>` - Custom Claude directory path (default: ~/.claude)
- `--debug` - Enable debug output and detailed logging
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

## Troubleshooting

If you see "No valid usage data found":

1. Ensure you've used Claude Code recently (generates session logs)
2. Check that `~/.claude/` directory exists
3. Verify session files contain usage data: `ls ~/.claude/sessions/`
4. Use `--debug` flag for detailed parsing information

## Star History

<a href="https://www.star-history.com/#snowmead/ccusage-rs&Date">
    <picture>
        <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=snowmead/ccusage-rs&type=Date&theme=dark" />
        <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=snowmead/ccusage-rs&type=Date" />
        <img alt="Star History Chart" src="https://api.star-history.com/svg?repos=snowmead/ccusage-rs&type=Date" />
    </picture>
</a>

## License

MIT
