//! # Analytics Module
//!
//! Advanced analytics for burn rate calculations and usage projections
//!
//! ## Key Components
//! - [`BurnRateAnalyzer`] - Calculate usage velocity over time windows
//! - [`ProjectionEngine`] - Predict token exhaustion and costs
//! - [`UsagePredictor`] - Statistical prediction algorithms

use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;

use crate::block_builder::{Block, BurnRate, Projection};
use crate::jsonl_parser::SessionData;

#[derive(Debug, Clone)]
pub struct BurnRateAnalyzer {
    time_window_minutes: i64,
    min_data_points: usize,
}

impl BurnRateAnalyzer {
    pub fn new() -> Self {
        Self {
            time_window_minutes: 60, // 1 hour window for burn rate calculation
            min_data_points: 2,      // Minimum sessions needed for rate calculation
        }
    }

    pub fn calculate_burn_rate(&self, sessions: &[SessionData], current_time: DateTime<Utc>) -> Option<BurnRate> {
        if sessions.len() < self.min_data_points {
            return None;
        }

        // Filter sessions within the time window
        let window_start = current_time - Duration::minutes(self.time_window_minutes);
        let recent_sessions: Vec<_> = sessions.iter()
            .filter(|s| s.start_time >= window_start)
            .collect();

        if recent_sessions.len() < self.min_data_points {
            return None;
        }

        // Calculate total tokens and time span
        let total_tokens: u64 = recent_sessions.iter()
            .map(|s| s.total_weighted_tokens)
            .sum();

        let earliest_time = recent_sessions.iter()
            .map(|s| s.start_time)
            .min()?;

        let latest_time = recent_sessions.iter()
            .filter_map(|s| s.end_time)
            .max()
            .unwrap_or(current_time);

        let duration_minutes = (latest_time - earliest_time).num_minutes() as f64;
        
        if duration_minutes <= 0.0 {
            return None;
        }

        let tokens_per_minute = total_tokens as f64 / duration_minutes;
        
        // Calculate cost per hour based on recent usage
        let total_cost = recent_sessions.iter()
            .map(|s| crate::pricing::calculate_session_cost(&s.model_usage))
            .sum::<f64>();

        let cost_per_hour = if duration_minutes > 0.0 {
            total_cost * (60.0 / duration_minutes)
        } else {
            0.0
        };

        Some(BurnRate {
            tokens_per_minute,
            cost_per_hour,
        })
    }

    pub fn calculate_weighted_burn_rate(&self, sessions: &[SessionData], current_time: DateTime<Utc>) -> Option<f64> {
        if sessions.is_empty() {
            return None;
        }

        let window_start = current_time - Duration::minutes(self.time_window_minutes);
        let recent_sessions: Vec<_> = sessions.iter()
            .filter(|s| s.start_time >= window_start)
            .collect();

        if recent_sessions.is_empty() {
            return None;
        }

        // Use exponential weighting - more recent sessions have higher weight
        let mut weighted_tokens = 0.0;
        let mut total_weight = 0.0;
        let mut total_duration = 0.0;

        for session in &recent_sessions {
            let age_minutes = (current_time - session.start_time).num_minutes() as f64;
            let weight = (-age_minutes / 30.0).exp(); // Decay with 30-minute half-life
            
            if let Some(end_time) = session.end_time {
                let session_duration = (end_time - session.start_time).num_minutes() as f64;
                if session_duration > 0.0 {
                    weighted_tokens += session.total_weighted_tokens as f64 * weight;
                    total_weight += weight;
                    total_duration += session_duration * weight;
                }
            }
        }

        if total_weight > 0.0 && total_duration > 0.0 {
            Some(weighted_tokens / total_duration) // Weighted tokens per minute
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProjectionEngine {
    confidence_threshold: f64,
    projection_window_hours: i64,
}

impl ProjectionEngine {
    pub fn new() -> Self {
        Self {
            confidence_threshold: 0.7, // Minimum confidence for projections
            projection_window_hours: 24, // Project up to 24 hours ahead
        }
    }

    pub fn calculate_projection(
        &self,
        current_tokens: u64,
        token_limit: u64,
        burn_rate: &BurnRate,
        current_cost: f64,
        _current_time: DateTime<Utc>,
    ) -> Option<Projection> {
        if burn_rate.tokens_per_minute <= 0.0 {
            return None;
        }

        let tokens_remaining = token_limit.saturating_sub(current_tokens);
        let minutes_remaining = tokens_remaining as f64 / burn_rate.tokens_per_minute;

        // Don't project beyond our window
        let max_minutes = self.projection_window_hours as f64 * 60.0;
        if minutes_remaining > max_minutes {
            return None;
        }

        // Calculate projected totals
        let projected_total_tokens = current_tokens + (burn_rate.tokens_per_minute * minutes_remaining) as u64;
        let projected_additional_cost = burn_rate.cost_per_hour * (minutes_remaining / 60.0);
        let projected_total_cost = current_cost + projected_additional_cost;

        Some(Projection {
            total_tokens: projected_total_tokens,
            total_cost: projected_total_cost,
            remaining_minutes: minutes_remaining,
        })
    }

    pub fn predict_exhaustion_time(
        &self,
        current_tokens: u64,
        token_limit: u64,
        burn_rate: f64,
        current_time: DateTime<Utc>,
    ) -> Option<DateTime<Utc>> {
        if burn_rate <= 0.0 {
            return None;
        }

        let tokens_remaining = token_limit.saturating_sub(current_tokens);
        let minutes_to_exhaustion = tokens_remaining as f64 / burn_rate;

        // Only predict if within reasonable time frame
        if minutes_to_exhaustion > 0.0 && minutes_to_exhaustion < (self.projection_window_hours as f64 * 60.0) {
            Some(current_time + Duration::minutes(minutes_to_exhaustion as i64))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct UsagePredictor {
    analyzer: BurnRateAnalyzer,
    projector: ProjectionEngine,
}

impl UsagePredictor {
    pub fn new() -> Self {
        Self {
            analyzer: BurnRateAnalyzer::new(),
            projector: ProjectionEngine::new(),
        }
    }

    pub fn predict_block_completion(
        &self,
        block: &Block,
        sessions: &[SessionData],
        token_limit: u64,
        _reset_time: DateTime<Utc>,
    ) -> Option<DateTime<Utc>> {
        let current_time = Utc::now();
        
        // Use the block's burn rate if available, otherwise calculate from sessions
        let burn_rate_per_minute = if let Some(burn_rate) = &block.burn_rate {
            burn_rate.tokens_per_minute
        } else {
            self.analyzer.calculate_weighted_burn_rate(sessions, current_time)?
        };

        self.projector.predict_exhaustion_time(
            block.total_tokens,
            token_limit,
            burn_rate_per_minute,
            current_time,
        )
    }

    pub fn analyze_usage_pattern(&self, sessions: &[SessionData]) -> UsagePattern {
        let total_sessions = sessions.len();
        let total_weighted_tokens: u64 = sessions.iter().map(|s| s.total_weighted_tokens).sum();
        
        // Calculate model distribution
        let mut model_tokens: HashMap<String, u64> = HashMap::new();
        for session in sessions {
            for (model, usage) in &session.model_usage {
                *model_tokens.entry(model.clone()).or_default() += usage.total_input + usage.total_output;
            }
        }

        // Find dominant model
        let dominant_model = model_tokens.iter()
            .max_by_key(|(_, tokens)| *tokens)
            .map(|(model, _)| model.clone());

        // Calculate session frequency
        let time_span = if sessions.len() > 1 {
            let earliest = sessions.iter().map(|s| s.start_time).min().unwrap();
            let latest = sessions.iter().map(|s| s.start_time).max().unwrap();
            (latest - earliest).num_hours() as f64
        } else {
            1.0
        };

        let sessions_per_hour = total_sessions as f64 / time_span.max(1.0);

        // Check for limit errors
        let has_limit_errors = sessions.iter().any(|s| s.has_limit_error);

        UsagePattern {
            total_sessions,
            total_weighted_tokens,
            model_distribution: model_tokens,
            dominant_model,
            sessions_per_hour,
            has_limit_errors,
            average_session_tokens: if total_sessions > 0 {
                total_weighted_tokens / total_sessions as u64
            } else {
                0
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct UsagePattern {
    pub total_sessions: usize,
    pub total_weighted_tokens: u64,
    pub model_distribution: HashMap<String, u64>,
    pub dominant_model: Option<String>,
    pub sessions_per_hour: f64,
    pub has_limit_errors: bool,
    pub average_session_tokens: u64,
}

impl UsagePattern {
    pub fn is_heavy_usage(&self) -> bool {
        self.sessions_per_hour > 2.0 || self.average_session_tokens > 5000
    }

    pub fn is_opus_heavy(&self) -> bool {
        if let Some(ref dominant) = self.dominant_model {
            return dominant.contains("opus");
        }
        
        // Check if Opus usage is significant
        let opus_tokens: u64 = self.model_distribution.iter()
            .filter(|(model, _)| model.contains("opus"))
            .map(|(_, &tokens)| tokens)
            .sum();

        if self.total_weighted_tokens > 0 {
            let opus_percentage = opus_tokens as f64 / self.total_weighted_tokens as f64;
            opus_percentage > 0.3 // More than 30% Opus usage
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jsonl_parser::{SessionData, ModelUsage};
    use std::collections::HashMap;

    fn create_test_session(minutes_ago: i64, tokens: u64, model: &str) -> SessionData {
        let start_time = Utc::now() - Duration::minutes(minutes_ago);
        let end_time = start_time + Duration::minutes(10);

        let mut model_usage = HashMap::new();
        model_usage.insert(model.to_string(), ModelUsage {
            model_name: model.to_string(),
            total_input: tokens / 2,
            total_output: tokens / 2,
            total_cache_write: 0,
            total_cache_read: 0,
            message_count: 1,
            weighted_tokens: tokens,
        });

        SessionData {
            session_id: format!("test_{}", minutes_ago),
            start_time,
            end_time: Some(end_time),
            model_usage,
            total_weighted_tokens: tokens,
            has_limit_error: false,
            limit_type: None,
        }
    }

    #[test]
    fn test_burn_rate_calculation() {
        let analyzer = BurnRateAnalyzer::new();
        let sessions = vec![
            create_test_session(30, 1000, "claude-3-5-sonnet"),
            create_test_session(20, 800, "claude-3-5-sonnet"),
        ];

        let burn_rate = analyzer.calculate_burn_rate(&sessions, Utc::now());
        assert!(burn_rate.is_some());
        
        let rate = burn_rate.unwrap();
        assert!(rate.tokens_per_minute > 0.0);
        assert!(rate.cost_per_hour > 0.0);
    }

    #[test]
    fn test_projection_calculation() {
        let projector = ProjectionEngine::new();
        let burn_rate = BurnRate {
            tokens_per_minute: 100.0,
            cost_per_hour: 10.0,
        };

        let projection = projector.calculate_projection(
            5000, // current tokens
            7000, // limit
            &burn_rate,
            5.0, // current cost
            Utc::now(),
        );

        assert!(projection.is_some());
        let proj = projection.unwrap();
        assert_eq!(proj.remaining_minutes, 20.0); // (7000-5000)/100 = 20 minutes
    }

    #[test]
    fn test_usage_pattern_analysis() {
        let predictor = UsagePredictor::new();
        let sessions = vec![
            create_test_session(60, 2000, "claude-3-opus"),
            create_test_session(30, 1500, "claude-3-opus"),
            create_test_session(10, 800, "claude-3-5-sonnet"),
        ];

        let pattern = predictor.analyze_usage_pattern(&sessions);
        assert_eq!(pattern.total_sessions, 3);
        assert_eq!(pattern.total_weighted_tokens, 4300);
        assert!(pattern.is_opus_heavy());
    }

    #[test]
    fn test_exhaustion_prediction() {
        let projector = ProjectionEngine::new();
        let current_time = Utc::now();
        
        let exhaustion_time = projector.predict_exhaustion_time(
            6000, // current tokens
            7000, // limit
            50.0, // burn rate tokens/min
            current_time,
        );

        assert!(exhaustion_time.is_some());
        let predicted = exhaustion_time.unwrap();
        let expected = current_time + Duration::minutes(20); // (7000-6000)/50 = 20 minutes
        
        // Allow for small timing differences in test
        assert!((predicted - expected).num_seconds().abs() < 60);
    }
}