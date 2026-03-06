pub mod aliases;
pub mod csv;
pub mod hf_trainer;
pub mod jsonl;
pub mod regex_parser;
pub mod tensorboard;

use color_eyre::Result;

use crate::types::TrainingMetrics;

pub trait LogParser: Send {
    fn parse_line(&self, line: &str) -> Result<Option<TrainingMetrics>>;
}

fn jsonl_confidence(sample_lines: &[&str]) -> usize {
    let parser = jsonl::JsonlParser;
    sample_lines
        .iter()
        .filter_map(|line| parser.parse_line(line).ok().flatten())
        .count()
}

fn csv_confidence(sample_lines: &[&str]) -> Option<(csv::CsvParser, usize)> {
    for header_candidate in sample_lines.iter().filter(|line| !line.trim().is_empty()) {
        if let Ok(parser) = csv::CsvParser::new(header_candidate) {
            let score = sample_lines
                .iter()
                .filter_map(|line| parser.parse_line(line).ok().flatten())
                .count();
            return Some((parser, score));
        }
    }

    None
}

pub fn detect_parser(sample_lines: &[&str]) -> Box<dyn LogParser + Send> {
    let jsonl_score = jsonl_confidence(sample_lines);
    if let Some((csv_parser, csv_score)) = csv_confidence(sample_lines)
        && csv_score > jsonl_score
        && csv_score > 0
    {
        return Box::new(csv_parser);
    }

    if jsonl_score > 0 {
        return Box::new(jsonl::JsonlParser);
    }

    Box::new(jsonl::JsonlParser)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::aliases::{
        EVAL_LOSS_KEYS, GRAD_NORM_KEYS, LEARNING_RATE_KEYS, LOSS_KEYS, SAMPLES_PER_SECOND_KEYS,
        STEP_KEYS, STEPS_PER_SECOND_KEYS, THROUGHPUT_KEYS, TOKEN_KEYS, TOKENS_PER_SECOND_KEYS,
    };
    use crate::parsers::csv::CsvParser;
    use crate::parsers::hf_trainer::parse_trainer_state;
    use crate::parsers::jsonl::JsonlParser;
    use crate::parsers::regex_parser::RegexParser;
    use crate::types::TrainingMetrics;
    use std::fs;

    fn read_fixture(path: &str) -> String {
        fs::read_to_string(path).expect("fixture should be readable")
    }

    #[test]
    fn test_detect_jsonl_from_sample() {
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
    fn test_detect_empty_sample_returns_jsonl() {
        let sample_lines: Vec<&str> = vec![];
        let parser = detect_parser(&sample_lines);

        let result = parser
            .parse_line(r#"{"loss": 1.0}"#)
            .expect("parse should succeed");
        assert!(result.is_some());
    }

    #[test]
    fn test_detect_csv_from_header() {
        let sample_lines = vec!["loss,step,lr", "0.5,100,0.001"];
        let parser = detect_parser(&sample_lines);

        let result = parser
            .parse_line("0.5,100,0.001")
            .expect("parse should succeed");
        assert!(result.is_some());

        let metrics = result.unwrap();
        assert_eq!(metrics.loss, Some(0.5));
        assert_eq!(metrics.step, Some(100));
        assert_eq!(metrics.learning_rate, Some(0.001));
    }

    #[test]
    fn test_detect_csv_tab_from_header() {
        let sample_lines = vec!["loss\tstep\tlr", "0.5\t100\t0.001"];
        let parser = detect_parser(&sample_lines);

        let result = parser
            .parse_line("0.5\t100\t0.001")
            .expect("parse should succeed");
        assert!(result.is_some());

        let metrics = result.unwrap();
        assert_eq!(metrics.loss, Some(0.5));
        assert_eq!(metrics.step, Some(100));
        assert_eq!(metrics.learning_rate, Some(0.001));
    }

    #[test]
    fn test_detect_csv_semicolon_from_header() {
        let sample_lines = vec!["loss;step;lr", "0.5;100;0.001"];
        let parser = detect_parser(&sample_lines);

        let result = parser
            .parse_line("0.4;101;0.0009")
            .expect("parse should succeed");
        assert!(result.is_some());

        let metrics = result.unwrap();
        assert_eq!(metrics.loss, Some(0.4));
        assert_eq!(metrics.step, Some(101));
        assert_eq!(metrics.learning_rate, Some(0.0009));
    }

    #[test]
    fn test_detect_csv_pipe_from_header() {
        let sample_lines = vec!["loss|step|lr", "0.5|100|0.001"];
        let parser = detect_parser(&sample_lines);

        let result = parser
            .parse_line("0.45|102|0.0007")
            .expect("parse should succeed");
        assert!(result.is_some());

        let metrics = result.unwrap();
        assert_eq!(metrics.loss, Some(0.45));
        assert_eq!(metrics.step, Some(102));
        assert_eq!(metrics.learning_rate, Some(0.0007));
    }

    #[test]
    fn test_detect_fallback_to_jsonl() {
        let sample_lines = vec!["garbage text", "still not data"];
        let parser = detect_parser(&sample_lines);

        let result = parser
            .parse_line(r#"{"loss": 1.0}"#)
            .expect("fallback parser should parse jsonl");
        assert!(result.is_some());
    }

    #[test]
    fn test_detect_mixed_jsonl_and_noise() {
        let sample_lines = vec!["INFO start", r#"{"loss": 0.9, "step": 1}"#, "garbage"];
        let parser = detect_parser(&sample_lines);

        let result = parser
            .parse_line(r#"{"step": 42}"#)
            .expect("parse should succeed");
        assert!(result.is_some());
    }

    #[test]
    fn test_detect_parser_confidence_prefers_best_match() {
        let sample_lines = vec![
            "loss,step,lr",
            "0.5,100,0.001",
            "0.4,101,0.001",
            "INFO noise",
            r#"{"loss": 0.9}"#,
        ];

        let parser = detect_parser(&sample_lines);
        let parsed = parser
            .parse_line("0.3,102,0.001")
            .expect("parse should succeed");

        assert!(parsed.is_some());
        let metrics = parsed.expect("metrics should exist");
        assert_eq!(metrics.loss, Some(0.3));
        assert_eq!(metrics.step, Some(102));
    }

    #[test]
    fn test_detect_csv_header_after_noise_line() {
        let sample_lines = vec!["INFO startup", "loss,step,lr", "0.5,100,0.001"];
        let parser = detect_parser(&sample_lines);

        let result = parser
            .parse_line("0.4,101,0.0009")
            .expect("parse should succeed");
        assert!(result.is_some());
        let metrics = result.expect("metrics should exist");
        assert_eq!(metrics.loss, Some(0.4));
        assert_eq!(metrics.step, Some(101));
    }

    #[test]
    fn test_detect_parser_returns_send() {
        fn assert_send<T: Send>(_: T) {}

        let parser = detect_parser(&[]);
        assert_send(parser);
    }

    #[test]
    fn test_parser_alias_contract_consistency_jsonl_csv_hf() {
        assert!(LOSS_KEYS.contains(&"loss"));
        assert!(LOSS_KEYS.contains(&"train.loss"));
        assert!(LEARNING_RATE_KEYS.contains(&"learning_rate"));
        assert!(LEARNING_RATE_KEYS.contains(&"optimizer.lr"));
        assert!(STEP_KEYS.contains(&"step"));
        assert!(STEP_KEYS.contains(&"train.global_step"));
        assert!(THROUGHPUT_KEYS.contains(&"throughput"));
        assert!(TOKEN_KEYS.contains(&"tokens"));
        assert!(EVAL_LOSS_KEYS.contains(&"eval_loss"));
        assert!(GRAD_NORM_KEYS.contains(&"grad_norm"));
        assert!(SAMPLES_PER_SECOND_KEYS.contains(&"samples_per_second"));
        assert!(SAMPLES_PER_SECOND_KEYS.contains(&"speed.samples_per_second"));
        assert!(STEPS_PER_SECOND_KEYS.contains(&"steps_per_second"));
        assert!(TOKENS_PER_SECOND_KEYS.contains(&"tokens_per_second"));
    }

    #[test]
    fn test_parser_contract_fixture_matrix() {
        let training_jsonl = read_fixture("tests/fixtures/training.jsonl");
        let jsonl = JsonlParser;
        let jsonl_count = training_jsonl
            .lines()
            .filter_map(|line| {
                jsonl
                    .parse_line(line)
                    .expect("jsonl fixture line should parse")
            })
            .count();
        assert_eq!(jsonl_count, 10);

        let wandb_jsonl = read_fixture("tests/fixtures/wandb-events.jsonl");
        let wandb_steps: Vec<u64> = wandb_jsonl
            .lines()
            .filter_map(|line| {
                jsonl
                    .parse_line(line)
                    .expect("wandb fixture line should parse")
                    .and_then(|m| m.step)
            })
            .collect();
        assert_eq!(wandb_steps.len(), 10);
        assert_eq!(wandb_steps.first().copied(), Some(10));
        assert_eq!(wandb_steps.last().copied(), Some(100));

        let training_csv = read_fixture("tests/fixtures/training.csv");
        let mut csv_lines = training_csv.lines();
        let csv_header = csv_lines.next().expect("csv fixture should contain header");
        let csv = CsvParser::new(csv_header).expect("csv parser should initialize");
        let csv_count = csv_lines
            .filter_map(|line| {
                csv.parse_line(line)
                    .expect("csv fixture row should parse")
                    .and_then(|m| m.step)
            })
            .count();
        assert_eq!(csv_count, 10);

        let training_tab = read_fixture("tests/fixtures/training_tab.csv");
        let mut tab_lines = training_tab.lines();
        let tab_header = tab_lines.next().expect("tab fixture should contain header");
        let tab_csv = CsvParser::new(tab_header).expect("tab csv parser should initialize");
        let tab_count = tab_lines
            .filter_map(|line| {
                tab_csv
                    .parse_line(line)
                    .expect("tab fixture row should parse")
                    .and_then(|m| m.tokens_per_second)
            })
            .count();
        assert_eq!(tab_count, 10);

        let trainer_state = read_fixture("tests/fixtures/trainer_state.json");
        let trainer_metrics = parse_trainer_state(&trainer_state).expect("hf fixture should parse");
        assert_eq!(trainer_metrics.len(), 6);

        let regex = RegexParser::new(
            r"step=(?P<step>\d+) loss=(?P<loss>[\d.]+) lr=(?P<lr>[\d.eE-]+) throughput=(?P<throughput>[\d.]+) tokens=(?P<tokens>\d+)",
        )
        .expect("regex parser should initialize");
        let regex_line = "step=2 loss=1.11 lr=9e-4 throughput=640.5 tokens=2048";
        let regex_metrics = regex
            .parse_line(regex_line)
            .expect("regex line should parse")
            .expect("regex should match known fixture line");
        assert_eq!(regex_metrics.step, Some(2));
        assert_eq!(regex_metrics.loss, Some(1.11));

        let sample_lines = vec!["loss,step,lr", "0.5,100,0.001"];
        let auto = detect_parser(&sample_lines);
        let auto_metrics = auto
            .parse_line("0.5,100,0.001")
            .expect("auto parser should parse csv line")
            .expect("auto parser should return metrics");
        assert_eq!(auto_metrics.step, Some(100));
    }

    #[test]
    fn test_parser_contract_mixed_noise_never_panics() {
        let mixed = read_fixture("tests/fixtures/parser_contract_mixed.txt");
        let jsonl = JsonlParser;
        let regex = RegexParser::new(
            r"step=(?P<step>\d+) loss=(?P<loss>[\d.]+) lr=(?P<lr>[\d.eE-]+) throughput=(?P<throughput>[\d.]+) tokens=(?P<tokens>\d+)",
        )
        .expect("regex parser should initialize");
        let csv = CsvParser::new("loss,step,lr,throughput").expect("csv parser should initialize");

        let mut jsonl_hits = 0usize;
        let mut regex_hits = 0usize;
        let mut csv_hits = 0usize;

        for line in mixed.lines() {
            if jsonl
                .parse_line(line)
                .expect("jsonl should never panic on noisy line")
                .is_some()
            {
                jsonl_hits += 1;
            }
            if regex
                .parse_line(line)
                .expect("regex should never panic on noisy line")
                .is_some()
            {
                regex_hits += 1;
            }
            if csv
                .parse_line(line)
                .expect("csv should never panic on noisy line")
                .is_some()
            {
                csv_hits += 1;
            }
        }

        assert!(jsonl_hits >= 1);
        assert!(regex_hits >= 1);
        assert!(csv_hits >= 1);
    }

    #[test]
    fn test_parser_contract_non_finite_values_are_handled() {
        let non_finite = read_fixture("tests/fixtures/parser_contract_non_finite.jsonl");
        let jsonl = JsonlParser;

        let parsed: Vec<TrainingMetrics> = non_finite
            .lines()
            .filter_map(|line| {
                jsonl
                    .parse_line(line)
                    .expect("jsonl should not fail on malformed numeric encodings")
            })
            .collect();

        assert_eq!(parsed.len(), 4);

        let first = &parsed[0];
        assert_eq!(first.step, Some(1));
        assert!(first.loss.is_none());
        assert!(first.learning_rate.is_none());

        let third = &parsed[2];
        assert_eq!(third.step, Some(3));
        assert_eq!(third.loss, Some(0.91));
        assert_eq!(third.tokens_per_second, Some(1500.0));
    }
}
