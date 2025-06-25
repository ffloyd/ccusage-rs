//! # Pricing Engine Module
//!
//! Handles cost calculations for Claude API usage based on current Anthropic pricing
//!
//! ## Key Components
//! - [`ModelPricing`] - Pricing structure for different token types
//! - [`calculate_session_cost`] - Calculate total cost for a session
//! - [`get_model_pricing`] - Get pricing configuration for a specific model
//! - [`CostCalculationMode`] - Cost calculation modes matching ccusage

use std::collections::HashMap;
use crate::jsonl_parser::{ModelUsage, Usage, SessionEntry};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CostCalculationMode {
    Auto,       // Use existing costUSD or calculate if missing
    Display,    // Always use existing costUSD 
    Calculate,  // Always recalculate from tokens
}

#[derive(Debug, Clone)]
pub struct ModelPricing {
    pub input_cost_per_token: f64,
    pub output_cost_per_token: f64,
    pub cache_creation_input_token_cost: f64,
    pub cache_read_input_token_cost: f64,
}

impl ModelPricing {
    pub fn calculate_cost(&self, usage: &ModelUsage) -> f64 {
        let input_cost = (usage.total_input as f64) * self.input_cost_per_token;
        let output_cost = (usage.total_output as f64) * self.output_cost_per_token;
        let cache_creation_cost = (usage.total_cache_write as f64) * self.cache_creation_input_token_cost;
        let cache_read_cost = (usage.total_cache_read as f64) * self.cache_read_input_token_cost;
        
        input_cost + output_cost + cache_creation_cost + cache_read_cost
    }
}

pub fn get_model_pricing(model_name: &str) -> Option<ModelPricing> {
    // Official Anthropic API pricing as of June 2025
    // Prices are per million tokens - source: https://www.anthropic.com/pricing
    match model_name {
        // Claude 3.5 Sonnet (latest)
        name if name.contains("claude-3-5-sonnet") || name.contains("claude-sonnet-3-5") => {
            Some(ModelPricing {
                input_cost_per_token: 3e-6,
                output_cost_per_token: 15e-6,
                cache_creation_input_token_cost: 3.75e-6,
                cache_read_input_token_cost: 0.3e-6,
            })
        },
        // Claude 3.5 Haiku (Official Anthropic pricing)
        name if name.contains("claude-3-5-haiku") || name.contains("claude-haiku-3-5") => {
            Some(ModelPricing {
                input_cost_per_token: 0.8e-6,
                output_cost_per_token: 4e-6,
                cache_creation_input_token_cost: 1e-6,
                cache_read_input_token_cost: 0.08e-6,
            })
        },
        // Claude 3 Opus
        name if name.contains("claude-3-opus") || name.contains("claude-opus-3") => {
            Some(ModelPricing {
                input_cost_per_token: 15e-6,
                output_cost_per_token: 75e-6,
                cache_creation_input_token_cost: 18.75e-6,
                cache_read_input_token_cost: 1.5e-6,
            })
        },
        // Claude 3 Sonnet (legacy)
        name if name.contains("claude-3-sonnet") || name.contains("claude-sonnet-3") => {
            Some(ModelPricing {
                input_cost_per_token: 3e-6,
                output_cost_per_token: 15e-6,
                cache_creation_input_token_cost: 3.75e-6,
                cache_read_input_token_cost: 0.3e-6,
            })
        },
        // Claude 3 Haiku (legacy)
        name if name.contains("claude-3-haiku") || name.contains("claude-haiku-3") => {
            Some(ModelPricing {
                input_cost_per_token: 0.25e-6,
                output_cost_per_token: 1.25e-6,
                cache_creation_input_token_cost: 0.31e-6,
                cache_read_input_token_cost: 0.025e-6,
            })
        },
        // Claude 4 Opus (Official Anthropic pricing)
        name if name.contains("claude-opus-4") || name.contains("claude-4-opus") => {
            Some(ModelPricing {
                input_cost_per_token: 15e-6,
                output_cost_per_token: 75e-6,
                cache_creation_input_token_cost: 18.75e-6,
                cache_read_input_token_cost: 1.5e-6,
            })
        },
        // Claude 4 Sonnet (Official Anthropic pricing)
        name if name.contains("claude-sonnet-4") || name.contains("claude-4-sonnet") => {
            Some(ModelPricing {
                input_cost_per_token: 3e-6,
                output_cost_per_token: 15e-6,
                cache_creation_input_token_cost: 3.75e-6,
                cache_read_input_token_cost: 0.3e-6,
            })
        },
        // Default fallback for unknown models - use Sonnet 3.5 pricing
        _ => {
            Some(ModelPricing {
                input_cost_per_token: 3e-6,
                output_cost_per_token: 15e-6,
                cache_creation_input_token_cost: 3.75e-6,
                cache_read_input_token_cost: 0.3e-6,
            })
        }
    }
}

pub fn calculate_session_cost(model_usage: &HashMap<String, ModelUsage>) -> f64 {
    model_usage.iter()
        .filter_map(|(model_name, usage)| {
            get_model_pricing(model_name).map(|pricing| pricing.calculate_cost(usage))
        })
        .sum()
}

/// Calculate cost for a single entry matching ccusage's calculateCostForEntry logic
pub fn calculate_cost_for_entry(
    entry: &SessionEntry,
    mode: CostCalculationMode,
) -> f64 {
    match mode {
        CostCalculationMode::Display => {
            // Always use existing costUSD, default to 0 if missing
            entry.message.as_ref()
                .and_then(|m| m.cost_usd)
                .unwrap_or(0.0)
        }
        CostCalculationMode::Calculate => {
            // Always recalculate from tokens
            if let Some(message) = &entry.message {
                if let (Some(model), Some(usage)) = (&message.model, &message.usage) {
                    calculate_cost_from_tokens(usage, model)
                } else {
                    0.0
                }
            } else {
                0.0
            }
        }
        CostCalculationMode::Auto => {
            // Use existing costUSD if available, otherwise calculate
            if let Some(message) = &entry.message {
                if let Some(existing_cost) = message.cost_usd {
                    existing_cost
                } else if let (Some(model), Some(usage)) = (&message.model, &message.usage) {
                    calculate_cost_from_tokens(usage, model)
                } else {
                    0.0
                }
            } else {
                0.0
            }
        }
    }
}

/// Calculate cost from token usage and model name
pub fn calculate_cost_from_tokens(usage: &Usage, model_name: &str) -> f64 {
    if let Some(pricing) = get_model_pricing(model_name) {
        let input_cost = (usage.input_tokens as f64) * pricing.input_cost_per_token;
        let output_cost = (usage.output_tokens as f64) * pricing.output_cost_per_token;
        let cache_write_cost = (usage.cache_creation_input_tokens as f64) * pricing.cache_creation_input_token_cost;
        let cache_read_cost = (usage.cache_read_input_tokens as f64) * pricing.cache_read_input_token_cost;
        
        input_cost + output_cost + cache_write_cost + cache_read_cost
    } else {
        0.0
    }
}

pub fn calculate_cost_per_hour(total_cost: f64, duration_minutes: f64) -> f64 {
    if duration_minutes <= 0.0 {
        0.0
    } else {
        total_cost * (60.0 / duration_minutes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jsonl_parser::ModelUsage;

    #[test]
    fn test_sonnet_pricing() {
        let pricing = get_model_pricing("claude-3-5-sonnet-20241022").unwrap();
        
        let usage = ModelUsage {
            model_name: "claude-3-5-sonnet-20241022".to_string(),
            total_input: 1_000_000,    // 1M input tokens
            total_output: 500_000,     // 500K output tokens
            total_cache_write: 100_000, // 100K cache write tokens
            total_cache_read: 200_000,  // 200K cache read tokens
            message_count: 10,
            weighted_tokens: 1_500_000,
        };

        let cost = pricing.calculate_cost(&usage);
        
        // Expected: (1M * $3) + (500K * $15) + (100K * $3.75) + (200K * $0.30) = $3 + $7.5 + $0.375 + $0.06 = $10.935
        assert_eq!(cost, 10.935);
    }

    #[test]
    fn test_opus_pricing() {
        let pricing = get_model_pricing("claude-3-opus-20240229").unwrap();
        
        let usage = ModelUsage {
            model_name: "claude-3-opus-20240229".to_string(),
            total_input: 100_000,    // 100K input tokens
            total_output: 50_000,    // 50K output tokens
            total_cache_write: 0,
            total_cache_read: 0,
            message_count: 5,
            weighted_tokens: 750_000, // 5x multiplier
        };

        let cost = pricing.calculate_cost(&usage);
        
        // Expected: (100K * $15) + (50K * $75) = $1.5 + $3.75 = $5.25
        assert_eq!(cost, 5.25);
    }

    #[test]
    fn test_cost_per_hour_calculation() {
        let cost_per_hour = calculate_cost_per_hour(5.0, 30.0); // $5 in 30 minutes
        assert_eq!(cost_per_hour, 10.0); // Should be $10/hour
        
        let zero_time = calculate_cost_per_hour(5.0, 0.0);
        assert_eq!(zero_time, 0.0);
    }

    #[test]
    fn test_session_cost_calculation() {
        let mut model_usage = HashMap::new();
        
        model_usage.insert("claude-3-5-sonnet-20241022".to_string(), ModelUsage {
            model_name: "claude-3-5-sonnet-20241022".to_string(),
            total_input: 500_000,
            total_output: 250_000,
            total_cache_write: 0,
            total_cache_read: 0,
            message_count: 5,
            weighted_tokens: 750_000,
        });
        
        model_usage.insert("claude-3-haiku-20240307".to_string(), ModelUsage {
            model_name: "claude-3-haiku-20240307".to_string(),
            total_input: 200_000,
            total_output: 100_000,
            total_cache_write: 0,
            total_cache_read: 0,
            message_count: 3,
            weighted_tokens: 240_000,
        });

        let total_cost = calculate_session_cost(&model_usage);
        
        // Sonnet: (500K * $3) + (250K * $15) = $1.5 + $3.75 = $5.25
        // Haiku: (200K * $0.25) + (100K * $1.25) = $0.05 + $0.125 = $0.175
        // Total: $5.25 + $0.175 = $5.425
        assert_eq!(total_cost, 5.425);
    }
}