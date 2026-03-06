use std::time::Instant;

use color_eyre::{Result, eyre::bail};

use super::LogParser;
use crate::parsers::aliases::{
    EVAL_LOSS_KEYS, GRAD_NORM_KEYS, LEARNING_RATE_KEYS, LOSS_KEYS, SAMPLES_PER_SECOND_KEYS,
    STEP_KEYS, STEPS_PER_SECOND_KEYS, THROUGHPUT_KEYS, TOKEN_KEYS, TOKENS_PER_SECOND_KEYS,
};
use crate::types::TrainingMetrics;

#[derive(Debug, Clone, Default)]
struct CsvColumnMap {
    loss: Option<usize>,
    learning_rate: Option<usize>,
    step: Option<usize>,
    throughput: Option<usize>,
    tokens: Option<usize>,
    eval_loss: Option<usize>,
    grad_norm: Option<usize>,
    samples_per_second: Option<usize>,
    steps_per_second: Option<usize>,
    tokens_per_second: Option<usize>,
}

impl CsvColumnMap {
    fn has_any(&self) -> bool {
        self.loss.is_some()
            || self.learning_rate.is_some()
            || self.step.is_some()
            || self.throughput.is_some()
            || self.tokens.is_some()
            || self.eval_loss.is_some()
            || self.grad_norm.is_some()
            || self.samples_per_second.is_some()
            || self.steps_per_second.is_some()
            || self.tokens_per_second.is_some()
    }
}

pub struct CsvParser {
    delimiter: u8,
    column_indices: CsvColumnMap,
}

impl CsvParser {
    pub fn new(header_line: &str) -> Result<Self> {
        for delimiter in [b',', b'\t', b';', b'|'] {
            if let Some(column_indices) = Self::build_column_map(header_line, delimiter)?
                && column_indices.has_any()
            {
                return Ok(Self {
                    delimiter,
                    column_indices,
                });
            }
        }

        bail!("csv header contains no known training metric columns")
    }

    fn build_column_map(header_line: &str, delimiter: u8) -> Result<Option<CsvColumnMap>> {
        let Some(record) = Self::parse_record(header_line, delimiter)? else {
            return Ok(None);
        };

        let mut map = CsvColumnMap::default();

        for (index, field) in record.iter().enumerate() {
            let normalized = field
                .trim()
                .trim_start_matches('\u{feff}')
                .to_ascii_lowercase();

            if map.loss.is_none() && LOSS_KEYS.contains(&normalized.as_str()) {
                map.loss = Some(index);
            }
            if map.learning_rate.is_none() && LEARNING_RATE_KEYS.contains(&normalized.as_str()) {
                map.learning_rate = Some(index);
            }
            if map.step.is_none() && STEP_KEYS.contains(&normalized.as_str()) {
                map.step = Some(index);
            }
            if map.throughput.is_none() && THROUGHPUT_KEYS.contains(&normalized.as_str()) {
                map.throughput = Some(index);
            }
            if map.tokens.is_none() && TOKEN_KEYS.contains(&normalized.as_str()) {
                map.tokens = Some(index);
            }
            if map.eval_loss.is_none() && EVAL_LOSS_KEYS.contains(&normalized.as_str()) {
                map.eval_loss = Some(index);
            }
            if map.grad_norm.is_none() && GRAD_NORM_KEYS.contains(&normalized.as_str()) {
                map.grad_norm = Some(index);
            }
            if map.samples_per_second.is_none()
                && SAMPLES_PER_SECOND_KEYS.contains(&normalized.as_str())
            {
                map.samples_per_second = Some(index);
            }
            if map.steps_per_second.is_none()
                && STEPS_PER_SECOND_KEYS.contains(&normalized.as_str())
            {
                map.steps_per_second = Some(index);
            }
            if map.tokens_per_second.is_none()
                && TOKENS_PER_SECOND_KEYS.contains(&normalized.as_str())
            {
                map.tokens_per_second = Some(index);
            }
        }

        Ok(Some(map))
    }

    fn parse_record(line: &str, delimiter: u8) -> Result<Option<::csv::StringRecord>> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }

        let mut reader = ::csv::ReaderBuilder::new()
            .has_headers(false)
            .delimiter(delimiter)
            .flexible(true)
            .from_reader(trimmed.as_bytes());

        let mut record = ::csv::StringRecord::new();
        let has_record = match reader.read_record(&mut record) {
            Ok(found) => found,
            Err(_) => return Ok(None),
        };

        if !has_record {
            return Ok(None);
        }

        Ok(Some(record))
    }

    fn parse_f64_field(record: &::csv::StringRecord, index: Option<usize>) -> Option<f64> {
        index
            .and_then(|idx| record.get(idx))
            .and_then(|value| value.trim().parse::<f64>().ok())
            .filter(|n| n.is_finite())
    }

    fn parse_u64_field(record: &::csv::StringRecord, index: Option<usize>) -> Option<u64> {
        fn u64_from_f64(v: f64) -> Option<u64> {
            if !v.is_finite() || v < 0.0 || v.fract() != 0.0 || v > u64::MAX as f64 {
                return None;
            }
            Some(v as u64)
        }

        index.and_then(|idx| record.get(idx)).and_then(|value| {
            let trimmed = value.trim();
            trimmed
                .parse::<u64>()
                .ok()
                .or_else(|| trimmed.parse::<f64>().ok().and_then(u64_from_f64))
        })
    }
}

impl LogParser for CsvParser {
    fn parse_line(&self, line: &str) -> Result<Option<TrainingMetrics>> {
        let Some(record) = Self::parse_record(line, self.delimiter)? else {
            return Ok(None);
        };

        if record
            .iter()
            .all(|value| value.trim().parse::<f64>().is_err())
        {
            return Ok(None);
        }

        let loss = Self::parse_f64_field(&record, self.column_indices.loss);
        let learning_rate = Self::parse_f64_field(&record, self.column_indices.learning_rate);
        let step = Self::parse_u64_field(&record, self.column_indices.step);
        let throughput = Self::parse_f64_field(&record, self.column_indices.throughput);
        let tokens = Self::parse_u64_field(&record, self.column_indices.tokens);
        let eval_loss = Self::parse_f64_field(&record, self.column_indices.eval_loss);
        let grad_norm = Self::parse_f64_field(&record, self.column_indices.grad_norm);
        let samples_per_second =
            Self::parse_f64_field(&record, self.column_indices.samples_per_second);
        let steps_per_second = Self::parse_f64_field(&record, self.column_indices.steps_per_second);
        let tokens_per_second =
            Self::parse_f64_field(&record, self.column_indices.tokens_per_second);

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
    fn test_csv_parser_new_with_comma_header() {
        let parser = CsvParser::new("loss,step,lr");
        assert!(parser.is_ok());
    }

    #[test]
    fn test_csv_parser_new_with_tab_header() {
        let parser = CsvParser::new("loss\tstep\tlr");
        assert!(parser.is_ok());
    }

    #[test]
    fn test_csv_parser_new_with_semicolon_header() {
        let parser = CsvParser::new("loss;step;lr");
        assert!(parser.is_ok());
    }

    #[test]
    fn test_csv_parser_new_with_pipe_header() {
        let parser = CsvParser::new("loss|step|lr");
        assert!(parser.is_ok());
    }

    #[test]
    fn test_csv_parser_new_with_aliases() {
        let parser = CsvParser::new("train_loss,global_step,learning_rate");
        assert!(parser.is_ok());
    }

    #[test]
    fn test_csv_parser_new_no_known_columns() {
        let parser = CsvParser::new("foo,bar,baz");
        assert!(parser.is_err());
    }

    #[test]
    fn test_csv_parse_data_row() {
        let parser = CsvParser::new("loss,step").expect("parser should be created");
        let result = parser.parse_line("0.5,100").expect("parse should succeed");

        assert!(result.is_some());
        let metrics = result.expect("metrics should exist");
        assert_eq!(metrics.loss, Some(0.5));
        assert_eq!(metrics.step, Some(100));
    }

    #[test]
    fn test_csv_parse_data_row_with_semicolon_delimiter() {
        let parser = CsvParser::new("loss;step;lr").expect("parser should be created");
        let result = parser
            .parse_line("0.5;100;0.001")
            .expect("parse should succeed");

        assert!(result.is_some());
        let metrics = result.expect("metrics should exist");
        assert_eq!(metrics.loss, Some(0.5));
        assert_eq!(metrics.step, Some(100));
        assert_eq!(metrics.learning_rate, Some(0.001));
    }

    #[test]
    fn test_csv_parse_data_row_with_pipe_delimiter() {
        let parser = CsvParser::new("loss|step|lr").expect("parser should be created");
        let result = parser
            .parse_line("0.6|101|0.0008")
            .expect("parse should succeed");

        assert!(result.is_some());
        let metrics = result.expect("metrics should exist");
        assert_eq!(metrics.loss, Some(0.6));
        assert_eq!(metrics.step, Some(101));
        assert_eq!(metrics.learning_rate, Some(0.0008));
    }

    #[test]
    fn test_csv_parse_skips_non_numeric() {
        let parser = CsvParser::new("loss,step").expect("parser should be created");
        let result = parser
            .parse_line("not_a_number,100")
            .expect("parse should succeed");

        assert!(result.is_some());
        let metrics = result.expect("metrics should exist");
        assert_eq!(metrics.loss, None);
        assert_eq!(metrics.step, Some(100));
    }

    #[test]
    fn test_csv_parse_empty_line() {
        let parser = CsvParser::new("loss,step").expect("parser should be created");
        let result = parser.parse_line("").expect("parse should succeed");
        assert!(result.is_none());
    }

    #[test]
    fn test_csv_parse_header_row_skipped() {
        let parser = CsvParser::new("loss,step").expect("parser should be created");
        let result = parser
            .parse_line("loss,step")
            .expect("parse should succeed");
        assert!(result.is_none());
    }

    #[test]
    fn test_csv_new_core_alias_columns() {
        let parser = CsvParser::new(
            "eval_loss,gradient_norm,train_samples_per_second,train_steps_per_second,train_tokens_per_second",
        )
        .expect("parser should be created");
        let result = parser
            .parse_line("0.7,1.3,12.0,0.8,2048.0")
            .expect("parse should succeed");

        assert!(result.is_some());
        let metrics = result.unwrap();
        assert_eq!(metrics.eval_loss, Some(0.7));
        assert_eq!(metrics.grad_norm, Some(1.3));
        assert_eq!(metrics.samples_per_second, Some(12.0));
        assert_eq!(metrics.steps_per_second, Some(0.8));
        assert_eq!(metrics.tokens_per_second, Some(2048.0));
    }

    #[test]
    fn test_csv_alias_expansion_for_common_framework_headers() {
        let parser =
            CsvParser::new("train.loss,optimizer.lr,train.global_step,speed.samples_per_second")
                .expect("parser should be created");
        let result = parser
            .parse_line("0.6,0.0002,42,18.5")
            .expect("parse should succeed");

        assert!(result.is_some());
        let metrics = result.expect("metrics should exist");
        assert_eq!(metrics.loss, Some(0.6));
        assert_eq!(metrics.learning_rate, Some(0.0002));
        assert_eq!(metrics.step, Some(42));
        assert_eq!(metrics.samples_per_second, Some(18.5));
    }

    #[test]
    fn test_csv_parse_integral_float_u64_fields() {
        let parser = CsvParser::new("step,tokens").expect("parser should be created");
        let result = parser
            .parse_line("10.0,5000.0")
            .expect("parse should succeed");

        assert!(result.is_some());
        let metrics = result.expect("metrics should exist");
        assert_eq!(metrics.step, Some(10));
        assert_eq!(metrics.tokens, Some(5000));
    }

    #[test]
    fn test_csv_parse_rejects_non_integral_u64_fields() {
        let parser = CsvParser::new("step,tokens").expect("parser should be created");
        let result = parser
            .parse_line("10.5,5000.25")
            .expect("parse should succeed");

        assert!(result.is_none());
    }

    #[test]
    fn test_csv_parser_new_with_bom_header() {
        let parser = CsvParser::new("\u{feff}loss,step,lr").expect("parser should be created");
        let result = parser
            .parse_line("0.4,20,0.001")
            .expect("parse should succeed");

        assert!(result.is_some());
        let metrics = result.expect("metrics should exist");
        assert_eq!(metrics.loss, Some(0.4));
        assert_eq!(metrics.step, Some(20));
    }
}
