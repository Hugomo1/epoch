use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use color_eyre::{Result, eyre::ContextCompat};
use notify::Watcher;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::config::Config;
use crate::parsers::LogParser;
use crate::parsers::csv::CsvParser;
use crate::parsers::detect_parser;
use crate::parsers::hf_trainer::parse_trainer_state;
use crate::parsers::jsonl::JsonlParser;
use crate::parsers::regex_parser::RegexParser;
use crate::parsers::tensorboard::TensorboardParser;
use crate::types::TrainingMetrics;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ParserTelemetrySnapshot {
    pub success_count: u64,
    pub skipped_count: u64,
    pub error_count: u64,
}

static PARSE_SUCCESS_COUNT: AtomicU64 = AtomicU64::new(0);
static PARSE_SKIPPED_COUNT: AtomicU64 = AtomicU64::new(0);
static PARSE_ERROR_COUNT: AtomicU64 = AtomicU64::new(0);

fn reset_parser_telemetry() {
    PARSE_SUCCESS_COUNT.store(0, Ordering::Relaxed);
    PARSE_SKIPPED_COUNT.store(0, Ordering::Relaxed);
    PARSE_ERROR_COUNT.store(0, Ordering::Relaxed);
}

fn record_parse_outcome(result: &Result<Option<TrainingMetrics>>) {
    match result {
        Ok(Some(_)) => {
            PARSE_SUCCESS_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        Ok(None) => {
            PARSE_SKIPPED_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        Err(_) => {
            PARSE_ERROR_COUNT.fetch_add(1, Ordering::Relaxed);
        }
    }
}

pub fn parser_telemetry_snapshot() -> ParserTelemetrySnapshot {
    ParserTelemetrySnapshot {
        success_count: PARSE_SUCCESS_COUNT.load(Ordering::Relaxed),
        skipped_count: PARSE_SKIPPED_COUNT.load(Ordering::Relaxed),
        error_count: PARSE_ERROR_COUNT.load(Ordering::Relaxed),
    }
}

struct CsvBootstrapParser {
    parser: Mutex<Option<CsvParser>>,
}

impl CsvBootstrapParser {
    fn new() -> Self {
        Self {
            parser: Mutex::new(None),
        }
    }
}

impl LogParser for CsvBootstrapParser {
    fn parse_line(&self, line: &str) -> Result<Option<TrainingMetrics>> {
        let mut guard = self
            .parser
            .lock()
            .map_err(|_| color_eyre::eyre::eyre!("csv parser mutex poisoned"))?;

        if let Some(parser) = guard.as_ref() {
            return parser.parse_line(line);
        }

        if let Ok(parser) = CsvParser::new(line) {
            *guard = Some(parser);
        }

        Ok(None)
    }
}

enum AutoState {
    Undetected,
    Jsonl,
    Csv(CsvParser),
}

struct AutoDetectingParser {
    state: Mutex<AutoState>,
}

impl AutoDetectingParser {
    fn new() -> Self {
        Self {
            state: Mutex::new(AutoState::Undetected),
        }
    }
}

impl LogParser for AutoDetectingParser {
    fn parse_line(&self, line: &str) -> Result<Option<TrainingMetrics>> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| color_eyre::eyre::eyre!("auto parser mutex poisoned"))?;

        match &mut *guard {
            AutoState::Undetected => {
                let jsonl = JsonlParser;
                if let Some(metrics) = jsonl.parse_line(line)? {
                    *guard = AutoState::Jsonl;
                    return Ok(Some(metrics));
                }

                if let Ok(csv) = CsvParser::new(line) {
                    *guard = AutoState::Csv(csv);
                    return Ok(None);
                }

                Ok(None)
            }
            AutoState::Jsonl => JsonlParser.parse_line(line),
            AutoState::Csv(csv) => csv.parse_line(line),
        }
    }
}

fn read_sample_lines(path: &PathBuf, max_lines: usize) -> Vec<String> {
    let Ok(file) = std::fs::File::open(path) else {
        return vec![];
    };

    let mut out = Vec::with_capacity(max_lines);
    for_each_lossy_line(BufReader::new(file), |line| {
        if out.len() >= max_lines {
            return;
        }
        let normalized = normalize_line(&line);
        if !normalized.is_empty() {
            out.push(normalized);
        }
    });
    out
}

fn read_first_non_empty_line(path: &PathBuf) -> Result<String> {
    let file = std::fs::File::open(path)?;
    let mut first = None;
    for_each_lossy_line(BufReader::new(file), |line| {
        if first.is_some() {
            return;
        }
        let normalized = normalize_line(&line);
        if !normalized.is_empty() {
            first = Some(normalized);
        }
    });

    if let Some(line) = first {
        return Ok(line);
    }

    color_eyre::eyre::bail!("csv parser requires a non-empty header line")
}

fn for_each_lossy_line<R, F>(mut reader: R, mut on_line: F)
where
    R: BufRead,
    F: FnMut(String),
{
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        match reader.read_until(b'\n', &mut buffer) {
            Ok(0) => break,
            Ok(_) => {
                if buffer.last() == Some(&b'\n') {
                    buffer.pop();
                }
                if buffer.last() == Some(&b'\r') {
                    buffer.pop();
                }
                on_line(String::from_utf8_lossy(&buffer).into_owned());
            }
            Err(err) => {
                tracing::debug!("line read error: {err}");
                break;
            }
        }
    }
}

fn normalize_line(line: &str) -> String {
    let without_trailing_cr = line.trim_end_matches('\r');
    let segment = without_trailing_cr
        .rsplit('\r')
        .find(|part| !part.is_empty())
        .unwrap_or("");

    let mut out = String::with_capacity(segment.len());
    let mut chars = segment.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            match chars.peek().copied() {
                Some('[') => {
                    let _ = chars.next();
                    for c in chars.by_ref() {
                        if ('@'..='~').contains(&c) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    let _ = chars.next();
                    let mut saw_esc = false;
                    for c in chars.by_ref() {
                        if c == '\u{0007}' || (saw_esc && c == '\\') {
                            break;
                        }
                        saw_esc = c == '\u{1b}';
                    }
                }
                Some(_) => {
                    let _ = chars.next();
                }
                None => {}
            }

            continue;
        }

        if ch.is_control() && ch != '\t' {
            continue;
        }

        out.push(ch);
    }

    out.trim().trim_start_matches('\u{feff}').trim().to_string()
}

pub fn create_parser(config: &Config) -> Result<Box<dyn LogParser + Send>> {
    reset_parser_telemetry();

    match config.parser.as_str() {
        "jsonl" => Ok(Box::new(JsonlParser)),
        "csv" => {
            if let Some(path) = config.log_file.as_ref()
                && let Ok(header) = read_first_non_empty_line(path)
            {
                return Ok(Box::new(CsvParser::new(&header)?));
            }

            Ok(Box::new(CsvBootstrapParser::new()))
        }
        "regex" => {
            let pattern = config
                .regex_pattern
                .as_deref()
                .context("regex_pattern required when parser is 'regex'")?;
            Ok(Box::new(RegexParser::new(pattern)?))
        }
        "tensorboard" => Ok(Box::new(TensorboardParser)),
        "auto" => {
            if let Some(path) = config.log_file.as_ref() {
                let sample_lines = read_sample_lines(path, 20);
                if !sample_lines.is_empty() {
                    let sample_refs: Vec<&str> = sample_lines.iter().map(String::as_str).collect();
                    return Ok(detect_parser(&sample_refs));
                }
            }
            Ok(Box::new(AutoDetectingParser::new()))
        }
        _ => Ok(Box::new(JsonlParser)),
    }
}

pub fn spawn_stdin_reader(
    parser: Box<dyn LogParser + Send>,
    tx: mpsc::Sender<TrainingMetrics>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        use tokio::io::{AsyncBufReadExt, BufReader};

        let stdin = tokio::io::stdin();
        let reader = BufReader::new(stdin);
        let mut segments = reader.split(b'\n');

        loop {
            let bytes = match segments.next_segment().await {
                Ok(Some(bytes)) => bytes,
                Ok(None) => break,
                Err(err) => {
                    PARSE_ERROR_COUNT.fetch_add(1, Ordering::Relaxed);
                    tracing::debug!("stdin read error: {err}");
                    continue;
                }
            };
            let line = String::from_utf8_lossy(&bytes).into_owned();

            let normalized = normalize_line(&line);
            if normalized.is_empty() {
                PARSE_SKIPPED_COUNT.fetch_add(1, Ordering::Relaxed);
                continue;
            }

            let parse_result = parser.parse_line(&normalized);
            record_parse_outcome(&parse_result);

            match parse_result {
                Ok(Some(metrics)) => {
                    if tx.send(metrics).await.is_err() {
                        break;
                    }
                }
                Ok(None) => {}
                Err(err) => {
                    tracing::debug!("parse error: {err}");
                }
            }
        }
    })
}

pub fn spawn_file_watcher(
    path: PathBuf,
    parser: Box<dyn LogParser + Send>,
    tx: mpsc::Sender<TrainingMetrics>,
) -> Result<JoinHandle<()>> {
    if let Some(parent) = path.parent()
        && !parent.exists()
    {
        color_eyre::eyre::bail!("parent directory does not exist: {}", parent.display());
    }

    Ok(tokio::spawn(async move {
        use notify::{Config as NotifyConfig, RecommendedWatcher, RecursiveMode};

        // NOTE: notify crate requires std::sync::mpsc::Sender (unbounded).
        // Back-pressure is applied via the bounded tokio channel bridge below.
        let (notify_tx, notify_rx) = std::sync::mpsc::channel();
        let _watcher_guard = {
            let mut watcher = match RecommendedWatcher::new(notify_tx, NotifyConfig::default()) {
                Ok(watcher) => watcher,
                Err(err) => {
                    tracing::error!("failed to create file watcher: {err}");
                    return;
                }
            };

            let watch_path = if path.exists() {
                path.as_path()
            } else {
                path.parent().unwrap_or(path.as_path())
            };

            if let Err(err) = watcher.watch(watch_path, RecursiveMode::NonRecursive) {
                tracing::error!("failed to watch path {}: {err}", watch_path.display());
                return;
            }

            watcher
        };

        let mut position: u64 = 0;

        if path.exists() {
            if let Ok(file) = std::fs::File::open(&path) {
                let mut initial_metrics = Vec::new();
                for_each_lossy_line(BufReader::new(file), |line| {
                    let normalized = normalize_line(&line);
                    if normalized.is_empty() {
                        PARSE_SKIPPED_COUNT.fetch_add(1, Ordering::Relaxed);
                        return;
                    }

                    let parse_result = parser.parse_line(&normalized);
                    record_parse_outcome(&parse_result);

                    match parse_result {
                        Ok(Some(metrics)) => {
                            initial_metrics.push(metrics);
                        }
                        Ok(None) => {}
                        Err(err) => {
                            tracing::debug!("parse error: {err}");
                        }
                    }
                });
                for metrics in initial_metrics {
                    if tx.send(metrics).await.is_err() {
                        return;
                    }
                }
                position = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            }
        } else {
            tracing::info!("waiting for {} to be created...", path.display());
        }

        let (async_tx, mut async_rx) = mpsc::channel::<()>(16);
        tokio::task::spawn_blocking(move || {
            for event in notify_rx.into_iter().flatten() {
                use notify::event::{EventKind, ModifyKind};

                if matches!(
                    event.kind,
                    EventKind::Modify(ModifyKind::Data(_))
                        | EventKind::Modify(ModifyKind::Any)
                        | EventKind::Create(_)
                ) && async_tx.blocking_send(()).is_err()
                {
                    break;
                }
            }
        });

        while async_rx.recv().await.is_some() {
            if !path.exists() {
                continue;
            }

            let current_size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

            if current_size < position {
                position = 0;
            }

            if current_size > position
                && let Ok(mut file) = std::fs::File::open(&path)
            {
                if file.seek(SeekFrom::Start(position)).is_ok() {
                    let mut new_metrics = Vec::new();
                    for_each_lossy_line(BufReader::new(&file), |line| {
                        let normalized = normalize_line(&line);
                        if normalized.is_empty() {
                            PARSE_SKIPPED_COUNT.fetch_add(1, Ordering::Relaxed);
                            return;
                        }

                        let parse_result = parser.parse_line(&normalized);
                        record_parse_outcome(&parse_result);

                        match parse_result {
                            Ok(Some(metrics)) => {
                                new_metrics.push(metrics);
                            }
                            Ok(None) => {}
                            Err(err) => {
                                tracing::debug!("parse error: {err}");
                            }
                        }
                    });
                    for metrics in new_metrics {
                        if tx.send(metrics).await.is_err() {
                            return;
                        }
                    }
                }

                position = current_size;
            }
        }
    }))
}

pub fn spawn_trainer_state_poller(
    path: PathBuf,
    tx: mpsc::Sender<TrainingMetrics>,
    interval: std::time::Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        let mut last_modified = std::time::UNIX_EPOCH;
        let mut last_emitted_step: Option<u64> = None;

        loop {
            ticker.tick().await;

            let Ok(metadata) = std::fs::metadata(&path) else {
                continue;
            };
            let Ok(modified) = metadata.modified() else {
                continue;
            };

            if modified <= last_modified {
                continue;
            }
            last_modified = modified;

            let content = match std::fs::read_to_string(&path) {
                Ok(content) => content,
                Err(err) => {
                    PARSE_ERROR_COUNT.fetch_add(1, Ordering::Relaxed);
                    tracing::debug!("trainer_state read error: {err}");
                    continue;
                }
            };
            let metrics_list = match parse_trainer_state(&content) {
                Ok(list) => list,
                Err(err) => {
                    PARSE_ERROR_COUNT.fetch_add(1, Ordering::Relaxed);
                    tracing::debug!("trainer_state parse error: {err}");
                    continue;
                }
            };

            let newest_step = metrics_list.iter().filter_map(|m| m.step).max();
            if let (Some(previous), Some(newest)) = (last_emitted_step, newest_step)
                && newest < previous
            {
                last_emitted_step = None;
            }

            for metrics in metrics_list {
                let should_send = metrics.step.is_some_and(|step| {
                    last_emitted_step
                        .map(|previous| step > previous)
                        .unwrap_or(true)
                });
                if should_send {
                    last_emitted_step = metrics.step;
                    if tx.send(metrics).await.is_err() {
                        return;
                    }
                    PARSE_SUCCESS_COUNT.fetch_add(1, Ordering::Relaxed);
                } else {
                    PARSE_SKIPPED_COUNT.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use tokio::sync::mpsc;
    use tokio::time::timeout;

    use crate::config::Config;

    #[test]
    fn test_normalization_strips_ansi_sequences() {
        let line = "\u{1b}[32m{\"loss\":0.5,\"step\":10}\u{1b}[0m";
        let normalized = normalize_line(line);
        assert_eq!(normalized, "{\"loss\":0.5,\"step\":10}");
    }

    #[test]
    fn test_normalization_handles_carriage_return_progress() {
        let line = "Epoch 1 loss=0.9\r{\"loss\":0.8,\"step\":20}";
        let normalized = normalize_line(line);
        assert_eq!(normalized, "{\"loss\":0.8,\"step\":20}");
    }

    #[test]
    fn test_normalization_handles_trailing_crlf_shape() {
        let line = "{\"loss\":1.0,\"step\":10}\r";
        let normalized = normalize_line(line);
        assert_eq!(normalized, "{\"loss\":1.0,\"step\":10}");
    }

    #[test]
    fn test_normalization_preserves_numeric_precision() {
        let line = "{\"loss\":0.123456789,\"learning_rate\":1e-7,\"step\":100}";
        let normalized = normalize_line(line);
        assert_eq!(normalized, line);
    }

    #[test]
    fn test_normalization_invalid_control_bytes_graceful_skip() {
        let line = "\u{0007}\u{0001}\u{0002}";
        let normalized = normalize_line(line);
        assert!(normalized.is_empty());
    }

    #[test]
    fn test_normalization_strips_osc_title_sequence() {
        let line = "\u{1b}]0;Epoch\u{0007}{\"loss\":0.4,\"step\":12}";
        let normalized = normalize_line(line);
        assert_eq!(normalized, "{\"loss\":0.4,\"step\":12}");
    }

    #[test]
    fn test_normalization_strips_osc_hyperlink_sequence() {
        let line = "\u{1b}]8;;https://example.com\u{1b}\\click\u{1b}]8;;\u{1b}\\ {\"step\":7}";
        let normalized = normalize_line(line);
        assert_eq!(normalized, "click {\"step\":7}");
    }

    #[test]
    fn test_normalization_strips_utf8_bom() {
        let line = "\u{feff}{\"loss\":0.7,\"step\":3}";
        let normalized = normalize_line(line);
        assert_eq!(normalized, "{\"loss\":0.7,\"step\":3}");
    }

    #[test]
    fn test_parser_telemetry_counters_increment() {
        reset_parser_telemetry();

        record_parse_outcome(&Ok(Some(TrainingMetrics::default())));
        record_parse_outcome(&Ok(None));
        record_parse_outcome(&Err(color_eyre::eyre::eyre!("parse failed")));

        let snapshot = parser_telemetry_snapshot();
        assert_eq!(snapshot.success_count, 1);
        assert_eq!(snapshot.skipped_count, 1);
        assert_eq!(snapshot.error_count, 1);
    }

    #[tokio::test]
    async fn test_create_parser_jsonl() {
        let config = Config {
            parser: "jsonl".to_string(),
            ..Config::default()
        };

        let parser = create_parser(&config).expect("jsonl parser should be created");
        let parsed = parser
            .parse_line(r#"{"loss": 0.25, "step": 10}"#)
            .expect("jsonl parse should succeed");

        assert!(parsed.is_some());
        assert_eq!(parsed.expect("metrics should exist").loss, Some(0.25));
    }

    #[tokio::test]
    async fn test_create_parser_auto() {
        let config = Config {
            parser: "auto".to_string(),
            ..Config::default()
        };

        let parser = create_parser(&config).expect("auto parser should be created");
        let parsed = parser
            .parse_line(r#"{"step": 99}"#)
            .expect("default parser parse should succeed");

        assert!(parsed.is_some());
        assert_eq!(parsed.expect("metrics should exist").step, Some(99));
    }

    #[tokio::test]
    async fn test_create_parser_auto_detects_csv_from_file() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("epoch-training-csv-test-{unique}"));
        fs::create_dir_all(&root).expect("test directory should be created");
        let file_path = root.join("train.csv");
        fs::write(&file_path, "loss,step,lr\n0.5,100,0.001\n")
            .expect("test csv file should be written");

        let config = Config {
            parser: "auto".to_string(),
            log_file: Some(file_path.clone()),
            ..Config::default()
        };

        let parser = create_parser(&config).expect("auto parser should be created");
        let parsed = parser
            .parse_line("0.5,100,0.001")
            .expect("auto-detected csv parser should parse csv data");

        assert!(parsed.is_some());
        let metrics = parsed.expect("metrics should exist");
        assert_eq!(metrics.loss, Some(0.5));
        assert_eq!(metrics.step, Some(100));
        assert_eq!(metrics.learning_rate, Some(0.001));

        fs::remove_file(&file_path).expect("test file should be removed");
        fs::remove_dir_all(&root).expect("test directory should be removed");
    }

    #[tokio::test]
    async fn test_create_parser_auto_detects_csv_from_ansi_header() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("epoch-parser-auto-ansi-{unique}"));
        fs::create_dir_all(&root).expect("test directory should be created");
        let file_path = root.join("train.csv");
        fs::write(
            &file_path,
            "\u{1b}[32mloss,step,lr\u{1b}[0m\n0.5,100,0.001\n",
        )
        .expect("csv fixture should be written");

        let config = Config {
            parser: "auto".to_string(),
            log_file: Some(file_path.clone()),
            ..Config::default()
        };

        let parser = create_parser(&config).expect("auto parser should be created");
        let parsed = parser
            .parse_line("0.5,100,0.001")
            .expect("csv row should parse");

        assert!(parsed.is_some());
        let metrics = parsed.expect("metrics should exist");
        assert_eq!(metrics.loss, Some(0.5));
        assert_eq!(metrics.step, Some(100));
        assert_eq!(metrics.learning_rate, Some(0.001));

        fs::remove_file(&file_path).expect("test file should be removed");
        fs::remove_dir_all(&root).expect("test directory should be removed");
    }

    #[tokio::test]
    async fn test_create_parser_auto_survives_invalid_utf8_prefix_line() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("epoch-parser-auto-invalid-utf8-{unique}"));
        fs::create_dir_all(&root).expect("test directory should be created");
        let file_path = root.join("train.csv");

        let mut bytes = vec![0xff, 0xfe, b'\n'];
        bytes.extend_from_slice(b"loss,step,lr\n0.5,100,0.001\n");
        fs::write(&file_path, bytes).expect("csv fixture should be written");

        let config = Config {
            parser: "auto".to_string(),
            log_file: Some(file_path.clone()),
            ..Config::default()
        };

        let parser = create_parser(&config).expect("auto parser should be created");
        let parsed = parser
            .parse_line("0.5,100,0.001")
            .expect("csv row should parse");

        assert!(parsed.is_some());
        let metrics = parsed.expect("metrics should exist");
        assert_eq!(metrics.loss, Some(0.5));
        assert_eq!(metrics.step, Some(100));

        fs::remove_file(&file_path).expect("test file should be removed");
        fs::remove_dir_all(&root).expect("test directory should be removed");
    }

    #[tokio::test]
    async fn test_create_parser_auto_detects_csv_after_header_line() {
        let config = Config {
            parser: "auto".to_string(),
            log_file: Some(std::env::temp_dir().join("epoch-does-not-exist.csv")),
            ..Config::default()
        };

        let parser = create_parser(&config).expect("auto parser should be created");
        assert!(
            parser
                .parse_line("loss,step,lr")
                .expect("header parse should succeed")
                .is_none()
        );

        let parsed = parser
            .parse_line("0.5,100,0.001")
            .expect("csv parse should succeed");
        assert!(parsed.is_some());
        let metrics = parsed.expect("metrics should exist");
        assert_eq!(metrics.loss, Some(0.5));
        assert_eq!(metrics.step, Some(100));
    }

    #[tokio::test]
    async fn test_auto_detecting_parser_switches_to_csv_after_noise_semicolon_header() {
        let config = Config {
            parser: "auto".to_string(),
            log_file: Some(std::env::temp_dir().join("epoch-does-not-exist-semicolon.csv")),
            ..Config::default()
        };

        let parser = create_parser(&config).expect("auto parser should be created");
        assert!(
            parser
                .parse_line("INFO startup")
                .expect("noise should not fail")
                .is_none()
        );
        assert!(
            parser
                .parse_line("loss;step;lr")
                .expect("header parse should succeed")
                .is_none()
        );

        let parsed = parser
            .parse_line("0.5;100;0.001")
            .expect("csv parse should succeed");
        assert!(parsed.is_some());
        let metrics = parsed.expect("metrics should exist");
        assert_eq!(metrics.loss, Some(0.5));
        assert_eq!(metrics.step, Some(100));
        assert_eq!(metrics.learning_rate, Some(0.001));
    }

    #[tokio::test]
    async fn test_create_parser_regex_with_pattern() {
        let config = Config {
            parser: "regex".to_string(),
            regex_pattern: Some(r"step=(?P<step>\d+) loss=(?P<loss>[\d.]+)".to_string()),
            ..Config::default()
        };

        let parser = create_parser(&config).expect("regex parser should be created");
        let parsed = parser
            .parse_line("step=42 loss=0.123")
            .expect("regex parse should succeed");

        assert!(parsed.is_some());
        let metrics = parsed.expect("metrics should exist");
        assert_eq!(metrics.step, Some(42));
        assert_eq!(metrics.loss, Some(0.123));
    }

    #[tokio::test]
    async fn test_create_parser_regex_without_pattern() {
        let config = Config {
            parser: "regex".to_string(),
            regex_pattern: None,
            ..Config::default()
        };

        let result = create_parser(&config);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_parser_unknown_defaults_to_jsonl() {
        let config = Config {
            parser: "something-new".to_string(),
            ..Config::default()
        };

        let parser = create_parser(&config).expect("unknown parser should default to jsonl");
        let parsed = parser
            .parse_line(r#"{"lr": 0.001}"#)
            .expect("jsonl parse should succeed");

        assert!(parsed.is_some());
        assert_eq!(
            parsed.expect("metrics should exist").learning_rate,
            Some(0.001)
        );
    }

    #[tokio::test]
    async fn test_create_parser_tensorboard_override() {
        let config = Config {
            parser: "tensorboard".to_string(),
            ..Config::default()
        };

        let parser = create_parser(&config).expect("tensorboard parser should be created");
        let parsed = parser
            .parse_line("not a tensorboard event stream")
            .expect("tensorboard parser should skip plain text safely");

        assert!(parsed.is_none());
    }

    #[tokio::test]
    async fn test_create_parser_csv_bootstrap_detects_header_then_rows() {
        let config = Config {
            parser: "csv".to_string(),
            log_file: None,
            ..Config::default()
        };

        let parser = create_parser(&config).expect("csv bootstrap parser should be created");
        assert!(
            parser
                .parse_line("loss,step,lr")
                .expect("csv header should be accepted")
                .is_none()
        );

        let parsed = parser
            .parse_line("0.5,100,0.001")
            .expect("csv row should parse");
        assert!(parsed.is_some());
        let metrics = parsed.expect("metrics should exist");
        assert_eq!(metrics.loss, Some(0.5));
        assert_eq!(metrics.step, Some(100));
        assert_eq!(metrics.learning_rate, Some(0.001));
    }

    #[tokio::test]
    async fn test_file_watcher_reads_existing_content() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("epoch-training-test-{unique}"));
        fs::create_dir_all(&root).expect("test directory should be created");
        let file_path = root.join("train.log");

        let contents = [
            r#"{"loss": 0.9, "step": 1}"#,
            "not-json",
            r#"{"loss": 0.8, "step": 2}"#,
        ]
        .join("\n");
        fs::write(&file_path, contents).expect("test log file should be written");

        let config = Config {
            parser: "jsonl".to_string(),
            ..Config::default()
        };
        let parser = create_parser(&config).expect("jsonl parser should be created");
        let (tx, mut rx) = mpsc::channel(8);

        let handle = spawn_file_watcher(file_path.clone(), parser, tx)
            .expect("watcher should spawn for existing file");

        let first = timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("first recv should complete")
            .expect("first metric should exist");
        let second = timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("second recv should complete")
            .expect("second metric should exist");

        assert_eq!(first.step, Some(1));
        assert_eq!(second.step, Some(2));

        handle.abort();
        fs::remove_file(&file_path).expect("test file should be removed");
        fs::remove_dir_all(&root).expect("test directory should be removed");
    }

    #[tokio::test]
    async fn test_file_watcher_missing_parent_dir() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let missing_parent =
            std::env::temp_dir().join(format!("epoch-nonexistent-parent-{unique}/child"));
        let path = missing_parent.join("train.log");

        let config = Config {
            parser: "jsonl".to_string(),
            ..Config::default()
        };
        let parser = create_parser(&config).expect("jsonl parser should be created");
        let (tx, _rx) = mpsc::channel(8);

        let result = spawn_file_watcher(path, parser, tx);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_trainer_state_poller_emits_new_entries_on_rewrite() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("epoch-trainer-state-test-{unique}"));
        fs::create_dir_all(&root).expect("test directory should be created");
        let file_path = root.join("trainer_state.json");

        fs::write(
            &file_path,
            r#"{"log_history":[{"loss":1.0,"learning_rate":0.001,"step":10}]}"#,
        )
        .expect("initial trainer state should be written");

        let (tx, mut rx) = mpsc::channel(8);
        let handle = spawn_trainer_state_poller(file_path.clone(), tx, Duration::from_millis(100));

        let first = timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("first recv should complete")
            .expect("first metric should exist");
        assert_eq!(first.step, Some(10));

        fs::write(
            &file_path,
            r#"{"log_history":[{"loss":1.0,"learning_rate":0.001,"step":10},{"loss":0.9,"learning_rate":0.001,"step":20}]}"#,
        )
        .expect("rewritten trainer state should be written");

        let second = timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("second recv should complete")
            .expect("second metric should exist");
        assert_eq!(second.step, Some(20));

        handle.abort();
        fs::remove_file(&file_path).expect("test file should be removed");
        fs::remove_dir_all(&root).expect("test directory should be removed");
    }

    #[tokio::test]
    async fn test_trainer_state_poller_emits_step_zero() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("epoch-trainer-state-zero-{unique}"));
        fs::create_dir_all(&root).expect("test directory should be created");
        let file_path = root.join("trainer_state.json");

        fs::write(
            &file_path,
            r#"{"log_history":[{"loss":1.0,"learning_rate":0.001,"step":0}]}"#,
        )
        .expect("initial trainer state should be written");

        let (tx, mut rx) = mpsc::channel(8);
        let handle = spawn_trainer_state_poller(file_path.clone(), tx, Duration::from_millis(100));

        let first = timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("recv should complete")
            .expect("metric should exist");
        assert_eq!(first.step, Some(0));

        handle.abort();
        fs::remove_file(&file_path).expect("test file should be removed");
        fs::remove_dir_all(&root).expect("test directory should be removed");
    }

    #[tokio::test]
    async fn test_trainer_state_poller_updates_parse_telemetry() {
        reset_parser_telemetry();

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("epoch-trainer-telemetry-{unique}"));
        fs::create_dir_all(&root).expect("test directory should be created");
        let file_path = root.join("trainer_state.json");

        fs::write(
            &file_path,
            r#"{"log_history":[{"loss":1.0,"step":1},{"loss":0.9,"step":2}]}"#,
        )
        .expect("trainer state should be written");

        let (tx, mut rx) = mpsc::channel(8);
        let handle = spawn_trainer_state_poller(file_path.clone(), tx, Duration::from_millis(100));

        let _first = timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("first recv should complete")
            .expect("first metric should exist");
        let _second = timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("second recv should complete")
            .expect("second metric should exist");

        let snapshot = parser_telemetry_snapshot();
        assert!(snapshot.success_count >= 2);

        handle.abort();
        fs::remove_file(&file_path).expect("test file should be removed");
        fs::remove_dir_all(&root).expect("test directory should be removed");
    }
}
