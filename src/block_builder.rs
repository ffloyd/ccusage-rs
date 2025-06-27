//! # Block Builder Module
//!
//! Converts Claude session data into time-based usage blocks equivalent to ccusage output
//!
//! ## Key Components
//! - [`build_blocks_from_sessions`] - Main conversion function
//! - [`BlockBuilder`] - Core block building logic
//! - [`detect_gaps`] - Identify time gaps between sessions

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;

use crate::jsonl_parser::SessionData;
use crate::pricing::{calculate_session_cost, calculate_cost_per_hour};

// Re-export main types from main.rs to avoid circular dependencies
#[derive(Debug, Clone, Default)]
pub struct TokenCounts {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub cache_read_input_tokens: u64,
}

#[derive(Debug, Clone, Default)]
pub struct BurnRate {
    pub tokens_per_minute: f64,
    pub cost_per_hour: f64,
}

#[derive(Debug, Clone, Default)]
pub struct Projection {
    pub total_tokens: u64,
    pub total_cost: f64,
    pub remaining_minutes: f64,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub id: String,
    pub start_time: String,
    pub end_time: String,
    pub actual_end_time: Option<String>,
    pub is_active: bool,
    pub is_gap: bool,
    pub entries: u64,
    pub token_counts: TokenCounts,
    pub total_tokens: u64,
    pub cost_usd: f64,
    pub models: Vec<String>,
    pub burn_rate: Option<BurnRate>,
    pub projection: Option<Projection>,
    pub model_breakdown: Option<HashMap<String, TokenCounts>>,
    pub weighted_total_tokens: Option<u64>,
    pub context_consumption_rate: Option<f64>,
}

impl Block {
    fn new(id: String, start_time: DateTime<Utc>) -> Self {
        Self {
            id,
            start_time: start_time.to_rfc3339(),
            end_time: String::new(),
            actual_end_time: None,
            is_active: false,
            is_gap: false,
            entries: 0,
            token_counts: TokenCounts::default(),
            total_tokens: 0,
            cost_usd: 0.0,
            models: Vec::new(),
            burn_rate: None,
            projection: None,
            model_breakdown: None,
            weighted_total_tokens: None,
            context_consumption_rate: None,
        }
    }

    fn add_session(&mut self, session: &SessionData) {
        self.entries += 1;
        
        // Update model breakdown and token counts
        let mut model_breakdown = self.model_breakdown.take().unwrap_or_default();
        let mut models_used = Vec::new();

        for (model_name, usage) in &session.model_usage {
            // Update model breakdown
            let counts = model_breakdown.entry(model_name.clone()).or_default();
            counts.input_tokens += usage.total_input;
            counts.output_tokens += usage.total_output;
            counts.cache_creation_input_tokens += usage.total_cache_write;
            counts.cache_read_input_tokens += usage.total_cache_read;

            // Update block totals
            self.token_counts.input_tokens += usage.total_input;
            self.token_counts.output_tokens += usage.total_output;
            self.token_counts.cache_creation_input_tokens += usage.total_cache_write;
            self.token_counts.cache_read_input_tokens += usage.total_cache_read;

            models_used.push(model_name.clone());
        }

        // Update models list (deduplicate)
        for model in models_used {
            if !self.models.contains(&model) {
                self.models.push(model);
            }
        }

        // Update totals
        self.total_tokens = self.token_counts.input_tokens + self.token_counts.output_tokens;
        self.weighted_total_tokens = Some(session.total_weighted_tokens);
        self.cost_usd += calculate_session_cost(&session.model_usage);
        self.model_breakdown = Some(model_breakdown);

        // Update timing
        if let Some(end_time) = session.end_time {
            self.actual_end_time = Some(end_time.to_rfc3339());
        }

        // Calculate context consumption rate
        if self.total_tokens > 0 {
            if let Some(weighted) = self.weighted_total_tokens {
                self.context_consumption_rate = Some(weighted as f64 / self.total_tokens as f64);
            }
        }
    }

    fn calculate_burn_rate(&mut self) {
        if let (Some(actual_end), start_time) = (&self.actual_end_time, &self.start_time) {
            if let (Ok(start), Ok(end)) = (
                DateTime::parse_from_rfc3339(start_time),
                DateTime::parse_from_rfc3339(actual_end)
            ) {
                let duration_minutes = (end - start).num_minutes() as f64;
                if duration_minutes > 0.0 {
                    let tokens_per_minute = self.total_tokens as f64 / duration_minutes;
                    let cost_per_hour = calculate_cost_per_hour(self.cost_usd, duration_minutes);

                    self.burn_rate = Some(BurnRate {
                        tokens_per_minute,
                        cost_per_hour,
                    });
                }
            }
        }
    }

    fn finalize(&mut self, end_time: DateTime<Utc>) {
        if self.actual_end_time.is_none() {
            self.end_time = end_time.to_rfc3339();
        } else {
            self.end_time = self.actual_end_time.as_ref().unwrap().clone();
        }
        
        self.calculate_burn_rate();
    }
}

pub struct BlockBuilder {
    blocks: Vec<Block>,
    current_block: Option<Block>,
    block_duration_hours: i64,
    gap_threshold_minutes: i64,
}

impl BlockBuilder {
    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            current_block: None,
            block_duration_hours: 5, // 5-hour blocks like ccusage
            gap_threshold_minutes: 30, // 30 minute gap detection
        }
    }

    pub fn add_session(&mut self, session: &SessionData) -> Result<()> {
        let session_start = session.start_time;

        // Check if we need to start a new block
        let should_start_new_block = match &self.current_block {
            None => true,
            Some(current) => {
                let current_start = DateTime::parse_from_rfc3339(&current.start_time)?
                    .with_timezone(&Utc);
                let time_diff = session_start - current_start;
                
                // Start new block if session is too far from current block start
                time_diff > Duration::hours(self.block_duration_hours)
            }
        };

        if should_start_new_block {
            // Finalize current block if it exists
            if let Some(mut current) = self.current_block.take() {
                current.finalize(session_start);
                self.blocks.push(current);
            }

            // Start new block
            let block_id = format!("block_{}", self.blocks.len() + 1);
            self.current_block = Some(Block::new(block_id, session_start));
        }

        // Add session to current block
        if let Some(ref mut current) = self.current_block {
            current.add_session(session);
        }

        Ok(())
    }

    pub fn finalize(mut self, current_time: DateTime<Utc>) -> Vec<Block> {
        // Finalize the current block
        if let Some(mut current) = self.current_block.take() {
            current.finalize(current_time);
            self.blocks.push(current);
        }

        // Detect and insert gap blocks
        self.insert_gap_blocks();

        // Mark the most recent non-gap block as active if it's recent enough
        self.mark_active_block(current_time);

        self.blocks
    }

    fn mark_active_block(&mut self, current_time: DateTime<Utc>) {
        // Find the most recent non-gap block
        let mut most_recent_block_idx = None;
        let mut most_recent_time = None;

        for (i, block) in self.blocks.iter().enumerate() {
            if !block.is_gap {
                if let Ok(block_start) = DateTime::parse_from_rfc3339(&block.start_time) {
                    let block_start_utc = block_start.with_timezone(&Utc);
                    if most_recent_time.is_none() || block_start_utc > most_recent_time.unwrap() {
                        most_recent_time = Some(block_start_utc);
                        most_recent_block_idx = Some(i);
                    }
                }
            }
        }

        // Mark the most recent block as active if it's within the last 6 hours
        if let (Some(idx), Some(recent_time)) = (most_recent_block_idx, most_recent_time) {
            let time_since = current_time - recent_time;
            if time_since <= Duration::hours(6) {
                self.blocks[idx].is_active = true;
            }
        }
    }

    fn insert_gap_blocks(&mut self) {
        let original_blocks = std::mem::take(&mut self.blocks);
        let mut blocks_with_gaps = Vec::new();
        
        for i in 0..original_blocks.len() {
            let block = original_blocks[i].clone();
            blocks_with_gaps.push(block.clone());

            // Check if there's a gap to the next block
            if let Some(next_block) = original_blocks.get(i + 1) {
                if let (Ok(current_end), Ok(next_start)) = (
                    DateTime::parse_from_rfc3339(&block.end_time),
                    DateTime::parse_from_rfc3339(&next_block.start_time)
                ) {
                    let gap_duration = next_start - current_end;
                    if gap_duration > Duration::minutes(self.gap_threshold_minutes) {
                        // Create gap block
                        let gap_id = format!("gap_{}", blocks_with_gaps.len());
                        let mut gap_block = Block::new(gap_id, current_end.with_timezone(&Utc));
                        gap_block.is_gap = true;
                        gap_block.finalize(next_start.with_timezone(&Utc));
                        blocks_with_gaps.push(gap_block);
                    }
                }
            }
        }

        self.blocks = blocks_with_gaps;
    }
}

pub fn build_blocks_from_sessions(sessions: &[SessionData]) -> Result<Vec<Block>> {
    let mut builder = BlockBuilder::new();

    // Sort sessions by start time
    let mut session_refs: Vec<_> = sessions.iter().collect();
    session_refs.sort_by_key(|s| s.start_time);

    // Process each session
    for session in session_refs {
        builder.add_session(session)?;
    }

    // Finalize with current time
    let current_time = Utc::now();
    Ok(builder.finalize(current_time))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jsonl_parser::{SessionData, ModelUsage};
    use std::collections::HashMap;

    fn create_test_session(
        id: &str,
        start_minutes_ago: i64,
        duration_minutes: i64,
        tokens: u64,
    ) -> SessionData {
        let start_time = Utc::now() - Duration::minutes(start_minutes_ago);
        let end_time = start_time + Duration::minutes(duration_minutes);

        let mut model_usage = HashMap::new();
        model_usage.insert("claude-3-5-sonnet".to_string(), ModelUsage {
            model_name: "claude-3-5-sonnet".to_string(),
            total_input: tokens / 2,
            total_output: tokens / 2,
            total_cache_write: 0,
            total_cache_read: 0,
            message_count: 1,
            weighted_tokens: tokens,
        });

        SessionData {
            session_id: id.to_string(),
            start_time,
            end_time: Some(end_time),
            model_usage,
            total_weighted_tokens: tokens,
            has_limit_error: false,
            _limit_type: None,
        }
    }

    #[test]
    fn test_single_session_block() {
        let sessions = vec![create_test_session("test1", 30, 15, 1000)];
        let blocks = build_blocks_from_sessions(&sessions).unwrap();

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].total_tokens, 1000);
        assert_eq!(blocks[0].entries, 1);
        assert!(blocks[0].is_active);
        assert!(!blocks[0].is_gap);
    }

    #[test]
    fn test_multiple_sessions_same_block() {
        let sessions = vec![
            create_test_session("test1", 120, 15, 500),
            create_test_session("test2", 90, 20, 750),
            create_test_session("test3", 60, 10, 250),
        ];
        let blocks = build_blocks_from_sessions(&sessions).unwrap();

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].total_tokens, 1500);
        assert_eq!(blocks[0].entries, 3);
    }

    #[test]
    fn test_sessions_requiring_multiple_blocks() {
        let sessions = vec![
            create_test_session("test1", 400, 15, 500), // 6+ hours ago
            create_test_session("test2", 60, 20, 750),  // 1 hour ago
        ];
        let blocks = build_blocks_from_sessions(&sessions).unwrap();

        // The sessions are 400-60 = 340 minutes apart (5.67 hours), which is > 5 hours
        // So they should be in separate blocks with a gap block in between
        assert_eq!(blocks.len(), 3); // 2 data blocks + 1 gap block
        
        // Filter out gap blocks to check data blocks
        let data_blocks: Vec<_> = blocks.iter().filter(|b| !b.is_gap).collect();
        assert_eq!(data_blocks.len(), 2);
        assert_eq!(data_blocks[0].total_tokens, 500);
        assert_eq!(data_blocks[1].total_tokens, 750);
        assert!(data_blocks[1].is_active); // Most recent block is active
        
        // Check that middle block is a gap
        assert!(blocks[1].is_gap);
    }

    #[test]
    fn test_burn_rate_calculation() {
        let sessions = vec![create_test_session("test1", 60, 30, 1800)]; // 1800 tokens in 30 minutes
        let blocks = build_blocks_from_sessions(&sessions).unwrap();

        assert_eq!(blocks.len(), 1);
        let burn_rate = blocks[0].burn_rate.as_ref().unwrap();
        assert_eq!(burn_rate.tokens_per_minute, 60.0); // 1800 tokens / 30 minutes
    }
}