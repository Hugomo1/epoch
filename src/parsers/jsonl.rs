use std::time::Instant;

use color_eyre::Result;

use super::LogParser;
use crate::types::TrainingMetrics;

pub struct JsonlParser;

impl LogParser for JsonlParser {
    fn parse_line(&self, line: &str) -> Result<Option<TrainingMetrics>> {
        let trimmed = line.trim();

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

        let loss = obj.get("loss").and_then(|v| v.as_f64());

        let learning_rate = obj
            .get("lr")
            .and_then(|v| v.as_f64())
            .or_else(|| obj.get("learning_rate").and_then(|v| v.as_f64()));

        let step = obj.get("step").and_then(|v| v.as_u64());
        let throughput = obj.get("throughput").and_then(|v| v.as_f64());
        let tokens = obj.get("tokens").and_then(|v| v.as_u64());

        if loss.is_none()
            && learning_rate.is_none()
            && step.is_none()
            && throughput.is_none()
            && tokens.is_none()
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
}
