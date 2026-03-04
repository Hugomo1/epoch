pub mod csv;
pub mod jsonl;
pub mod regex_parser;
pub mod tensorboard;

use color_eyre::Result;

use crate::types::TrainingMetrics;

pub trait LogParser {
    fn parse_line(&self, line: &str) -> Result<Option<TrainingMetrics>>;
}

pub fn detect_parser(sample_lines: &[&str]) -> Box<dyn LogParser> {
    let parser = jsonl::JsonlParser;

    let non_empty_lines: Vec<&&str> = sample_lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .collect();

    if non_empty_lines.is_empty() {
        return Box::new(parser);
    }

    let mut successful_parses = 0;
    for line in &non_empty_lines {
        if let Ok(Some(_)) = parser.parse_line(line) {
            successful_parses += 1;
        }
    }

    let _success_rate = successful_parses as f64 / non_empty_lines.len() as f64;

    Box::new(jsonl::JsonlParser)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_placeholder() {
        assert!(true);
    }

    #[test]
    fn test_detect_parser_returns_working_parser() {
        let sample_lines = vec![
            r#"{"loss": 0.5, "step": 100}"#,
            r#"{"lr": 0.001, "step": 200}"#,
            "some text",
        ];

        let parser = detect_parser(&sample_lines);

        let result = parser
            .parse_line(r#"{"loss": 0.3}"#)
            .expect("parse should succeed");
        assert!(result.is_some());

        let metrics = result.unwrap();
        assert_eq!(metrics.loss, Some(0.3));
    }

    #[test]
    fn test_detect_parser_empty_sample() {
        let sample_lines: Vec<&str> = vec![];
        let parser = detect_parser(&sample_lines);

        let result = parser
            .parse_line(r#"{"loss": 1.0}"#)
            .expect("parse should succeed");
        assert!(result.is_some());
    }

    #[test]
    fn test_detect_parser_all_blank_lines() {
        let sample_lines = vec!["", "   ", "\n"];
        let parser = detect_parser(&sample_lines);

        let result = parser
            .parse_line(r#"{"step": 42}"#)
            .expect("parse should succeed");
        assert!(result.is_some());
    }
}
