//! # Session Analysis Module
//!
//! Analyzes session boundaries and patterns from JSONL data
//!
//! ## Key Components
//! - [`SessionBoundary`] - Represents session start/end events
//! - [`detect_session_boundaries`] - Identify session transitions
//! - [`analyze_session_patterns`] - Extract usage patterns

use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;

use crate::jsonl_parser::{SessionData, SessionEntry};

#[derive(Debug, Clone)]
pub struct SessionBoundary {
    pub session_id: String,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub end_reason: SessionEndReason,
    pub total_weighted_tokens: u64,
    pub duration_minutes: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SessionEndReason {
    UserStopped,
    Timeout,
    LimitReached(String), // opus or general
    SystemError,
    Unknown,
}

#[derive(Debug)]
pub struct SessionPattern {
    pub avg_tokens_per_minute: f64,
    pub avg_session_duration: f64,
    pub limit_hit_rate: f64,
    pub model_distribution: HashMap<String, f64>,
}

impl SessionBoundary {
    pub fn tokens_per_minute(&self) -> f64 {
        if self.duration_minutes > 0.0 {
            self.total_weighted_tokens as f64 / self.duration_minutes
        } else {
            0.0
        }
    }
}

pub fn detect_session_end_reason(
    entries: &[SessionEntry],
    has_limit_error: bool,
) -> SessionEndReason {
    if has_limit_error {
        // TODO: Parse specific limit type from error message
        return SessionEndReason::LimitReached("general".to_string());
    }
    
    // Check for explicit user stop patterns
    for entry in entries.iter().rev().take(5) {
        if let Some(message) = &entry.message {
            if let Some(content) = &message.content {
                if let Some(text) = content.as_array()
                    .and_then(|arr| arr.first())
                    .and_then(|v| v.get("text"))
                    .and_then(|t| t.as_str())
                {
                    if text.contains("API Error") {
                        return SessionEndReason::SystemError;
                    }
                }
            }
        }
    }
    
    // Check time gap to next session to detect timeout
    // This would require analyzing multiple sessions
    SessionEndReason::Unknown
}

pub fn analyze_session_gaps(sessions: &[SessionData]) -> Vec<Duration> {
    let mut gaps = Vec::new();
    
    for window in sessions.windows(2) {
        if let (Some(end1), start2) = (window[0].end_time, window[1].start_time) {
            let gap = start2 - end1;
            gaps.push(gap);
        }
    }
    
    gaps
}

pub fn calculate_session_patterns(sessions: &[SessionData]) -> SessionPattern {
    let mut total_duration = 0.0;
    let mut total_tokens = 0u64;
    let mut limit_hits = 0;
    let mut model_counts: HashMap<String, u64> = HashMap::new();
    
    for session in sessions {
        if let Some(end_time) = session.end_time {
            let duration = (end_time - session.start_time).num_minutes() as f64;
            total_duration += duration;
            total_tokens += session.total_weighted_tokens;
            
            if session.has_limit_error {
                limit_hits += 1;
            }
            
            for (model, usage) in &session.model_usage {
                *model_counts.entry(model.clone()).or_insert(0) += usage.message_count as u64;
            }
        }
    }
    
    let avg_tokens_per_minute = if total_duration > 0.0 {
        total_tokens as f64 / total_duration
    } else {
        0.0
    };
    
    let avg_session_duration = if !sessions.is_empty() {
        total_duration / sessions.len() as f64
    } else {
        0.0
    };
    
    let limit_hit_rate = if !sessions.is_empty() {
        limit_hits as f64 / sessions.len() as f64
    } else {
        0.0
    };
    
    let total_model_messages: u64 = model_counts.values().sum();
    let model_distribution = model_counts.into_iter()
        .map(|(model, count)| {
            let percentage = if total_model_messages > 0 {
                count as f64 / total_model_messages as f64
            } else {
                0.0
            };
            (model, percentage)
        })
        .collect();
    
    SessionPattern {
        avg_tokens_per_minute,
        avg_session_duration,
        limit_hit_rate,
        model_distribution,
    }
}

// Constants for session timeout detection
pub const SESSION_TIMEOUT_MINUTES: i64 = 30;
pub const BLOCK_DURATION_HOURS: i64 = 5;

pub fn is_session_timeout(gap: &Duration) -> bool {
    gap.num_minutes() > SESSION_TIMEOUT_MINUTES
}

pub fn is_new_block(gap: &Duration) -> bool {
    gap.num_hours() >= BLOCK_DURATION_HOURS
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_session_boundary() {
        let start = Utc::now();
        let end = start + Duration::minutes(30);
        
        let boundary = SessionBoundary {
            session_id: "test".to_string(),
            start_time: start,
            end_time: end,
            end_reason: SessionEndReason::UserStopped,
            total_weighted_tokens: 15000,
            duration_minutes: 30.0,
        };
        
        assert_eq!(boundary.tokens_per_minute(), 500.0);
    }
    
    #[test]
    fn test_timeout_detection() {
        assert!(is_session_timeout(&Duration::minutes(45)));
        assert!(!is_session_timeout(&Duration::minutes(15)));
    }
}