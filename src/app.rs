use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::config::Config;
use crate::event::Event;
use crate::types::{SystemMetrics, TrainingMetrics};
use crate::ui::Tab;

#[derive(Debug)]
pub struct TrainingState {
    pub latest: Option<TrainingMetrics>,
    pub loss_history: VecDeque<u64>,
    pub lr_history: VecDeque<u64>,
    pub step_history: VecDeque<u64>,
    pub throughput_history: VecDeque<u64>,
    pub total_steps: u64,
    pub start_time: Option<Instant>,
    pub input_active: bool,
    pub last_data_at: Option<Instant>,
}

#[derive(Debug)]
pub struct SystemState {
    pub latest: Option<SystemMetrics>,
    pub cpu_history: VecDeque<u64>,
    pub ram_history: VecDeque<u64>,
    pub gpu_history: VecDeque<u64>,
}

#[derive(Debug)]
pub struct UiState {
    pub selected_tab: Tab,
}

#[derive(Debug)]
pub struct App {
    pub running: bool,
    pub training: TrainingState,
    pub system: SystemState,
    pub ui_state: UiState,
    pub config: Config,
}

impl App {
    pub fn new(config: Config) -> Self {
        let capacity = config.history_size;
        Self {
            running: true,
            training: TrainingState {
                latest: None,
                loss_history: VecDeque::with_capacity(capacity),
                lr_history: VecDeque::with_capacity(capacity),
                step_history: VecDeque::with_capacity(capacity),
                throughput_history: VecDeque::with_capacity(capacity),
                total_steps: 0,
                start_time: None,
                input_active: false,
                last_data_at: None,
            },
            system: SystemState {
                latest: None,
                cpu_history: VecDeque::with_capacity(capacity),
                ram_history: VecDeque::with_capacity(capacity),
                gpu_history: VecDeque::with_capacity(capacity),
            },
            ui_state: UiState {
                selected_tab: Tab::Dashboard,
            },
            config,
        }
    }

    pub fn handle_event(&mut self, event: Event) {
        match event {
            Event::Key(key) => self.handle_key(key),
            Event::Tick => self.on_tick(),
            Event::Metrics(m) => self.push_metrics(m),
            Event::System(s) => self.push_system(s),
            Event::Resize(..) | Event::Mouse(..) => {}
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), KeyModifiers::NONE) => {
                self.running = false;
            }
            (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => {
                self.running = false;
            }
            (KeyCode::Tab, _) => {
                let current = self.ui_state.selected_tab as usize;
                self.ui_state.selected_tab =
                    Tab::from_repr((current + 1) % 3).unwrap_or(Tab::Dashboard);
            }
            (KeyCode::BackTab, _) => {
                let current = self.ui_state.selected_tab as usize;
                self.ui_state.selected_tab =
                    Tab::from_repr((current + 2) % 3).unwrap_or(Tab::Dashboard);
            }
            (KeyCode::Char('1'), KeyModifiers::NONE) => {
                self.ui_state.selected_tab = Tab::Dashboard;
            }
            (KeyCode::Char('2'), KeyModifiers::NONE) => {
                self.ui_state.selected_tab = Tab::Metrics;
            }
            (KeyCode::Char('3'), KeyModifiers::NONE) => {
                self.ui_state.selected_tab = Tab::System;
            }
            _ => {}
        }
    }

    pub fn on_tick(&mut self) {
        if let Some(last_data) = self.training.last_data_at {
            if last_data.elapsed() > Duration::from_secs(10) {
                self.training.input_active = false;
            }
        }
    }

    pub fn push_metrics(&mut self, m: TrainingMetrics) {
        let capacity = self.config.history_size;

        self.training.latest = Some(m.clone());

        if let Some(loss) = m.loss {
            let scaled = (loss * 1000.0) as u64;
            Self::push_bounded(&mut self.training.loss_history, scaled, capacity);
        }

        if let Some(lr) = m.learning_rate {
            let scaled = (lr * 1_000_000.0) as u64;
            Self::push_bounded(&mut self.training.lr_history, scaled, capacity);
        }

        if let Some(step) = m.step {
            Self::push_bounded(&mut self.training.step_history, step, capacity);
            self.training.total_steps = self.training.total_steps.max(step);
        }

        if let Some(throughput) = m.throughput {
            let scaled = throughput as u64;
            Self::push_bounded(&mut self.training.throughput_history, scaled, capacity);
        }

        self.training.input_active = true;
        self.training.last_data_at = Some(Instant::now());

        if self.training.start_time.is_none() {
            self.training.start_time = Some(Instant::now());
        }
    }

    pub fn push_system(&mut self, s: SystemMetrics) {
        let capacity = self.config.history_size;

        self.system.latest = Some(s.clone());

        let cpu_scaled = (s.cpu_usage_percent() * 100.0) as u64;
        Self::push_bounded(&mut self.system.cpu_history, cpu_scaled, capacity);

        let ram_scaled = (s.memory_usage_percent() * 100.0) as u64;
        Self::push_bounded(&mut self.system.ram_history, ram_scaled, capacity);

        if s.has_gpu() && !s.gpus.is_empty() {
            let gpu_scaled = (s.gpus[0].utilization * 100.0) as u64;
            Self::push_bounded(&mut self.system.gpu_history, gpu_scaled, capacity);
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.training
            .start_time
            .map(|start| start.elapsed())
            .unwrap_or(Duration::ZERO)
    }

    fn push_bounded(buf: &mut VecDeque<u64>, value: u64, capacity: usize) {
        buf.push_back(value);
        if buf.len() > capacity {
            buf.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::GpuMetrics;

    #[test]
    fn test_app_new_defaults() {
        let app = App::new(Config::default());
        assert!(app.running);
        assert!(app.training.loss_history.is_empty());
        assert!(app.training.lr_history.is_empty());
        assert!(app.training.step_history.is_empty());
        assert!(app.training.throughput_history.is_empty());
        assert_eq!(app.ui_state.selected_tab, Tab::Dashboard);
        assert!(app.training.latest.is_none());
        assert!(app.system.latest.is_none());
    }

    #[test]
    fn test_push_metrics_stores_latest() {
        let mut app = App::new(Config::default());
        let metrics = TrainingMetrics {
            loss: Some(0.5),
            learning_rate: Some(0.001),
            step: Some(100),
            throughput: Some(1000.0),
            tokens: Some(50000),
            timestamp: Instant::now(),
        };
        app.push_metrics(metrics);
        assert!(app.training.latest.is_some());
        assert_eq!(app.training.latest.as_ref().unwrap().loss, Some(0.5));
    }

    #[test]
    fn test_push_metrics_appends_to_history() {
        let mut app = App::new(Config::default());
        let metrics = TrainingMetrics {
            loss: Some(0.5),
            ..TrainingMetrics::default()
        };
        app.push_metrics(metrics);
        assert_eq!(app.training.loss_history.len(), 1);
        assert_eq!(app.training.loss_history[0], 500); // 0.5 * 1000
    }

    #[test]
    fn test_history_respects_capacity() {
        let config = Config {
            history_size: 300,
            ..Config::default()
        };
        let mut app = App::new(config);
        // Push 400 items
        for i in 0..400 {
            let metrics = TrainingMetrics {
                loss: Some(i as f64),
                ..TrainingMetrics::default()
            };
            app.push_metrics(metrics);
        }
        assert_eq!(app.training.loss_history.len(), 300);
    }

    #[test]
    fn test_handle_key_q_quits() {
        let mut app = App::new(Config::default());
        let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        app.handle_key(key);
        assert!(!app.running);
    }

    #[test]
    fn test_handle_key_ctrl_c_quits() {
        let mut app = App::new(Config::default());
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        app.handle_key(key);
        assert!(!app.running);
    }

    #[test]
    fn test_tab_cycle_forward() {
        let mut app = App::new(Config::default());
        assert_eq!(app.ui_state.selected_tab, Tab::Dashboard);

        let tab_key = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        app.handle_key(tab_key);
        assert_eq!(app.ui_state.selected_tab, Tab::Metrics);

        app.handle_key(tab_key);
        assert_eq!(app.ui_state.selected_tab, Tab::System);

        app.handle_key(tab_key);
        assert_eq!(app.ui_state.selected_tab, Tab::Dashboard); // wrap
    }

    #[test]
    fn test_tab_cycle_backward() {
        let mut app = App::new(Config::default());
        assert_eq!(app.ui_state.selected_tab, Tab::Dashboard);

        let backtab_key = KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT);
        app.handle_key(backtab_key);
        assert_eq!(app.ui_state.selected_tab, Tab::System); // wrap around
    }

    #[test]
    fn test_tab_direct_number() {
        let mut app = App::new(Config::default());

        let key1 = KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE);
        app.handle_key(key1);
        assert_eq!(app.ui_state.selected_tab, Tab::Dashboard);

        let key2 = KeyEvent::new(KeyCode::Char('2'), KeyModifiers::NONE);
        app.handle_key(key2);
        assert_eq!(app.ui_state.selected_tab, Tab::Metrics);

        let key3 = KeyEvent::new(KeyCode::Char('3'), KeyModifiers::NONE);
        app.handle_key(key3);
        assert_eq!(app.ui_state.selected_tab, Tab::System);
    }

    #[test]
    fn test_on_tick_staleness() {
        let mut app = App::new(Config::default());
        // Simulate old data
        app.training.last_data_at = Some(Instant::now() - Duration::from_secs(11));
        app.training.input_active = true;

        app.on_tick();
        assert!(!app.training.input_active);
    }

    #[test]
    fn test_push_metrics_sets_active() {
        let mut app = App::new(Config::default());
        let metrics = TrainingMetrics {
            loss: Some(0.5),
            ..TrainingMetrics::default()
        };
        app.push_metrics(metrics);
        assert!(app.training.input_active);
    }

    #[test]
    fn test_push_system_updates() {
        let mut app = App::new(Config::default());
        let system = SystemMetrics {
            cpu_usage: 50.0,
            memory_used: 4_000_000_000,
            memory_total: 16_000_000_000,
            gpus: vec![],
        };
        app.push_system(system);
        assert_eq!(app.system.cpu_history.len(), 1);
        assert_eq!(app.system.cpu_history[0], 5000); // 50.0 * 100
    }

    #[test]
    fn test_elapsed_zero_before_data() {
        let app = App::new(Config::default());
        assert_eq!(app.elapsed(), Duration::ZERO);
    }

    #[test]
    fn test_handle_event_dispatches() {
        let mut app = App::new(Config::default());

        // Test Event::Tick dispatch
        app.training.last_data_at = Some(Instant::now() - Duration::from_secs(11));
        app.training.input_active = true;
        app.handle_event(Event::Tick);
        assert!(!app.training.input_active);

        // Test Event::Metrics dispatch
        let metrics = TrainingMetrics {
            loss: Some(0.5),
            ..TrainingMetrics::default()
        };
        app.handle_event(Event::Metrics(metrics));
        assert!(app.training.latest.is_some());
    }

    #[test]
    fn test_push_metrics_all_fields() {
        let mut app = App::new(Config::default());
        let metrics = TrainingMetrics {
            loss: Some(0.5),
            learning_rate: Some(0.001),
            step: Some(100),
            throughput: Some(1000.0),
            tokens: Some(50000),
            timestamp: Instant::now(),
        };
        app.push_metrics(metrics);

        assert_eq!(app.training.loss_history.len(), 1);
        assert_eq!(app.training.loss_history[0], 500); // 0.5 * 1000

        assert_eq!(app.training.lr_history.len(), 1);
        assert_eq!(app.training.lr_history[0], 1000); // 0.001 * 1_000_000

        assert_eq!(app.training.step_history.len(), 1);
        assert_eq!(app.training.step_history[0], 100);

        assert_eq!(app.training.throughput_history.len(), 1);
        assert_eq!(app.training.throughput_history[0], 1000);

        assert_eq!(app.training.total_steps, 100);
    }

    #[test]
    fn test_push_system_with_gpu() {
        let mut app = App::new(Config::default());
        let system = SystemMetrics {
            cpu_usage: 50.0,
            memory_used: 8_000_000_000,
            memory_total: 16_000_000_000,
            gpus: vec![GpuMetrics {
                name: "RTX 4090".to_string(),
                utilization: 75.5,
                memory_used: 12_000_000_000,
                memory_total: 24_000_000_000,
                temperature: 65.0,
            }],
        };
        app.push_system(system);

        assert_eq!(app.system.cpu_history.len(), 1);
        assert_eq!(app.system.cpu_history[0], 5000); // 50.0 * 100

        assert_eq!(app.system.ram_history.len(), 1);
        assert_eq!(app.system.ram_history[0], 5000); // 50.0 * 100

        assert_eq!(app.system.gpu_history.len(), 1);
        assert_eq!(app.system.gpu_history[0], 7550); // 75.5 * 100
    }

    #[test]
    fn test_app_new() {
        let app = App::new(Config::default());
        assert!(app.running);
        assert_eq!(app.ui_state.selected_tab, Tab::Dashboard);
        assert!(app.training.latest.is_none());
    }
}
