//! # Plan Detector Module
//!
//! Automatically detects Claude subscription plan from usage patterns and limits
//!
//! ## Key Components
//! - [`PlanDetector`] - Main plan detection logic
//! - [`detect_plan_from_usage`] - Analyze usage patterns to infer plan
//! - [`validate_plan_limits`] - Check if usage matches expected plan limits

use chrono::{Duration, Utc};
use std::collections::HashMap;

use crate::block_builder::Block;
use crate::jsonl_parser::SessionData;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DetectedPlan {
    Pro,
    Max5,
    Max20,
    CustomMax,
    Unknown,
}

impl DetectedPlan {
    pub fn expected_limit(&self) -> Option<u64> {
        match self {
            DetectedPlan::Pro => Some(7_000),
            DetectedPlan::Max5 => Some(35_000),
            DetectedPlan::Max20 => Some(140_000),
            DetectedPlan::CustomMax => None, // Variable limit
            DetectedPlan::Unknown => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            DetectedPlan::Pro => "Pro",
            DetectedPlan::Max5 => "Max5",
            DetectedPlan::Max20 => "Max20",
            DetectedPlan::CustomMax => "CustomMax",
            DetectedPlan::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlanDetectionResult {
    pub detected_plan: DetectedPlan,
    pub confidence: f64,
    pub evidence: Vec<String>,
    pub max_observed_tokens: u64,
    pub has_limit_errors: bool,
    pub opus_usage_percentage: f64,
}

impl PlanDetectionResult {
    pub fn is_confident(&self) -> bool {
        self.confidence >= 0.8
    }
}

pub struct PlanDetector {
    min_confidence: f64,
    lookback_days: i64,
}

impl PlanDetector {
    pub fn new() -> Self {
        Self {
            min_confidence: 0.7,
            lookback_days: 7, // Look at past week of data
        }
    }

    pub fn detect_plan_from_blocks(&self, blocks: &[Block]) -> PlanDetectionResult {
        let mut evidence = Vec::new();
        let mut confidence: f64 = 0.0;
        let mut detected_plan = DetectedPlan::Unknown;

        // Find maximum observed tokens across all blocks
        let max_observed_tokens = blocks.iter()
            .map(|b| b.total_tokens)
            .max()
            .unwrap_or(0);

        // Check for limit-related evidence in sessions
        let has_limit_errors = blocks.iter().any(|_b| {
            // Look for sessions that might have hit limits
            // This would need to be implemented based on error detection in sessions
            false // Placeholder
        });

        // Calculate Opus usage percentage
        let total_tokens: u64 = blocks.iter().map(|b| b.total_tokens).sum();
        let opus_tokens: u64 = blocks.iter()
            .filter_map(|b| b.model_breakdown.as_ref())
            .flat_map(|breakdown| breakdown.iter())
            .filter(|(model, _)| model.contains("opus"))
            .map(|(_, counts)| counts.input_tokens + counts.output_tokens)
            .sum();

        let opus_usage_percentage = if total_tokens > 0 {
            opus_tokens as f64 / total_tokens as f64 * 100.0
        } else {
            0.0
        };

        // Plan detection logic based on observed patterns
        if max_observed_tokens > 100_000 {
            detected_plan = DetectedPlan::Max20;
            confidence = 0.9;
            evidence.push(format!("Observed {} tokens, exceeds Max5 limit", max_observed_tokens));
        } else if max_observed_tokens > 25_000 {
            detected_plan = DetectedPlan::Max5;
            confidence = 0.85;
            evidence.push(format!("Observed {} tokens, likely Max5", max_observed_tokens));
        } else if max_observed_tokens > 7_000 {
            // Could be Pro with custom max or actual Max5 with low usage
            if opus_usage_percentage > 20.0 {
                // High Opus usage would hit Max5 Opus limits quickly
                detected_plan = DetectedPlan::CustomMax;
                confidence = 0.7;
                evidence.push(format!("High Opus usage ({}%) with {} tokens suggests custom limits", 
                    opus_usage_percentage, max_observed_tokens));
            } else {
                detected_plan = DetectedPlan::Max5;
                confidence = 0.75;
                evidence.push(format!("Observed {} tokens, likely Max5 or custom Pro", max_observed_tokens));
            }
        } else {
            // Low usage - could be Pro or underutilized higher plan
            detected_plan = DetectedPlan::Pro;
            confidence = 0.6;
            evidence.push(format!("Low usage observed ({} tokens), likely Pro", max_observed_tokens));
        }

        // Adjust confidence based on data quality
        let recent_blocks = blocks.iter()
            .filter(|b| !b.is_gap && b.total_tokens > 0)
            .count();

        if recent_blocks < 3 {
            confidence *= 0.7; // Lower confidence with limited data
            evidence.push("Limited usage data available".to_string());
        }

        // Check for Opus usage patterns on Max5
        if detected_plan == DetectedPlan::Max5 && opus_usage_percentage > 30.0 {
            evidence.push(format!("High Opus usage ({}%) may trigger early limits on Max5", opus_usage_percentage));
        }

        PlanDetectionResult {
            detected_plan,
            confidence,
            evidence,
            max_observed_tokens,
            has_limit_errors,
            opus_usage_percentage,
        }
    }

    pub fn detect_plan_from_sessions(&self, sessions: &[SessionData]) -> PlanDetectionResult {
        let mut evidence = Vec::new();
        let mut confidence: f64 = 0.0;
        let mut detected_plan = DetectedPlan::Unknown;

        // Filter recent sessions
        let cutoff_time = Utc::now() - Duration::days(self.lookback_days);
        let recent_sessions: Vec<_> = sessions.iter()
            .filter(|s| s.start_time >= cutoff_time)
            .collect();

        if recent_sessions.is_empty() {
            return PlanDetectionResult {
                detected_plan: DetectedPlan::Unknown,
                confidence: 0.0,
                evidence: vec!["No recent sessions found".to_string()],
                max_observed_tokens: 0,
                has_limit_errors: false,
                opus_usage_percentage: 0.0,
            };
        }

        // Calculate total usage and patterns
        let total_weighted_tokens: u64 = recent_sessions.iter()
            .map(|s| s.total_weighted_tokens)
            .sum();

        let max_session_tokens = recent_sessions.iter()
            .map(|s| s.total_weighted_tokens)
            .max()
            .unwrap_or(0);

        // Check for limit errors
        let has_limit_errors = recent_sessions.iter().any(|s| s.has_limit_error);

        // Calculate model distribution
        let mut model_tokens: HashMap<String, u64> = HashMap::new();
        for session in &recent_sessions {
            for (model, usage) in &session.model_usage {
                *model_tokens.entry(model.clone()).or_default() += 
                    usage.total_input + usage.total_output;
            }
        }

        let opus_tokens: u64 = model_tokens.iter()
            .filter(|(model, _)| model.contains("opus"))
            .map(|(_, &tokens)| tokens)
            .sum();

        let opus_usage_percentage = if total_weighted_tokens > 0 {
            opus_tokens as f64 / total_weighted_tokens as f64 * 100.0
        } else {
            0.0
        };

        // Detection logic based on accumulated patterns
        if has_limit_errors {
            evidence.push("Observed limit reached errors".to_string());
            confidence += 0.3;

            // Analyze what kind of limits were hit
            if total_weighted_tokens > 100_000 {
                detected_plan = DetectedPlan::Max20;
                confidence = 0.95;
                evidence.push("Hit limits with high usage - Max20 plan".to_string());
            } else if total_weighted_tokens > 25_000 {
                detected_plan = DetectedPlan::Max5;
                confidence = 0.9;
                evidence.push("Hit limits with moderate usage - Max5 plan".to_string());
            } else {
                detected_plan = DetectedPlan::Pro;
                confidence = 0.85;
                evidence.push("Hit limits with low usage - Pro plan".to_string());
            }
        } else {
            // No limit errors - infer from usage patterns
            if total_weighted_tokens > 80_000 {
                detected_plan = DetectedPlan::Max20;
                confidence = 0.8;
                evidence.push(format!("High usage ({} tokens) without limits - Max20", total_weighted_tokens));
            } else if total_weighted_tokens > 20_000 {
                detected_plan = DetectedPlan::Max5;
                confidence = 0.75;
                evidence.push(format!("Moderate usage ({} tokens) - likely Max5", total_weighted_tokens));
            } else {
                detected_plan = DetectedPlan::Pro;
                confidence = 0.6;
                evidence.push(format!("Low usage ({} tokens) - likely Pro", total_weighted_tokens));
            }
        }

        // Adjust for Opus usage patterns
        if opus_usage_percentage > 25.0 && detected_plan == DetectedPlan::Max5 {
            evidence.push(format!("High Opus usage ({}%) on Max5 may trigger early limits", opus_usage_percentage));
        }

        // Adjust confidence based on data amount
        if recent_sessions.len() >= 10 {
            confidence += 0.1;
        } else if recent_sessions.len() < 3 {
            confidence *= 0.8;
            evidence.push("Limited session data".to_string());
        }

        PlanDetectionResult {
            detected_plan,
            confidence: confidence.min(1.0),
            evidence,
            max_observed_tokens: max_session_tokens,
            has_limit_errors,
            opus_usage_percentage,
        }
    }

    pub fn validate_plan_against_usage(&self, plan: DetectedPlan, blocks: &[Block]) -> bool {
        let expected_limit = plan.expected_limit();
        
        if let Some(limit) = expected_limit {
            let max_observed = blocks.iter().map(|b| b.total_tokens).max().unwrap_or(0);
            
            // Allow some variance for weighted token calculations
            let variance_factor = 1.2;
            max_observed <= (limit as f64 * variance_factor) as u64
        } else {
            true // CustomMax and Unknown have no fixed limits
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jsonl_parser::{SessionData, ModelUsage};
    use std::collections::HashMap;

    fn create_test_session(
        minutes_ago: i64,
        weighted_tokens: u64,
        model: &str,
        has_limit_error: bool,
    ) -> SessionData {
        let start_time = Utc::now() - Duration::minutes(minutes_ago);
        let end_time = start_time + Duration::minutes(10);

        let mut model_usage = HashMap::new();
        model_usage.insert(model.to_string(), ModelUsage {
            model_name: model.to_string(),
            total_input: weighted_tokens / 2,
            total_output: weighted_tokens / 2,
            total_cache_write: 0,
            total_cache_read: 0,
            message_count: 1,
            weighted_tokens,
        });

        SessionData {
            session_id: format!("test_{}", minutes_ago),
            start_time,
            end_time: Some(end_time),
            model_usage,
            total_weighted_tokens: weighted_tokens,
            has_limit_error,
            limit_type: None,
        }
    }

    #[test]
    fn test_pro_plan_detection() {
        let detector = PlanDetector::new();
        let sessions = vec![
            create_test_session(60, 2000, "claude-3-5-sonnet", false),
            create_test_session(30, 3000, "claude-3-5-sonnet", false),
            create_test_session(10, 1500, "claude-3-haiku", false),
        ];

        let result = detector.detect_plan_from_sessions(&sessions);
        assert_eq!(result.detected_plan, DetectedPlan::Pro);
        assert!(result.confidence > 0.5);
    }

    #[test]
    fn test_max5_plan_detection() {
        let detector = PlanDetector::new();
        let sessions = vec![
            create_test_session(120, 15000, "claude-3-5-sonnet", false),
            create_test_session(60, 20000, "claude-3-5-sonnet", false),
            create_test_session(30, 8000, "claude-3-haiku", false),
        ];

        let result = detector.detect_plan_from_sessions(&sessions);
        assert_eq!(result.detected_plan, DetectedPlan::Max5);
        assert!(result.confidence > 0.7);
    }

    #[test]
    fn test_limit_error_detection() {
        let detector = PlanDetector::new();
        let sessions = vec![
            create_test_session(60, 5000, "claude-3-5-sonnet", false),
            create_test_session(30, 2000, "claude-3-5-sonnet", true), // Hit limit
        ];

        let result = detector.detect_plan_from_sessions(&sessions);
        assert!(result.has_limit_errors);
        assert!(result.confidence > 0.8); // High confidence due to limit evidence
    }

    #[test]
    fn test_opus_heavy_usage() {
        let detector = PlanDetector::new();
        let sessions = vec![
            create_test_session(60, 10000, "claude-3-opus", false),
            create_test_session(30, 15000, "claude-3-opus", false),
            create_test_session(10, 2000, "claude-3-5-sonnet", false),
        ];

        let result = detector.detect_plan_from_sessions(&sessions);
        assert!(result.opus_usage_percentage > 70.0);
        // High Opus usage might suggest custom limits or Max20
    }

    #[test]
    fn test_plan_limit_validation() {
        let detector = PlanDetector::new();
        
        // Create a block that exceeds Pro limits
        let high_usage_block = Block {
            id: "test".to_string(),
            start_time: Utc::now().to_rfc3339(),
            end_time: Utc::now().to_rfc3339(),
            actual_end_time: None,
            is_active: false,
            is_gap: false,
            entries: 1,
            token_counts: crate::block_builder::TokenCounts::default(),
            total_tokens: 10000, // Exceeds Pro limit
            cost_usd: 0.0,
            models: vec!["claude-3-5-sonnet".to_string()],
            burn_rate: None,
            projection: None,
            model_breakdown: None,
            weighted_total_tokens: Some(10000),
            context_consumption_rate: None,
        };

        // Should fail validation for Pro plan
        assert!(!detector.validate_plan_against_usage(DetectedPlan::Pro, &[high_usage_block.clone()]));
        
        // Should pass validation for Max5 plan
        assert!(detector.validate_plan_against_usage(DetectedPlan::Max5, &[high_usage_block]));
    }
}