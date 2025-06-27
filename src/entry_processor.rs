//! # Entry-Level Processing Module
//!
//! Direct entry processing matching ccusage behavior exactly
//!
//! ## Key Components
//! - [`process_all_entries`] - Process all JSONL entries with global deduplication
//! - [`aggregate_entries_by_date`] - Group and aggregate entries by date

use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::jsonl_parser::{SessionEntry, Usage};
use crate::pricing::calculate_cost_from_tokens;
use crate::table_display::{DailyStats, ModelBreakdown};

#[derive(Debug)]
pub struct ProcessedEntry {
    pub date: String,
    pub model: String,
    pub usage: Usage,
    pub cost: f64,
}

/// Create unique hash for entry deduplication (matching ccusage logic exactly)
fn create_unique_hash(entry: &SessionEntry) -> Option<String> {
    if let Some(message) = &entry.message {
        if let (Some(message_id), Some(request_id)) = (&message.id, &entry.request_id) {
            return Some(format!("{}:{}", message_id, request_id));
        }
    }
    None
}

/// Process all JSONL files with global entry-level deduplication (matching ccusage)
pub fn process_all_entries(session_files: &[std::path::PathBuf]) -> Result<Vec<DailyStats>> {
    let mut global_processed_hashes = HashSet::new();
    let mut all_entries = Vec::new();
    
    // Process files sequentially to maintain global hash consistency (like ccusage)
    for file in session_files {
        if let Err(e) = process_file_entries(file, &mut global_processed_hashes, &mut all_entries) {
            eprintln!("Warning: Failed to process file {}: {}", file.display(), e);
        }
    }
    
    // Group entries by date and aggregate
    aggregate_entries_by_date(all_entries)
}

fn process_file_entries(
    file_path: &Path,
    processed_hashes: &mut HashSet<String>,
    all_entries: &mut Vec<ProcessedEntry>,
) -> Result<()> {
    let file = File::open(file_path).context("Failed to open JSONL file")?;
    let reader = BufReader::new(file);
    
    for line in reader.lines() {
        let line = line.context("Failed to read line")?;
        if line.trim().is_empty() {
            continue;
        }
        
        // Try to parse as SessionEntry
        match serde_json::from_str::<SessionEntry>(&line) {
            Ok(entry) => {
                // Create unique hash for deduplication (matching ccusage logic)
                if let Some(unique_hash) = create_unique_hash(&entry) {
                    if processed_hashes.contains(&unique_hash) {
                        continue; // Skip duplicate entry
                    }
                    processed_hashes.insert(unique_hash);
                }
                
                // Process entry if it has usage data
                if let Some(message) = &entry.message {
                    if let (Some(model), Some(usage)) = (&message.model, &message.usage) {
                        // Skip synthetic models (matching ccusage behavior)
                        if model == "<synthetic>" {
                            continue;
                        }
                        
                        let timestamp = DateTime::parse_from_rfc3339(&entry.timestamp)
                            .context("Failed to parse timestamp")?
                            .with_timezone(&Local);
                        
                        let date = timestamp.format("%Y-%m-%d").to_string();
                        
                        // Calculate cost for this entry (matching our pricing logic)
                        let cost = if let Some(existing_cost) = message.cost_usd {
                            existing_cost
                        } else {
                            // Calculate cost using our pricing model
                            calculate_entry_cost(model, usage)
                        };
                        
                        all_entries.push(ProcessedEntry {
                            date,
                            model: model.clone(),
                            usage: usage.clone(),
                            cost,
                        });
                    }
                }
            }
            Err(_) => {
                // Skip invalid entries (like ccusage does)
                continue;
            }
        }
    }
    
    Ok(())
}

fn aggregate_entries_by_date(entries: Vec<ProcessedEntry>) -> Result<Vec<DailyStats>> {
    let mut daily_map: HashMap<String, DailyStats> = HashMap::new();
    
    for entry in entries {
        let daily_stat = daily_map
            .entry(entry.date.clone())
            .or_insert_with(|| DailyStats {
                date: entry.date.clone(),
                models: Vec::new(),
                input_tokens: 0,
                output_tokens: 0,
                cache_creation_tokens: 0,
                cache_read_tokens: 0,
                total_tokens: 0,
                cost_usd: 0.0,
                model_breakdowns: Vec::new(),
            });
        
        // Add model to list if not already present
        let simplified_model = simplify_model_name(&entry.model);
        if !daily_stat.models.contains(&simplified_model) {
            daily_stat.models.push(simplified_model.clone());
        }
        
        // Update per-model breakdown
        if let Some(breakdown) = daily_stat.model_breakdowns.iter_mut().find(|b| b.model_name == simplified_model) {
            breakdown.input_tokens += entry.usage.input_tokens;
            breakdown.output_tokens += entry.usage.output_tokens;
            breakdown.cache_creation_tokens += entry.usage.cache_creation_input_tokens;
            breakdown.cache_read_tokens += entry.usage.cache_read_input_tokens;
            breakdown.total_tokens += entry.usage.input_tokens 
                + entry.usage.output_tokens 
                + entry.usage.cache_creation_input_tokens 
                + entry.usage.cache_read_input_tokens;
            breakdown.cost_usd += entry.cost;
        } else {
            daily_stat.model_breakdowns.push(ModelBreakdown {
                model_name: simplified_model,
                input_tokens: entry.usage.input_tokens,
                output_tokens: entry.usage.output_tokens,
                cache_creation_tokens: entry.usage.cache_creation_input_tokens,
                cache_read_tokens: entry.usage.cache_read_input_tokens,
                total_tokens: entry.usage.input_tokens 
                    + entry.usage.output_tokens 
                    + entry.usage.cache_creation_input_tokens 
                    + entry.usage.cache_read_input_tokens,
                cost_usd: entry.cost,
            });
        }
        
        // Add token counts to totals
        daily_stat.input_tokens += entry.usage.input_tokens;
        daily_stat.output_tokens += entry.usage.output_tokens;
        daily_stat.cache_creation_tokens += entry.usage.cache_creation_input_tokens;
        daily_stat.cache_read_tokens += entry.usage.cache_read_input_tokens;
        daily_stat.cost_usd += entry.cost;
    }
    
    // Update totals
    for daily_stat in daily_map.values_mut() {
        daily_stat.total_tokens = daily_stat.input_tokens
            + daily_stat.output_tokens
            + daily_stat.cache_creation_tokens
            + daily_stat.cache_read_tokens;
    }
    
    // Convert to sorted vector
    let mut daily_stats: Vec<DailyStats> = daily_map.into_values().collect();
    daily_stats.sort_by(|a, b| a.date.cmp(&b.date));
    
    Ok(daily_stats)
}

fn calculate_entry_cost(model: &str, usage: &Usage) -> f64 {
    calculate_cost_from_tokens(usage, model)
}

fn simplify_model_name(model: &str) -> String {
    if model.contains("opus") {
        "opus-4".to_string()
    } else if model.contains("sonnet") {
        "sonnet-4".to_string()
    } else if model.contains("haiku") {
        "haiku".to_string()
    } else {
        // Extract first two parts separated by dashes
        model.split('-').take(2).collect::<Vec<_>>().join("-")
    }
}