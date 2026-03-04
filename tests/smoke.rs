use epoch::app::App;
use epoch::config::Config;
use epoch::types::{GpuMetrics, SystemMetrics};
use epoch::ui::Tab;

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
