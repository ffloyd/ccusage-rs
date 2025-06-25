//! # Context Window Predictor Module
//!
//! Predicts when context window will be exhausted based on weighted token consumption
//!
//! ## Key Components
//! - [`ContextPredictor`] - Main prediction engine
//! - [`predict_exhaustion`] - Calculate time until limit
//! - [`adjust_for_plan`] - Account for plan-specific limits

use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;

use crate::models::get_model_config;

#[derive(Debug)]
pub struct ContextPredictor {
    pub current_weighted_tokens: u64,
    pub context_limit: u64,
    pub burn_rate_per_minute: f64,
    pub model_mix: HashMap<String, f64>, // Model name -> percentage of usage
}

#[derive(Debug)]
pub struct PredictionResult {
    pub minutes_remaining: f64,
    pub predicted_exhaustion_time: DateTime<Utc>,
    pub confidence: f64, // 0.0 to 1.0
    pub limiting_factor: LimitingFactor,
}

#[derive(Debug, PartialEq)]
pub enum LimitingFactor {
    ContextWindow,
    OpusLimit,
    GeneralLimit,
    TimeReset,
}

impl ContextPredictor {
    pub fn new(
        current_weighted_tokens: u64,
        context_limit: u64,
        model_breakdown: &HashMap<String, u64>,
    ) -> Self {
        let total_tokens: u64 = model_breakdown.values().sum();
        let model_mix = model_breakdown
            .iter()
            .map(|(model, tokens)| {
                let percentage = if total_tokens > 0 {
                    *tokens as f64 / total_tokens as f64
                } else {
                    0.0
                };
                (model.clone(), percentage)
            })
            .collect();

        Self {
            current_weighted_tokens,
            context_limit,
            burn_rate_per_minute: 0.0,
            model_mix,
        }
    }

    pub fn set_burn_rate(&mut self, raw_tokens_per_minute: f64) {
        // Calculate weighted burn rate based on model mix
        self.burn_rate_per_minute = self.model_mix
            .iter()
            .map(|(model, percentage)| {
                let multiplier = get_model_config(model)
                    .map(|c| c.consumption_multiplier)
                    .unwrap_or(1.0);
                raw_tokens_per_minute * percentage * multiplier
            })
            .sum();
    }

    pub fn predict_exhaustion(&self, reset_time: DateTime<Utc>) -> PredictionResult {
        let now = Utc::now();
        let minutes_to_reset = (reset_time - now).num_minutes() as f64;
        
        // Calculate remaining weighted tokens
        let remaining_tokens = self.context_limit.saturating_sub(self.current_weighted_tokens);
        
        // Calculate time to exhaustion
        let minutes_to_exhaustion = if self.burn_rate_per_minute > 0.0 {
            remaining_tokens as f64 / self.burn_rate_per_minute
        } else {
            f64::INFINITY
        };

        // Determine limiting factor
        let (minutes_remaining, limiting_factor) = if minutes_to_exhaustion < minutes_to_reset {
            (minutes_to_exhaustion, LimitingFactor::ContextWindow)
        } else {
            (minutes_to_reset, LimitingFactor::TimeReset)
        };

        // Check for Opus-specific limits (20% rule for max5 plan)
        let limiting_factor = if self.is_opus_limited() {
            LimitingFactor::OpusLimit
        } else {
            limiting_factor
        };

        let predicted_exhaustion_time = now + Duration::minutes(minutes_remaining as i64);
        
        // Calculate confidence based on data quality
        let confidence = self.calculate_confidence();

        PredictionResult {
            minutes_remaining,
            predicted_exhaustion_time,
            confidence,
            limiting_factor,
        }
    }

    fn is_opus_limited(&self) -> bool {
        // Check if Opus usage is approaching 20% limit (for max5 plan)
        if let Some(opus_percentage) = self.model_mix.iter()
            .find(|(model, _)| model.contains("opus"))
            .map(|(_, pct)| pct)
        {
            // If Opus is being used heavily and we're on max5 plan
            opus_percentage > &0.15 && self.context_limit == 35000
        } else {
            false
        }
    }

    fn calculate_confidence(&self) -> f64 {
        let mut confidence = 1.0;
        
        // Lower confidence if burn rate is very low (not enough data)
        if self.burn_rate_per_minute < 10.0 {
            confidence *= 0.7;
        }
        
        // Lower confidence if model mix is uncertain
        if self.model_mix.is_empty() {
            confidence *= 0.5;
        }
        
        // Lower confidence for very new sessions
        if self.current_weighted_tokens < 1000 {
            confidence *= 0.8;
        }
        
        confidence
    }
}

pub fn adjust_limit_for_plan(base_limit: u64, plan: &str, model_mix: &HashMap<String, f64>) -> u64 {
    match plan {
        "max5" => {
            // Max5 has complex 20:80 split rules
            if let Some(opus_pct) = model_mix.iter()
                .find(|(model, _)| model.contains("opus"))
                .map(|(_, pct)| pct)
            {
                if opus_pct > &0.2 {
                    // If using more than 20% Opus, effective limit is reduced
                    (base_limit as f64 * 0.8) as u64
                } else {
                    base_limit
                }
            } else {
                base_limit
            }
        }
        _ => base_limit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_prediction_with_high_opus_usage() {
        let mut model_breakdown = HashMap::new();
        model_breakdown.insert("claude-opus-4-20250514".to_string(), 8000);
        model_breakdown.insert("claude-sonnet-4-20250514".to_string(), 2000);
        
        let mut predictor = ContextPredictor::new(10000, 35000, &model_breakdown);
        predictor.set_burn_rate(100.0); // 100 raw tokens per minute
        
        // With 80% Opus usage, weighted burn rate should be ~420 tokens/min
        // (80 * 5.0 + 20 * 1.0)
        assert!(predictor.burn_rate_per_minute > 400.0);
        assert!(predictor.burn_rate_per_minute < 440.0);
    }
    
    #[test]
    fn test_opus_limit_detection() {
        let mut model_breakdown = HashMap::new();
        model_breakdown.insert("claude-opus-4-20250514".to_string(), 200);
        model_breakdown.insert("claude-sonnet-4-20250514".to_string(), 800);
        
        let predictor = ContextPredictor::new(1000, 35000, &model_breakdown);
        assert!(predictor.is_opus_limited()); // 20% Opus on max5
    }
}