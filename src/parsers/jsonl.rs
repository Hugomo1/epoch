use std::time::Instant;

use color_eyre::Result;

use super::LogParser;
use crate::parsers::aliases::{
    EVAL_LOSS_KEYS, GRAD_NORM_KEYS, LEARNING_RATE_KEYS, LOSS_KEYS, SAMPLES_PER_SECOND_KEYS,
    STEP_KEYS, STEPS_PER_SECOND_KEYS, THROUGHPUT_KEYS, TOKEN_KEYS, TOKENS_PER_SECOND_KEYS,
};
use crate::types::TrainingMetrics;

pub struct JsonlParser;

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

impl LogParser for JsonlParser {
    fn parse_line(&self, line: &str) -> Result<Option<TrainingMetrics>> {
        let trimmed = line.trim().trim_start_matches('\u{feff}');

        if trimmed.is_empty() {
            return Ok(None);
        }

        let value: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => {
                tracing::debug!("Skipping non-JSON line: {}", line);
                return Ok(None);
            }
        };

        if !value.is_object() {
            tracing::debug!("Skipping non-object JSON: {}", line);
            return Ok(None);
        }

        let Some(obj) = value.as_object() else {
            return Ok(None);
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
            tracing::debug!("JSON line has no known training fields: {}", line);
            return Ok(None);
        }

        Ok(Some(TrainingMetrics {
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
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jsonl_parser_instantiation() {
        let _parser = JsonlParser;
    }

    #[test]
    fn test_parse_valid_jsonl_all_fields() {
        let parser = JsonlParser;
        let line =
            r#"{"loss": 0.5, "lr": 0.0001, "step": 100, "throughput": 1000.5, "tokens": 50000}"#;
        let result = parser.parse_line(line).expect("parse failed");

        assert!(result.is_some());
        let metrics = result.unwrap();
        assert_eq!(metrics.loss, Some(0.5));
        assert_eq!(metrics.learning_rate, Some(0.0001));
        assert_eq!(metrics.step, Some(100));
        assert_eq!(metrics.throughput, Some(1000.5));
        assert_eq!(metrics.tokens, Some(50000));
    }

    #[test]
    fn test_parse_only_loss() {
        let parser = JsonlParser;
        let line = r#"{"loss": 1.25}"#;
        let result = parser.parse_line(line).expect("parse failed");

        assert!(result.is_some());
        let metrics = result.unwrap();
        assert_eq!(metrics.loss, Some(1.25));
        assert_eq!(metrics.learning_rate, None);
        assert_eq!(metrics.step, None);
        assert_eq!(metrics.throughput, None);
        assert_eq!(metrics.tokens, None);
    }

    #[test]
    fn test_parse_blank_line() {
        let parser = JsonlParser;
        let result = parser.parse_line("").expect("parse failed");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_non_json_line() {
        let parser = JsonlParser;
        let result = parser.parse_line("not json").expect("parse failed");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_json_no_known_fields() {
        let parser = JsonlParser;
        let line = r#"{"epoch": 1, "batch": 32}"#;
        let result = parser.parse_line(line).expect("parse failed");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_learning_rate_field_name() {
        let parser = JsonlParser;
        let line = r#"{"learning_rate": 0.001}"#;
        let result = parser.parse_line(line).expect("parse failed");

        assert!(result.is_some());
        let metrics = result.unwrap();
        assert_eq!(metrics.learning_rate, Some(0.001));
    }

    #[test]
    fn test_parse_lr_field_name() {
        let parser = JsonlParser;
        let line = r#"{"lr": 0.002}"#;
        let result = parser.parse_line(line).expect("parse failed");

        assert!(result.is_some());
        let metrics = result.unwrap();
        assert_eq!(metrics.learning_rate, Some(0.002));
    }

    #[test]
    fn test_parse_train_loss_alias() {
        let parser = JsonlParser;
        let line = r#"{"train_loss": 0.5}"#;
        let result = parser.parse_line(line).expect("parse failed");

        assert!(result.is_some());
        let metrics = result.unwrap();
        assert_eq!(metrics.loss, Some(0.5));
    }

    #[test]
    fn test_parse_training_loss_alias() {
        let parser = JsonlParser;
        let line = r#"{"training_loss": 0.5}"#;
        let result = parser.parse_line(line).expect("parse failed");

        assert!(result.is_some());
        let metrics = result.unwrap();
        assert_eq!(metrics.loss, Some(0.5));
    }

    #[test]
    fn test_parse_slash_loss_alias() {
        let parser = JsonlParser;
        let line = r#"{"train/loss": 0.5}"#;
        let result = parser.parse_line(line).expect("parse failed");

        assert!(result.is_some());
        let metrics = result.unwrap();
        assert_eq!(metrics.loss, Some(0.5));
    }

    #[test]
    fn test_parse_global_step_alias() {
        let parser = JsonlParser;
        let line = r#"{"global_step": 100}"#;
        let result = parser.parse_line(line).expect("parse failed");

        assert!(result.is_some());
        let metrics = result.unwrap();
        assert_eq!(metrics.step, Some(100));
    }

    #[test]
    fn test_parse_wandb_step_alias() {
        let parser = JsonlParser;
        let line = r#"{"_step": 100}"#;
        let result = parser.parse_line(line).expect("parse failed");

        assert!(result.is_some());
        let metrics = result.unwrap();
        assert_eq!(metrics.step, Some(100));
    }

    #[test]
    fn test_parse_samples_per_second_alias() {
        let parser = JsonlParser;
        let line = r#"{"samples_per_second": 42.0}"#;
        let result = parser.parse_line(line).expect("parse failed");

        assert!(result.is_some());
        let metrics = result.unwrap();
        assert_eq!(metrics.samples_per_second, Some(42.0));
    }

    #[test]
    fn test_parse_wandb_combined() {
        let parser = JsonlParser;
        let line = r#"{"_step": 100, "train/loss": 0.5, "_runtime": 12.3}"#;
        let result = parser.parse_line(line).expect("parse failed");

        assert!(result.is_some());
        let metrics = result.unwrap();
        assert_eq!(metrics.step, Some(100));
        assert_eq!(metrics.loss, Some(0.5));
        assert_eq!(metrics.learning_rate, None);
        assert_eq!(metrics.throughput, None);
        assert_eq!(metrics.tokens, None);
    }

    #[test]
    fn test_parse_nested_metric_paths() {
        let parser = JsonlParser;
        let line = r#"{"train": {"loss": 0.42, "learning_rate": 0.0003, "global_step": 77}, "speed": {"samples_per_second": 12.5}}"#;
        let result = parser.parse_line(line).expect("parse failed");

        assert!(result.is_some());
        let metrics = result.expect("metrics should exist");
        assert_eq!(metrics.loss, Some(0.42));
        assert_eq!(metrics.learning_rate, Some(0.0003));
        assert_eq!(metrics.step, Some(77));
        assert_eq!(metrics.samples_per_second, Some(12.5));
    }

    #[test]
    fn test_parse_unknown_nested_objects_are_ignored() {
        let parser = JsonlParser;
        let line = r#"{"metrics": {"accuracy": 0.9}, "optimizer": {"beta1": 0.9}}"#;
        let result = parser.parse_line(line).expect("parse failed");
        assert!(result.is_none());
    }

    #[test]
    fn test_original_fields_still_work() {
        let parser = JsonlParser;
        let line =
            r#"{"loss": 0.5, "lr": 0.001, "step": 100, "throughput": 1000.5, "tokens": 50000}"#;
        let result = parser.parse_line(line).expect("parse failed");

        assert!(result.is_some());
        let metrics = result.unwrap();
        assert_eq!(metrics.loss, Some(0.5));
        assert_eq!(metrics.learning_rate, Some(0.001));
        assert_eq!(metrics.step, Some(100));
        assert_eq!(metrics.throughput, Some(1000.5));
        assert_eq!(metrics.tokens, Some(50000));
    }

    #[test]
    fn test_parse_empty_json_object() {
        let parser = JsonlParser;
        let result = parser.parse_line("{}").expect("parse failed");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_json_null() {
        let parser = JsonlParser;
        let result = parser.parse_line("null").expect("parse failed");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_invalid_json() {
        let parser = JsonlParser;
        let result = parser.parse_line("{invalid json").expect("parse failed");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_step_and_tokens_from_integral_float_values() {
        let parser = JsonlParser;
        let line = r#"{"step": 100.0, "tokens": "50000.0"}"#;
        let result = parser.parse_line(line).expect("parse failed");

        assert!(result.is_some());
        let metrics = result.unwrap();
        assert_eq!(metrics.step, Some(100));
        assert_eq!(metrics.tokens, Some(50000));
    }

    #[test]
    fn test_parse_rejects_non_integral_u64_values() {
        let parser = JsonlParser;
        let line = r#"{"step": 12.5, "tokens": "99.9"}"#;
        let result = parser.parse_line(line).expect("parse failed");

        assert!(result.is_none());
    }

    #[test]
    fn test_parse_jsonl_line_with_utf8_bom_prefix() {
        let parser = JsonlParser;
        let line = "\u{feff}{\"loss\": 0.33, \"step\": 9}";
        let result = parser.parse_line(line).expect("parse failed");

        assert!(result.is_some());
        let metrics = result.unwrap();
        assert_eq!(metrics.loss, Some(0.33));
        assert_eq!(metrics.step, Some(9));
    }

    #[test]
    fn test_parse_mixed_valid_invalid_lines() {
        let parser = JsonlParser;
        let lines = vec![
            r#"{"loss": 0.5}"#,
            "not json",
            "",
            r#"{"step": 100}"#,
            "{invalid",
            r#"{"epoch": 1}"#,
            r#"{"lr": 0.001, "step": 200}"#,
        ];

        let mut valid_count = 0;
        for line in lines {
            if let Ok(Some(_)) = parser.parse_line(line) {
                valid_count += 1;
            }
        }

        assert_eq!(valid_count, 3); // lines with loss, step, and lr+step
    }

    #[test]
    fn test_parse_lr_preferred_over_learning_rate() {
        let parser = JsonlParser;
        // If both are present, "lr" should take precedence
        let line = r#"{"lr": 0.001, "learning_rate": 0.002}"#;
        let result = parser.parse_line(line).expect("parse failed");

        assert!(result.is_some());
        let metrics = result.unwrap();
        assert_eq!(metrics.learning_rate, Some(0.001)); // lr takes precedence
    }

    #[test]
    fn test_parse_new_core_aliases() {
        let parser = JsonlParser;
        let line = r#"{"eval_loss": 0.75, "gradient_norm": 1.4, "train_samples_per_second": 12.0, "train_steps_per_second": 0.8, "train_tokens_per_second": 2048.0}"#;
        let result = parser.parse_line(line).expect("parse failed");

        assert!(result.is_some());
        let metrics = result.unwrap();
        assert_eq!(metrics.eval_loss, Some(0.75));
        assert_eq!(metrics.grad_norm, Some(1.4));
        assert_eq!(metrics.samples_per_second, Some(12.0));
        assert_eq!(metrics.steps_per_second, Some(0.8));
        assert_eq!(metrics.tokens_per_second, Some(2048.0));
    }
}
