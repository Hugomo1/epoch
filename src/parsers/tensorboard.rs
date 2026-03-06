use color_eyre::Result;
use std::sync::Once;

use super::LogParser;
use crate::types::TrainingMetrics;

pub struct TensorboardParser;

static TENSORBOARD_NOTICE: Once = Once::new();

impl LogParser for TensorboardParser {
    fn parse_line(&self, line: &str) -> Result<Option<TrainingMetrics>> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }

        TENSORBOARD_NOTICE.call_once(|| {
            tracing::warn!(
                "TensorBoard parser selected for line stream; metric extraction is currently disabled"
            );
        });
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tensorboard_parser_instantiation() {
        let _parser = TensorboardParser;
    }

    #[test]
    fn test_tensorboard_parser_skips_non_empty_line_gracefully() {
        let parser = TensorboardParser;
        let result = parser
            .parse_line("this is not a tensorboard event record")
            .expect("tensorboard parser should not fail on plain text");
        assert!(result.is_none());
    }

    #[test]
    fn test_tensorboard_parser_skips_empty_line() {
        let parser = TensorboardParser;
        let result = parser
            .parse_line("   ")
            .expect("tensorboard parser should skip whitespace");
        assert!(result.is_none());
    }
}
