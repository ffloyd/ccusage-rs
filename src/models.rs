//! # Model Configuration Module
//!
//! Defines consumption multipliers and pricing for different Claude models
//!
//! ## Key Components
//! - [`ModelConfig`] - Configuration for each model
//! - [`get_model_config`] - Retrieve config by model name
//! - [`calculate_weighted_tokens`] - Apply consumption multiplier

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub name: &'static str,
    pub consumption_multiplier: f64,
}

impl ModelConfig {
    pub fn calculate_weighted_tokens(&self, raw_tokens: u64) -> u64 {
        (raw_tokens as f64 * self.consumption_multiplier) as u64
    }
}

// Model configurations based on user observations and pricing
pub const MODEL_CONFIGS: &[ModelConfig] = &[
    ModelConfig {
        name: "claude-opus-4-20250514",
        consumption_multiplier: 5.0,  // Opus consumes 5x context window
    },
    ModelConfig {
        name: "claude-sonnet-4-20250514",
        consumption_multiplier: 1.0,  // Baseline
    },
    ModelConfig {
        name: "claude-3-5-haiku-20241022",
        consumption_multiplier: 0.8,  // Haiku is more efficient
    },
];

lazy_static::lazy_static! {
    static ref MODEL_MAP: HashMap<&'static str, &'static ModelConfig> = {
        let mut map = HashMap::new();
        for config in MODEL_CONFIGS {
            map.insert(config.name, config);
        }
        map
    };
}

pub fn get_model_config(model_name: &str) -> Option<&'static ModelConfig> {
    // Try exact match first
    if let Some(config) = MODEL_MAP.get(model_name) {
        return Some(*config);
    }
    
    // Try to match by prefix
    for (name, config) in MODEL_MAP.iter() {
        if model_name.starts_with(name) {
            return Some(*config);
        }
    }
    
    None
}

pub fn calculate_weighted_tokens(model_name: &str, raw_tokens: u64) -> u64 {
    get_model_config(model_name)
        .map(|config| config.calculate_weighted_tokens(raw_tokens))
        .unwrap_or(raw_tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_model_lookup() {
        assert!(get_model_config("claude-opus-4-20250514").is_some());
        assert!(get_model_config("claude-sonnet-4-20250514").is_some());
        assert!(get_model_config("claude-3-5-haiku-20241022").is_some());
    }
    
    #[test]
    fn test_weighted_tokens() {
        assert_eq!(calculate_weighted_tokens("claude-opus-4-20250514", 1000), 5000);
        assert_eq!(calculate_weighted_tokens("claude-sonnet-4-20250514", 1000), 1000);
        assert_eq!(calculate_weighted_tokens("claude-3-5-haiku-20241022", 1000), 800);
    }
    
}