use color_eyre::Result;
use regex::Regex;
use std::time::Instant;

use super::LogParser;
use crate::types::TrainingMetrics;

pub const DEFAULT_PATTERN: &str = r"[Ss]tep\s*[:=]?\s*(?P<step>\d+).*[Ll]oss\s*[:=]?\s*(?P<loss>[\d.]+)(?:.*[Ll](?:earning_)?[Rr](?:ate)?\s*[:=]?\s*(?P<lr>[\d.eE-]+))?";

pub struct RegexParser {
    pattern: Regex,
}

impl RegexParser {
    pub fn new(pattern: &str) -> Result<Self> {
        let compiled = Regex::new(pattern)?;
        Ok(Self { pattern: compiled })
    }
}

impl LogParser for RegexParser {
    fn parse_line(&self, line: &str) -> Result<Option<TrainingMetrics>> {
        fn parse_finite_f64(value: &str) -> Option<f64> {
            value.parse::<f64>().ok().filter(|n| n.is_finite())
        }

        fn parse_u64_like(value: &str) -> Option<u64> {
            value.parse::<u64>().ok().or_else(|| {
                value.parse::<f64>().ok().and_then(|n| {
                    if n.is_finite() && n >= 0.0 && n.fract() == 0.0 && n <= u64::MAX as f64 {
                        Some(n as u64)
                    } else {
                        None
                    }
                })
            })
        }

        let captures = match self.pattern.captures(line) {
            Some(caps) => caps,
            None => return Ok(None),
        };

        let mut metrics = TrainingMetrics {
            timestamp: Instant::now(),
            ..Default::default()
        };

        let mut has_any_field = false;

        if let Some(loss_match) = captures.name("loss") {
            if let Some(val) = parse_finite_f64(loss_match.as_str()) {
                metrics.loss = Some(val);
                has_any_field = true;
            }
        }

        if let Some(lr_match) = captures.name("lr") {
            if let Some(val) = parse_finite_f64(lr_match.as_str()) {
                metrics.learning_rate = Some(val);
                has_any_field = true;
            }
        }

        if let Some(step_match) = captures.name("step") {
            if let Some(val) = parse_u64_like(step_match.as_str()) {
                metrics.step = Some(val);
                has_any_field = true;
            }
        }

        if let Some(throughput_match) = captures.name("throughput") {
            if let Some(val) = parse_finite_f64(throughput_match.as_str()) {
                metrics.throughput = Some(val);
                has_any_field = true;
            }
        }

        if let Some(eval_loss_match) = captures.name("eval_loss")
            && let Some(val) = parse_finite_f64(eval_loss_match.as_str())
        {
            metrics.eval_loss = Some(val);
            has_any_field = true;
        }

        if let Some(grad_norm_match) = captures.name("grad_norm")
            && let Some(val) = parse_finite_f64(grad_norm_match.as_str())
        {
            metrics.grad_norm = Some(val);
            has_any_field = true;
        }

        if let Some(samples_match) = captures.name("samples_per_second")
            && let Some(val) = parse_finite_f64(samples_match.as_str())
        {
            metrics.samples_per_second = Some(val);
            has_any_field = true;
        }

        if let Some(steps_match) = captures.name("steps_per_second")
            && let Some(val) = parse_finite_f64(steps_match.as_str())
        {
            metrics.steps_per_second = Some(val);
            has_any_field = true;
        }

        if let Some(tokens_ps_match) = captures.name("tokens_per_second")
            && let Some(val) = parse_finite_f64(tokens_ps_match.as_str())
        {
            metrics.tokens_per_second = Some(val);
            has_any_field = true;
        }

        if let Some(tokens_match) = captures.name("tokens") {
            if let Some(val) = parse_u64_like(tokens_match.as_str()) {
                metrics.tokens = Some(val);
                has_any_field = true;
            }
        }

        if has_any_field {
            Ok(Some(metrics))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regex_parser_with_all_fields() {
        let pattern = r"step=(?P<step>\d+) loss=(?P<loss>[\d.]+) lr=(?P<lr>[\d.eE-]+) throughput=(?P<throughput>[\d.]+) tokens=(?P<tokens>\d+)";
        let parser = RegexParser::new(pattern).expect("valid pattern");

        let line = "step=100 loss=0.5 lr=1e-4 throughput=1000.0 tokens=50000";
        let result = parser.parse_line(line).expect("parse should succeed");

        assert!(result.is_some());
        let metrics = result.unwrap();
        assert_eq!(metrics.step, Some(100));
        assert_eq!(metrics.loss, Some(0.5));
        assert_eq!(metrics.learning_rate, Some(1e-4));
        assert_eq!(metrics.throughput, Some(1000.0));
        assert_eq!(metrics.tokens, Some(50000));
    }

    #[test]
    fn test_regex_parser_partial_fields() {
        let pattern = r"step=(?P<step>\d+) loss=(?P<loss>[\d.]+)";
        let parser = RegexParser::new(pattern).expect("valid pattern");

        let line = "step=100 loss=0.5";
        let result = parser.parse_line(line).expect("parse should succeed");

        assert!(result.is_some());
        let metrics = result.unwrap();
        assert_eq!(metrics.step, Some(100));
        assert_eq!(metrics.loss, Some(0.5));
        assert_eq!(metrics.learning_rate, None);
        assert_eq!(metrics.throughput, None);
        assert_eq!(metrics.tokens, None);
    }

    #[test]
    fn test_regex_parser_no_match() {
        let pattern = r"step=(?P<step>\d+) loss=(?P<loss>[\d.]+)";
        let parser = RegexParser::new(pattern).expect("valid pattern");

        let line = "this line does not match the pattern";
        let result = parser.parse_line(line).expect("parse should succeed");

        assert!(result.is_none());
    }

    #[test]
    fn test_regex_parser_invalid_pattern() {
        let pattern = "[invalid";
        let result = RegexParser::new(pattern);

        assert!(result.is_err());
    }

    #[test]
    fn test_regex_parser_unparseable_capture() {
        let pattern = r"step=(?P<step>\d+) loss=(?P<loss>\w+)";
        let parser = RegexParser::new(pattern).expect("valid pattern");

        // "abc" can't be parsed as f64
        let line = "step=100 loss=abc";
        let result = parser.parse_line(line).expect("parse should succeed");

        assert!(result.is_some());
        let metrics = result.unwrap();
        assert_eq!(metrics.step, Some(100));
        assert_eq!(metrics.loss, None); // Should be None, not panic
    }

    #[test]
    fn test_default_pattern_matches() {
        let parser = RegexParser::new(DEFAULT_PATTERN).expect("default pattern should be valid");

        let line = "Step 100 | Loss: 0.5 | LR: 1e-4";
        let result = parser.parse_line(line).expect("parse should succeed");

        assert!(result.is_some());
        let metrics = result.unwrap();
        assert_eq!(metrics.step, Some(100));
        assert_eq!(metrics.loss, Some(0.5));
        assert_eq!(metrics.learning_rate, Some(1e-4));
    }

    #[test]
    fn test_default_pattern_no_match() {
        let parser = RegexParser::new(DEFAULT_PATTERN).expect("default pattern should be valid");

        let line = "unrelated log message";
        let result = parser.parse_line(line).expect("parse should succeed");

        assert!(result.is_none());
    }

    #[test]
    fn test_regex_parser_ignores_non_finite_numeric_values() {
        let pattern = r"step=(?P<step>\d+) loss=(?P<loss>\S+) throughput=(?P<throughput>\S+)";
        let parser = RegexParser::new(pattern).expect("valid pattern");

        let result = parser
            .parse_line("step=12 loss=NaN throughput=inf")
            .expect("parse should succeed");

        assert!(result.is_some());
        let metrics = result.unwrap();
        assert_eq!(metrics.step, Some(12));
        assert_eq!(metrics.loss, None);
        assert_eq!(metrics.throughput, None);
    }

    #[test]
    fn test_regex_parser_accepts_integral_float_u64_fields() {
        let pattern = r"step=(?P<step>\S+) tokens=(?P<tokens>\S+)";
        let parser = RegexParser::new(pattern).expect("valid pattern");

        let result = parser
            .parse_line("step=10.0 tokens=2048.0")
            .expect("parse should succeed");

        assert!(result.is_some());
        let metrics = result.unwrap();
        assert_eq!(metrics.step, Some(10));
        assert_eq!(metrics.tokens, Some(2048));
    }
}
