# CC Usage Monitor

A real-time token usage monitor for Claude Code (cc) written in Rust. This tool provides comprehensive monitoring of your Claude API token usage across all instances, with detailed cost tracking and predictions.

## Features

### Core Monitoring

- **Real-time token usage tracking** across all Claude Code instances
- **Visual progress bars** for token usage and time remaining
- **Automatic plan detection** (Pro/Max5/Max20/CustomMax)
- **Multi-instance support** - monitors all Claude usage on your account

### Financial Tracking

- **Cost monitoring** - displays current session cost and projected total
- **Hourly burn rate** - shows both tokens/minute and $/hour rates
- **Cost projections** - predicts total session cost based on current usage

### Token Analytics

- **Detailed token breakdown**:
  - Input tokens
  - Output tokens
  - Cache creation tokens
  - Cache read tokens
- **Model tracking** - shows which Claude models are being used
- **Accurate projections** using data from `ccusage` API

### Customization

- **Flexible reset times** - customize when your token limits reset
- **Timezone support** - set your local timezone for accurate reset times
- **Multiple plan types** with automatic switching when limits exceeded

## Installation

### From crates.io (Recommended)

```bash
cargo install cc-usage-rs
```

### From source

```bash
# Clone the repository
git clone https://github.com/snowmead/cc-usage-rs
cd cc-usage-rs

# Install locally
cargo install --path .

# Or build manually
cargo build --release
# Binary will be at ./target/release/cc-usage-rs
```

## Usage

After installation, the `cc-usage-rs` command will be available in your PATH:

```bash
# Run with default settings (Pro plan)
cc-usage-rs

# Specify a plan
cc-usage-rs --plan max5

# Custom reset hour (e.g., 3 AM)
cc-usage-rs --reset-hour 3

# Different timezone
cc-usage-rs --timezone "America/New_York"
```

## Options

- `--plan` - Claude plan type:
  - `pro` (default) - 7,000 tokens
  - `max5` - 35,000 tokens
  - `max20` - 140,000 tokens
  - `custom-max` - auto-detects based on historical usage
- `--reset-hour` - Custom reset hour (0-23). Default: 4, 9, 14, 18, 23
- `--timezone` - Timezone for reset times. Default: "Europe/Warsaw"

## Display Information

The monitor provides a comprehensive real-time view:

```
✦ ✧ ✦ ✧ CLAUDE TOKEN MONITOR ✦ ✧ ✦ ✧
============================================================

📊 Token Usage:    🟢 [███████████░░░░░░░░░░░░░] 46.3%
⏳ Time to Reset:  ⏰ [█████████████████░░░░░░░] 1h 13m

🎯 Tokens:         3,239 / ~7,000 (3,761 left)
💰 Cost:           $20.65 → $228.24 (projected)
🔥 Burn Rate:      119.3 tokens/min | $45.65/hr
📊 Token Types:    In: 455, Out: 2,784, Cache: 6,943,224

🏁 Predicted End: 21:19
🔄 Token Reset:   18:00
🤖 Model:         claude-opus-4-20250514

⏰ 10:46:39 📝 Smooth sailing... | Ctrl+C to exit
```

### Display Elements:

- **Token Usage Bar**: Visual representation of current usage vs limit
- **Time to Reset Bar**: Shows time remaining until next token reset
- **Token Stats**: Current usage, limit, and remaining tokens
- **Cost Tracking**: Current cost and projected session total
- **Burn Rate**: Token consumption rate and hourly cost
- **Token Breakdown**: Detailed view of different token types
- **Predictions**: When tokens will run out and when they reset
- **Model Info**: Which Claude model(s) are currently being used

### Warnings:

- 🔄 Automatic plan switching notification when exceeding Pro limits
- 🚨 Red alert when tokens exceed maximum limit
- ⚠️ Warning when tokens will run out before reset time

## Requirements

- Rust 1.70+
- `ccusage` command must be installed and available in PATH
- Active Claude Code subscription

## How It Works

The monitor polls the `ccusage blocks --json` command every 3 seconds to get real-time usage data. It processes the JSON response to extract:

- Active session information
- Token counts by type
- Cost data
- Burn rates and projections
- Model usage

The tool intelligently handles:

- Null values in the API response
- Multiple Claude instances running simultaneously
- Automatic plan detection and switching
- Time zone conversions for reset times

## Troubleshooting

If you see "Error running ccusage":

1. Ensure `ccusage` is installed: `npm install -g ccusage`
2. Check that `ccusage` is in your PATH
3. Verify you're logged into Claude Code
4. Run `ccusage blocks --json` manually to test

## Star History

<a href="https://www.star-history.com/#ryoppippi/ccusage&Date">
    <picture>
        <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=ryoppippi/ccusage&type=Date&theme=dark" />
        <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=ryoppippi/ccusage&type=Date" />
        <img alt="Star History Chart" src="https://api.star-history.com/svg?repos=ryoppippi/ccusage&type=Date" />
    </picture>
</a>

## License

MIT
