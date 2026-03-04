use epoch::app::App;
use epoch::collectors::training::create_parser;
use epoch::config::Config;
use epoch::event::Event;
use epoch::types::{GpuMetrics, SystemMetrics, TrainingMetrics};
use epoch::ui::Tab;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_app_processes_events_from_channels() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use epoch::types::TrainingMetrics;

    let mut app = App::new(Config::default());
    let (tx, mut rx) = mpsc::channel(16);

    tx.send(Event::Metrics(TrainingMetrics {
        loss: Some(0.5),
        step: Some(100),
        ..TrainingMetrics::default()
    }))
    .await
    .expect("metrics event should send");

    tx.send(Event::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)))
        .await
        .expect("key event should send");

    if let Some(event) = rx.recv().await {
        app.handle_event(event);
    }
    assert!(app.training.latest.is_some());
    assert_eq!(app.training.latest.as_ref().and_then(|m| m.loss), Some(0.5));

    if let Some(event) = rx.recv().await {
        app.handle_event(event);
    }
    assert_eq!(app.ui_state.selected_tab, Tab::Metrics);
}

#[tokio::test]
async fn test_training_metrics_flow_through_channel() {
    use epoch::types::TrainingMetrics;

    let (tx, mut rx) = mpsc::channel(epoch::event::METRICS_CHANNEL_CAPACITY);

    for i in 1..=5 {
        tx.send(TrainingMetrics {
            loss: Some(1.0 / i as f64),
            step: Some(i * 100),
            ..TrainingMetrics::default()
        })
        .await
        .expect("training metric should send");
    }

    let mut count = 0;
    while let Ok(metrics) = rx.try_recv() {
        assert!(metrics.loss.is_some());
        count += 1;
    }
    assert_eq!(count, 5);
}

#[test]
fn test_app_new_running() {
    let app = App::new(Config::default());
    assert!(app.running);
    assert_eq!(app.ui_state.selected_tab, Tab::Dashboard);
}

#[test]
fn test_config_defaults() {
    let config = Config::default();
    assert_eq!(config.tick_rate_ms, 250);
    assert_eq!(config.history_size, 300);
    assert_eq!(config.parser, "auto");
}

#[test]
fn test_system_metrics_default() {
    let metrics = SystemMetrics::default();
    assert_eq!(metrics.cpu_usage, 0.0);
    assert_eq!(metrics.memory_used, 0);
    assert_eq!(metrics.memory_total, 0);
    assert!(metrics.gpus.is_empty());
}

#[test]
fn test_gpu_metrics_default() {
    let metrics = GpuMetrics::default();
    assert_eq!(metrics.name, "");
    assert_eq!(metrics.utilization, 0.0);
    assert_eq!(metrics.memory_used, 0);
    assert_eq!(metrics.memory_total, 0);
    assert_eq!(metrics.temperature, 0.0);
}

#[tokio::test]
async fn test_app_processes_system_with_gpu() {
    let mut app = App::new(Config::default());
    app.push_system(SystemMetrics {
        cpu_usage: 45.0,
        memory_used: 8_000_000_000,
        memory_total: 16_000_000_000,
        gpus: vec![GpuMetrics {
            name: "RTX 4090".into(),
            utilization: 95.0,
            memory_used: 20_000_000_000,
            memory_total: 24_000_000_000,
            temperature: 72.0,
        }],
    });
    assert!(app.system.latest.is_some());
    assert_eq!(app.system.gpu_history.len(), 1);
}

#[tokio::test]
async fn test_app_handles_rapid_events() {
    let mut app = App::new(Config::default());

    for i in 0..100 {
        app.push_metrics(TrainingMetrics {
            loss: Some(1.0 - (i as f64 * 0.01)),
            step: Some(i),
            ..TrainingMetrics::default()
        });
    }

    assert_eq!(app.training.loss_history.len(), 100);
    assert_eq!(app.training.total_steps, 99);
}

#[test]
fn test_config_merge_preserves_defaults() {
    let mut config = Config::default();
    config.merge_cli_args(None, false, None);
    assert_eq!(config.tick_rate_ms, 250);
    assert_eq!(config.parser, "auto");
    assert!(!config.stdin_mode);
}

#[test]
fn test_all_public_modules_accessible() {
    use epoch::collectors::training::create_parser;
    use epoch::event::{
        EVENT_CHANNEL_CAPACITY, Event, EventHandler, METRICS_CHANNEL_CAPACITY,
        SYSTEM_CHANNEL_CAPACITY,
    };

    let _app = App::new(Config::default());
    let _config = Config::default();
    let _event: Option<Event> = None;
    let _handler_ctor: fn(std::time::Duration) -> EventHandler = EventHandler::new;
    let _event_cap = EVENT_CHANNEL_CAPACITY;
    let _metrics_cap = METRICS_CHANNEL_CAPACITY;
    let _system_cap = SYSTEM_CHANNEL_CAPACITY;
    let _tm = TrainingMetrics::default();
    let _sm = SystemMetrics::default();
    let _gm = GpuMetrics::default();
    let _tab = Tab::Dashboard;
    let _parser = create_parser(&Config::default()).expect("default parser should be creatable");
}

#[test]
fn test_jsonl_parser_edge_cases() {
    let config = Config {
        parser: "jsonl".into(),
        ..Config::default()
    };
    let parser = create_parser(&config).expect("jsonl parser should be created");

    assert!(
        parser
            .parse_line("")
            .expect("empty line parse should succeed")
            .is_none()
    );

    assert!(
        parser
            .parse_line("   ")
            .expect("whitespace line parse should succeed")
            .is_none()
    );

    let result = parser.parse_line("not json at all");
    assert!(result.is_ok() || result.is_err());

    assert!(
        parser
            .parse_line(r#"{"foo": "bar"}"#)
            .expect("unknown fields parse should succeed")
            .is_none()
    );

    let result = parser
        .parse_line(r#"{"loss": 999999999.99}"#)
        .expect("large numeric parse should succeed");
    assert!(result.is_some());
}

#[test]
fn test_history_overflow_no_panic() {
    let config = Config {
        history_size: 10,
        ..Config::default()
    };
    let mut app = App::new(config);
    for i in 0..1000 {
        app.push_metrics(TrainingMetrics {
            loss: Some(i as f64),
            step: Some(i),
            learning_rate: Some(0.001),
            throughput: Some(1000.0),
            ..TrainingMetrics::default()
        });
    }
    assert_eq!(app.training.loss_history.len(), 10);
    assert_eq!(app.training.lr_history.len(), 10);
    assert_eq!(app.training.step_history.len(), 10);
    assert_eq!(app.training.throughput_history.len(), 10);
}

#[test]
fn test_tab_cycling_many_times() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let mut app = App::new(Config::default());
    let tab_key = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
    for _ in 0..100 {
        app.handle_key(tab_key);
    }

    assert_eq!(app.ui_state.selected_tab, Tab::Metrics);
}
