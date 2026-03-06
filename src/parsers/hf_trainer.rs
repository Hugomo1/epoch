use std::time::Instant;

use color_eyre::Result;

use crate::parsers::aliases::{
    EVAL_LOSS_KEYS, GRAD_NORM_KEYS, LEARNING_RATE_KEYS, LOSS_KEYS, SAMPLES_PER_SECOND_KEYS,
    STEP_KEYS, STEPS_PER_SECOND_KEYS, THROUGHPUT_KEYS, TOKEN_KEYS, TOKENS_PER_SECOND_KEYS,
};
use crate::types::TrainingMetrics;

fn extract_value_for_key<'a>(
    obj: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<&'a serde_json::Value> {
    if let Some(value) = obj.get(key) {
        return Some(value);
    }

    if !key.contains('.') {
        return None;
    }

    let mut parts = key.split('.').peekable();
    let mut current = obj;

    while let Some(part) = parts.next() {
        let value = current.get(part)?;
        if parts.peek().is_none() {
            return Some(value);
        }
        current = value.as_object()?;
    }

    None
}

fn value_as_f64(value: &serde_json::Value) -> Option<f64> {
    let parsed = value
        .as_f64()
        .or_else(|| value.as_str().and_then(|s| s.parse::<f64>().ok()));
    parsed.filter(|n| n.is_finite())
}

fn value_as_u64(value: &serde_json::Value) -> Option<u64> {
    fn u64_from_f64(v: f64) -> Option<u64> {
        if !v.is_finite() || v < 0.0 || v.fract() != 0.0 || v > u64::MAX as f64 {
            return None;
        }
        Some(v as u64)
    }

    fn parse_u64_text(text: &str) -> Option<u64> {
        let trimmed = text.trim();
        trimmed
            .parse::<u64>()
            .ok()
            .or_else(|| trimmed.parse::<f64>().ok().and_then(u64_from_f64))
    }

    value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|n| u64::try_from(n).ok()))
        .or_else(|| value.as_f64().and_then(u64_from_f64))
        .or_else(|| value.as_str().and_then(parse_u64_text))
}

fn try_extract_f64(obj: &serde_json::Map<String, serde_json::Value>, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .find_map(|key| extract_value_for_key(obj, key).and_then(value_as_f64))
}

fn try_extract_u64(obj: &serde_json::Map<String, serde_json::Value>, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| extract_value_for_key(obj, key).and_then(value_as_u64))
}

pub fn parse_trainer_state(content: &str) -> Result<Vec<TrainingMetrics>> {
    let value: serde_json::Value = serde_json::from_str(content.trim_start_matches('\u{feff}'))?;

    let Some(log_history) = value.get("log_history").and_then(|v| v.as_array()) else {
        return Ok(vec![]);
    };

    let mut metrics_vec = Vec::with_capacity(log_history.len());

    for entry in log_history {
        let Some(obj) = entry.as_object() else {
            continue;
        };

        let loss = try_extract_f64(obj, LOSS_KEYS);
        let learning_rate = try_extract_f64(obj, LEARNING_RATE_KEYS);
        let step = try_extract_u64(obj, STEP_KEYS);
        let throughput = try_extract_f64(obj, THROUGHPUT_KEYS);
        let tokens = try_extract_u64(obj, TOKEN_KEYS);
        let eval_loss = try_extract_f64(obj, EVAL_LOSS_KEYS);
        let grad_norm = try_extract_f64(obj, GRAD_NORM_KEYS);
        let samples_per_second = try_extract_f64(obj, SAMPLES_PER_SECOND_KEYS);
        let steps_per_second = try_extract_f64(obj, STEPS_PER_SECOND_KEYS);
        let tokens_per_second = try_extract_f64(obj, TOKENS_PER_SECOND_KEYS);

        if loss.is_none()
            && learning_rate.is_none()
            && step.is_none()
            && throughput.is_none()
            && tokens.is_none()
            && eval_loss.is_none()
            && grad_norm.is_none()
            && samples_per_second.is_none()
            && steps_per_second.is_none()
            && tokens_per_second.is_none()
        {
            continue;
        }

        metrics_vec.push(TrainingMetrics {
            loss,
            learning_rate,
            step,
            throughput,
            tokens,
            eval_loss,
            grad_norm,
            samples_per_second,
            steps_per_second,
            tokens_per_second,
            timestamp: Instant::now(),
        });
    }

    Ok(metrics_vec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_trainer_state_valid() {
        let content = r#"{
            "log_history": [
                {"loss": 4.5, "learning_rate": 0.0001, "step": 10, "epoch": 0.1},
                {"loss": 3.8, "learning_rate": 0.0002, "step": 20, "epoch": 0.2},
                {"loss": 3.1, "learning_rate": 0.0003, "step": 30, "epoch": 0.3}
            ]
        }"#;

        let parsed = parse_trainer_state(content).expect("parse should succeed");
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].loss, Some(4.5));
        assert_eq!(parsed[0].learning_rate, Some(0.0001));
        assert_eq!(parsed[0].step, Some(10));
    }

    #[test]
    fn test_parse_trainer_state_missing_log_history() {
        let content = r#"{"best_metric": 0.9}"#;
        let parsed = parse_trainer_state(content).expect("parse should succeed");
        assert!(parsed.is_empty());
    }

    #[test]
    fn test_parse_trainer_state_empty_log_history() {
        let content = r#"{"log_history": []}"#;
        let parsed = parse_trainer_state(content).expect("parse should succeed");
        assert!(parsed.is_empty());
    }

    #[test]
    fn test_parse_trainer_state_partial_fields() {
        let content = r#"{"log_history": [{"loss": 1.25}]}"#;

        let parsed = parse_trainer_state(content).expect("parse should succeed");
        assert_eq!(parsed.len(), 1);
        let metrics = &parsed[0];
        assert_eq!(metrics.loss, Some(1.25));
        assert_eq!(metrics.learning_rate, None);
        assert_eq!(metrics.step, None);
        assert_eq!(metrics.throughput, None);
        assert_eq!(metrics.tokens, None);
    }

    #[test]
    fn test_parse_trainer_state_invalid_json() {
        let result = parse_trainer_state("{invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_trainer_state_ignores_epoch_field() {
        let content = r#"{"log_history": [{"loss": 0.5, "epoch": 2.0, "step": 100}]}"#;

        let parsed = parse_trainer_state(content).expect("parse should succeed");
        assert_eq!(parsed.len(), 1);
        let metrics = &parsed[0];
        assert_eq!(metrics.loss, Some(0.5));
        assert_eq!(metrics.step, Some(100));
        assert_eq!(metrics.learning_rate, None);
        assert_eq!(metrics.throughput, None);
        assert_eq!(metrics.tokens, None);
    }

    #[test]
    fn test_parse_trainer_state_new_core_fields() {
        let content = r#"{"log_history": [{"eval_loss": 0.7, "gradient_norm": 1.3, "train_samples_per_second": 12.0, "train_steps_per_second": 0.8, "train_tokens_per_second": 2048.0}]}"#;

        let parsed = parse_trainer_state(content).expect("parse should succeed");
        assert_eq!(parsed.len(), 1);
        let metrics = &parsed[0];
        assert_eq!(metrics.eval_loss, Some(0.7));
        assert_eq!(metrics.grad_norm, Some(1.3));
        assert_eq!(metrics.samples_per_second, Some(12.0));
        assert_eq!(metrics.steps_per_second, Some(0.8));
        assert_eq!(metrics.tokens_per_second, Some(2048.0));
    }

    #[test]
    fn test_parse_trainer_state_nested_metric_paths() {
        let content = r#"{
            "log_history": [
                {
                    "train": {"loss": 0.77, "learning_rate": 0.0004, "global_step": 55},
                    "speed": {"samples_per_second": 14.2}
                }
            ]
        }"#;

        let parsed = parse_trainer_state(content).expect("parse should succeed");
        assert_eq!(parsed.len(), 1);
        let metrics = &parsed[0];
        assert_eq!(metrics.loss, Some(0.77));
        assert_eq!(metrics.learning_rate, Some(0.0004));
        assert_eq!(metrics.step, Some(55));
        assert_eq!(metrics.samples_per_second, Some(14.2));
    }

    #[test]
    fn test_parse_trainer_state_accepts_integral_float_step_and_tokens() {
        let content = r#"{"log_history": [{"step": 42.0, "tokens": "1000.0", "loss": 0.4}]}"#;

        let parsed = parse_trainer_state(content).expect("parse should succeed");
        assert_eq!(parsed.len(), 1);
        let metrics = &parsed[0];
        assert_eq!(metrics.step, Some(42));
        assert_eq!(metrics.tokens, Some(1000));
    }

    #[test]
    fn test_parse_trainer_state_rejects_non_integral_u64_values() {
        let content = r#"{"log_history": [{"step": 10.5, "tokens": "7.7"}]}"#;

        let parsed = parse_trainer_state(content).expect("parse should succeed");
        assert!(parsed.is_empty());
    }

    #[test]
    fn test_parse_trainer_state_with_utf8_bom_prefix() {
        let content = "\u{feff}{\"log_history\": [{\"loss\": 0.8, \"step\": 3}]}";

        let parsed = parse_trainer_state(content).expect("parse should succeed");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].loss, Some(0.8));
        assert_eq!(parsed[0].step, Some(3));
    }

    #[test]
    fn test_parse_trainer_state_skips_non_object_entries() {
        let content = r#"{"log_history": ["bad", 7, {"loss": 0.6, "step": 2}]}"#;

        let parsed = parse_trainer_state(content).expect("parse should succeed");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].loss, Some(0.6));
        assert_eq!(parsed[0].step, Some(2));
    }
}
