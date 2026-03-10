use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config as MatcherConfig, Matcher};

use crate::collectors::process::ProcessCandidate;
use crate::collectors::training::parser_telemetry_snapshot;
use crate::config::{AlertEvalMode, AlertRuleConfig, AlertRuleKind, Config};
use crate::discovery::DiscoveredFile;
use crate::event::Event;
use crate::types::{SystemMetrics, TrainingMetrics};

impl std::fmt::Debug for crate::store::repository::RunStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunStore").finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub struct TrainingState {
    pub latest: Option<TrainingMetrics>,
    pub loss_history: VecDeque<u64>,
    pub lr_history: VecDeque<u64>,
    pub step_history: VecDeque<u64>,
    pub throughput_history: VecDeque<u64>,
    pub tokens_history: VecDeque<u64>,
    pub eval_loss_history: VecDeque<u64>,
    pub grad_norm_history: VecDeque<u64>,
    pub samples_per_second_history: VecDeque<u64>,
    pub steps_per_second_history: VecDeque<u64>,
    pub tokens_per_second_history: VecDeque<u64>,
    pub step_loss_points: VecDeque<(u64, u64)>,
    pub step_lr_points: VecDeque<(u64, u64)>,
    pub preferred_rate_metric: RateMetricPreference,
    pub relevance_profile: RelevanceProfile,
    pub perplexity_latest: Option<f64>,
    pub loss_spike_count: u64,
    pub nan_inf_count: u64,
    pub last_loss_spike_at: Option<Instant>,
    pub last_nan_inf_at: Option<Instant>,
    pub parser_success_count: u64,
    pub parser_skipped_count: u64,
    pub parser_error_count: u64,
    pub total_steps: u64,
    pub start_time: Option<Instant>,
    pub input_active: bool,
    pub last_data_at: Option<Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateMetricPreference {
    TokensPerSecond,
    SamplesPerSecond,
    StepsPerSecond,
    Throughput,
}

impl RateMetricPreference {
    pub fn metric_id(self) -> &'static str {
        match self {
            Self::TokensPerSecond => "tokens_per_second",
            Self::SamplesPerSecond => "samples_per_second",
            Self::StepsPerSecond => "steps_per_second",
            Self::Throughput => "throughput",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelevanceProfile {
    TrainOnly,
    EvalHeavy,
}

#[derive(Debug)]
pub struct SystemState {
    pub latest: Option<SystemMetrics>,
    pub cpu_history: VecDeque<u64>,
    pub ram_history: VecDeque<u64>,
    pub gpu_history: VecDeque<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct RunExplorerUiState {
    pub records: Vec<crate::store::types::RunRecord>,
    pub selected_idx: usize,
    pub search_query: String,
    pub search_active: bool,
    pub status_filter: Option<crate::store::types::RunStatus>,
}

#[derive(Debug)]
pub struct UiState {
    pub primary_view: PrimaryView,
    pub focused_box: u8,
    pub mode: AppMode,
    pub selected_file: Option<PathBuf>,
    pub scanning_frame: usize,
    pub graph_viewports: [ViewportState; 4],
    pub system_viewport: ViewportState,
    pub explorer: RunExplorerUiState,
    pub selected_process_idx: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimaryView {
    Home,
    LiveRun,
    RunExplorer,
    SystemProcesses,
}

impl PrimaryView {
    pub const COUNT: usize = 4;

    pub fn label(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::LiveRun => "Live Run",
            Self::RunExplorer => "Run Explorer",
            Self::SystemProcesses => "System/Processes",
        }
    }

    pub fn index(self) -> usize {
        match self {
            Self::Home => 0,
            Self::LiveRun => 1,
            Self::RunExplorer => 2,
            Self::SystemProcesses => 3,
        }
    }

    pub fn from_index(index: usize) -> Self {
        match index {
            0 => Self::Home,
            1 => Self::LiveRun,
            2 => Self::RunExplorer,
            3 => Self::SystemProcesses,
            _ => Self::Home,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertLevel {
    Warning,
    Critical,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlertRecord {
    pub rule_id: String,
    pub level: AlertLevel,
    pub value: f64,
    pub message: String,
    pub tick: u64,
}

#[derive(Debug, Default)]
pub struct AlertsState {
    pub tick: u64,
    pub active: Vec<AlertRecord>,
    pub resolved: Vec<AlertRecord>,
    cooldown_until: HashMap<String, u64>,
}

#[derive(Debug, Default)]
pub struct RunComparisonState {
    pub baseline_loss_history: VecDeque<u64>,
    pub baseline_lr_history: VecDeque<u64>,
    pub baseline_step_history: VecDeque<u64>,
    pub baseline_step_loss_points: VecDeque<(u64, u64)>,
    pub baseline_step_lr_points: VecDeque<(u64, u64)>,
    pub baseline_step_loss_map: HashMap<u64, u64>,
    pub baseline_step_lr_map: HashMap<u64, u64>,
    pub snapshot_mode: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewportState {
    pub follow_latest: bool,
    pub offset_samples: usize,
    pub zoom_level: u8,
}

impl Default for ViewportState {
    fn default() -> Self {
        Self {
            follow_latest: true,
            offset_samples: 0,
            zoom_level: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataHealthState {
    Live,
    Stale,
    NoData,
}

impl DataHealthState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Live => "Live",
            Self::Stale => "Stale",
            Self::NoData => "No data",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Scanning,
    FilePicker(FilePickerState),
    Monitoring,
    Settings(Box<SettingsState>),
    Help(Box<HelpState>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SettingsState {
    pub selected_row: usize,
    pub draft: Config,
    pub original: Config,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HelpState {
    pub entries: Vec<(String, String)>,
    pub theme: String,
    pub custom_theme: Option<crate::config::CustomTheme>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FilePickerState {
    pub files: Vec<DiscoveredFile>,
    pub query: String,
    pub filtered_indices: Vec<usize>,
    pub selected_index: usize,
    pub input_mode: FilePickerInputMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilePickerInputMode {
    Insert,
    Normal,
}

#[derive(Clone)]
struct FuzzyCandidate {
    index: usize,
    text: String,
}

impl AsRef<str> for FuzzyCandidate {
    fn as_ref(&self) -> &str {
        &self.text
    }
}

impl FilePickerState {
    pub fn new(files: Vec<DiscoveredFile>) -> Self {
        Self::new_for_keymap(files, "default")
    }

    pub fn new_for_keymap(files: Vec<DiscoveredFile>, keymap_profile: &str) -> Self {
        Self {
            filtered_indices: (0..files.len()).collect(),
            files,
            query: String::new(),
            selected_index: 0,
            input_mode: if keymap_profile == "vim" {
                FilePickerInputMode::Normal
            } else {
                FilePickerInputMode::Insert
            },
        }
    }

    pub fn refresh_filter(&mut self) {
        if self.query.is_empty() {
            self.filtered_indices = (0..self.files.len()).collect();
        } else {
            let candidates = self
                .files
                .iter()
                .enumerate()
                .map(|(index, file)| FuzzyCandidate {
                    index,
                    text: file.path.to_string_lossy().to_string(),
                })
                .collect::<Vec<_>>();

            let pattern = Pattern::parse(&self.query, CaseMatching::Smart, Normalization::Smart);
            let mut matcher = Matcher::new(MatcherConfig::DEFAULT.match_paths());
            self.filtered_indices = pattern
                .match_list(candidates, &mut matcher)
                .into_iter()
                .map(|(candidate, _)| candidate.index)
                .collect();
        }

        if self.filtered_indices.is_empty() {
            self.selected_index = 0;
        } else if self.selected_index >= self.filtered_indices.len() {
            self.selected_index = self.filtered_indices.len() - 1;
        }
    }

    fn move_down(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }

        self.selected_index = (self.selected_index + 1) % self.filtered_indices.len();
    }

    fn move_up(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }

        if self.selected_index == 0 {
            self.selected_index = self.filtered_indices.len() - 1;
        } else {
            self.selected_index -= 1;
        }
    }
}

impl SettingsState {
    const ROW_PARSER: usize = 0;
    const ROW_THEME: usize = 1;
    const ROW_GRAPH_MODE: usize = 2;
    const ROW_ADAPTIVE_LAYOUT: usize = 3;
    const ROW_PINNED_RATE_METRIC: usize = 4;
    const ROW_KEYMAP_PROFILE: usize = 5;
    const ROW_PROFILE_TARGET: usize = 6;
    const ROW_COUNT: usize = 7;

    fn from_config(config: &Config) -> Self {
        Self {
            selected_row: 0,
            draft: config.clone(),
            original: config.clone(),
        }
    }

    fn move_up(&mut self) {
        if self.selected_row == 0 {
            self.selected_row = Self::ROW_COUNT - 1;
        } else {
            self.selected_row -= 1;
        }
    }

    fn move_down(&mut self) {
        self.selected_row = (self.selected_row + 1) % Self::ROW_COUNT;
    }

    fn cycle_current(&mut self, delta: i8) {
        match self.selected_row {
            Self::ROW_PARSER => {
                self.draft.parser = cycle_option(
                    &self.draft.parser,
                    &["auto", "jsonl", "csv", "regex", "tensorboard"],
                    delta,
                );
            }
            Self::ROW_THEME => {
                self.draft.theme = cycle_option_normalized(
                    &self.draft.theme,
                    crate::ui::theme::SELECTABLE_THEMES,
                    delta,
                );
            }
            Self::ROW_GRAPH_MODE => {
                self.draft.graph_mode =
                    cycle_option(&self.draft.graph_mode, &["sparkline", "line"], delta);
            }
            Self::ROW_ADAPTIVE_LAYOUT => {
                self.draft.adaptive_layout = !self.draft.adaptive_layout;
            }
            Self::ROW_PINNED_RATE_METRIC => {
                let current = pinned_rate_preset_id(&self.draft.pinned_metrics);
                let cycle_current = if current == "mixed" { "none" } else { current };
                let next = cycle_option(
                    cycle_current,
                    &["none", "tokens", "samples", "steps", "all"],
                    delta,
                );
                let mut next_pinned = self
                    .draft
                    .pinned_metrics
                    .iter()
                    .filter(|metric| !is_rate_metric(metric))
                    .cloned()
                    .collect::<Vec<_>>();

                next_pinned.extend(
                    pinned_rate_values_for_preset(&next)
                        .iter()
                        .map(|v| (*v).to_string()),
                );
                self.draft.pinned_metrics = next_pinned;
            }
            Self::ROW_KEYMAP_PROFILE => {
                self.draft.keymap_profile =
                    cycle_option(&self.draft.keymap_profile, &["default", "vim"], delta);
            }
            Self::ROW_PROFILE_TARGET => {
                self.draft.profile_target =
                    cycle_option(&self.draft.profile_target, &["global", "project"], delta);
            }
            _ => {}
        }
    }
}

impl HelpState {
    fn from_config(config: &Config) -> Self {
        Self {
            entries: keymap_entries(&config.keymap_profile),
            theme: config.theme.clone(),
            custom_theme: config.custom_theme.clone(),
        }
    }
}

fn keymap_entries(profile: &str) -> Vec<(String, String)> {
    let mut entries = vec![
        ("q / Ctrl+C".to_string(), "Quit".to_string()),
        ("Tab / Shift+Tab".to_string(), "Switch view".to_string()),
        ("1-4".to_string(), "Focus graph".to_string()),
        ("Space".to_string(), "Toggle live/pause".to_string()),
        (
            "Left/Right".to_string(),
            "Pan active graph history".to_string(),
        ),
        ("- / =".to_string(), "Zoom active graph".to_string()),
        ("g".to_string(), "Reset all viewports to live".to_string()),
        ("s".to_string(), "Open settings".to_string()),
        ("?".to_string(), "Toggle help overlay".to_string()),
    ];

    entries.push(("Home: o".to_string(), "Open file picker".to_string()));
    entries.push(("Home: a".to_string(), "Attach to process".to_string()));
    entries.push(("Home: e".to_string(), "Explore all runs".to_string()));
    entries.push(("Home: s".to_string(), "Scan directory".to_string()));
    entries.push(("Home: r".to_string(), "Refresh run list".to_string()));
    entries.push(("Explorer: j/k".to_string(), "Move cursor".to_string()));
    entries.push(("Explorer: /".to_string(), "Search runs".to_string()));
    entries.push(("Explorer: f".to_string(), "Cycle status filter".to_string()));
    entries.push(("Explorer: Enter".to_string(), "Open active run".to_string()));
    entries.push(("Explorer: r".to_string(), "Refresh".to_string()));
    entries.push(("Processes: j/k".to_string(), "Move cursor".to_string()));
    entries.push(("Processes: a".to_string(), "Attach to process".to_string()));
    entries.push(("Processes: r".to_string(), "Refresh".to_string()));

    if profile == "vim" {
        entries.push(("j/k".to_string(), "Switch view (vim)".to_string()));
        entries.push(("h/l".to_string(), "Pan history (vim)".to_string()));
        entries.push((
            "Picker (vim): i".to_string(),
            "Enter insert mode for query".to_string(),
        ));
        entries.push((
            "Picker (vim): Esc".to_string(),
            "Leave insert mode (normal Esc quits)".to_string(),
        ));
        entries.push((
            "Picker (vim): j/k".to_string(),
            "Move selection in normal mode".to_string(),
        ));
    }

    entries
}

fn cycle_option(current: &str, options: &[&str], delta: i8) -> String {
    let current_index = options.iter().position(|v| *v == current).unwrap_or(0) as isize;
    let len = options.len() as isize;
    let next = (current_index + delta as isize).rem_euclid(len) as usize;
    options[next].to_string()
}

fn cycle_option_normalized(current: &str, options: &[&str], delta: i8) -> String {
    let normalized = current.trim().to_ascii_lowercase();
    let current_index = options
        .iter()
        .position(|v| v.eq_ignore_ascii_case(&normalized))
        .unwrap_or(0) as isize;
    let len = options.len() as isize;
    let next = (current_index + delta as isize).rem_euclid(len) as usize;
    options[next].to_string()
}

fn is_rate_metric(metric_id: &str) -> bool {
    matches!(
        metric_id,
        "tokens_per_second" | "samples_per_second" | "steps_per_second"
    )
}

fn pinned_rate_values_for_preset(preset: &str) -> &'static [&'static str] {
    match preset {
        "tokens" => &["tokens_per_second"],
        "samples" => &["samples_per_second"],
        "steps" => &["steps_per_second"],
        "all" => &[
            "tokens_per_second",
            "samples_per_second",
            "steps_per_second",
        ],
        _ => &[],
    }
}

fn pinned_rate_preset_id(pinned_metrics: &[String]) -> &'static str {
    let tokens = pinned_metrics.iter().any(|m| m == "tokens_per_second");
    let samples = pinned_metrics.iter().any(|m| m == "samples_per_second");
    let steps = pinned_metrics.iter().any(|m| m == "steps_per_second");

    match (tokens, samples, steps) {
        (false, false, false) => "none",
        (true, false, false) => "tokens",
        (false, true, false) => "samples",
        (false, false, true) => "steps",
        (true, true, true) => "all",
        _ => "mixed",
    }
}

#[derive(Debug)]
pub struct App {
    pub running: bool,
    pub training: TrainingState,
    pub system: SystemState,
    pub discovered_processes: Vec<ProcessCandidate>,
    pub ui_state: UiState,
    pub alerts: AlertsState,
    pub run_comparison: RunComparisonState,
    pub config: Config,
    pub run_store: Option<crate::store::repository::RunStore>,
    pub project_root: Option<std::path::PathBuf>,
    pub recent_runs: Vec<crate::store::types::RunRecord>,
    pub discovered_files: Vec<crate::discovery::DiscoveredFile>,
}

impl App {
    const VIEWPORT_PAN_STEP: usize = 10;
    const VIEWPORT_MAX_ZOOM_LEVEL: u8 = 6;

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
                tokens_history: VecDeque::with_capacity(capacity),
                eval_loss_history: VecDeque::with_capacity(capacity),
                grad_norm_history: VecDeque::with_capacity(capacity),
                samples_per_second_history: VecDeque::with_capacity(capacity),
                steps_per_second_history: VecDeque::with_capacity(capacity),
                tokens_per_second_history: VecDeque::with_capacity(capacity),
                step_loss_points: VecDeque::with_capacity(capacity),
                step_lr_points: VecDeque::with_capacity(capacity),
                preferred_rate_metric: RateMetricPreference::Throughput,
                relevance_profile: RelevanceProfile::TrainOnly,
                perplexity_latest: None,
                loss_spike_count: 0,
                nan_inf_count: 0,
                last_loss_spike_at: None,
                last_nan_inf_at: None,
                parser_success_count: 0,
                parser_skipped_count: 0,
                parser_error_count: 0,
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
            discovered_processes: Vec::new(),
            ui_state: UiState {
                primary_view: PrimaryView::LiveRun,
                focused_box: 1,
                mode: AppMode::Monitoring,
                selected_file: None,
                scanning_frame: 0,
                graph_viewports: [ViewportState::default(); 4],
                system_viewport: ViewportState::default(),
                explorer: RunExplorerUiState::default(),
                selected_process_idx: 0,
            },
            alerts: AlertsState::default(),
            run_comparison: RunComparisonState::default(),
            config,
            run_store: None,
            project_root: None,
            recent_runs: Vec::new(),
            discovered_files: Vec::new(),
        }
    }

    pub fn should_show_metric_panel(&self, metric_id: &str, present: bool) -> bool {
        if !self.config.adaptive_layout {
            return true;
        }
        if self.config.pinned_metrics.iter().any(|p| p == metric_id) {
            return true;
        }
        if self
            .config
            .hidden_metrics
            .iter()
            .any(|hidden| hidden == metric_id)
        {
            return false;
        }
        present
    }

    pub fn preferred_rate_metric_id(&self) -> &'static str {
        self.training.preferred_rate_metric.metric_id()
    }

    pub fn set_store(&mut self, store: crate::store::repository::RunStore) {
        self.run_store = Some(store);
        self.load_recent_runs();
        self.refresh_explorer_records();
    }

    pub fn load_recent_runs(&mut self) {
        if let Some(store) = &self.run_store {
            self.recent_runs = store.list_recent_runs(5).unwrap_or_default();
        }
    }

    pub fn refresh_explorer_records(&mut self) {
        if let Some(store) = &self.run_store {
            let status_str = self
                .ui_state
                .explorer
                .status_filter
                .as_ref()
                .map(|s| s.as_str());
            let query = if self.ui_state.explorer.search_query.is_empty() {
                None
            } else {
                Some(self.ui_state.explorer.search_query.as_str())
            };
            self.ui_state.explorer.records =
                store.list_runs(status_str, query, 100).unwrap_or_default();
            let max_idx = self.ui_state.explorer.records.len().saturating_sub(1);
            self.ui_state.explorer.selected_idx = self.ui_state.explorer.selected_idx.min(max_idx);
        }
    }

    pub fn set_discovered_files(&mut self, files: Vec<crate::discovery::DiscoveredFile>) {
        self.discovered_files = files;
    }

    pub fn set_discovered_processes(
        &mut self,
        processes: Vec<crate::collectors::process::ProcessCandidate>,
    ) {
        self.discovered_processes = processes;
        let max_idx = self.discovered_processes.len().saturating_sub(1);
        self.ui_state.selected_process_idx = self.ui_state.selected_process_idx.min(max_idx);
    }

    fn attach_process_and_switch(
        &mut self,
        candidate: &crate::collectors::process::ProcessCandidate,
    ) {
        if let Some(store) = &self.run_store {
            let project_root_str = self
                .project_root
                .as_ref()
                .map(|p| p.to_string_lossy().to_string());
            let _ = crate::home::service::attach_to_discovered_process(
                store,
                candidate,
                project_root_str.as_deref(),
            );
        }
        self.ui_state.primary_view = PrimaryView::LiveRun;
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
        let is_help_key = matches!(key.code, KeyCode::Char('?'));

        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), KeyModifiers::NONE) => {
                self.running = false;
                return;
            }
            (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => {
                self.running = false;
                return;
            }
            _ => {}
        }

        if let AppMode::FilePicker(ref mut picker) = self.ui_state.mode {
            if self.config.keymap_profile == "vim" {
                match picker.input_mode {
                    FilePickerInputMode::Insert => match (key.code, key.modifiers) {
                        (KeyCode::Esc, _) => {
                            picker.input_mode = FilePickerInputMode::Normal;
                        }
                        (KeyCode::Backspace, _) => {
                            picker.query.pop();
                            picker.refresh_filter();
                        }
                        (KeyCode::Enter, _) => {
                            if let Some(index) =
                                picker.filtered_indices.get(picker.selected_index).copied()
                            {
                                self.ui_state.selected_file =
                                    Some(picker.files[index].path.clone());
                                self.ui_state.mode = AppMode::Monitoring;
                            } else if !picker.query.trim().is_empty() {
                                self.ui_state.selected_file =
                                    Some(PathBuf::from(picker.query.clone()));
                                self.ui_state.mode = AppMode::Monitoring;
                            }
                        }
                        (KeyCode::Char(c), KeyModifiers::NONE) => {
                            picker.query.push(c);
                            picker.refresh_filter();
                        }
                        _ => {}
                    },
                    FilePickerInputMode::Normal => match (key.code, key.modifiers) {
                        (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::NONE) => {
                            picker.move_up();
                        }
                        (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::NONE) => {
                            picker.move_down();
                        }
                        (KeyCode::Char('i'), KeyModifiers::NONE) => {
                            picker.input_mode = FilePickerInputMode::Insert;
                        }
                        (KeyCode::Enter, _) => {
                            if let Some(index) =
                                picker.filtered_indices.get(picker.selected_index).copied()
                            {
                                self.ui_state.selected_file =
                                    Some(picker.files[index].path.clone());
                                self.ui_state.mode = AppMode::Monitoring;
                            } else if !picker.query.trim().is_empty() {
                                self.ui_state.selected_file =
                                    Some(PathBuf::from(picker.query.clone()));
                                self.ui_state.mode = AppMode::Monitoring;
                            }
                        }
                        (KeyCode::Backspace, _) => {
                            picker.query.pop();
                            picker.refresh_filter();
                        }
                        (KeyCode::Esc, _) => {
                            self.running = false;
                        }
                        _ => {}
                    },
                }
                return;
            }

            match (key.code, key.modifiers) {
                (KeyCode::Up, _) => {
                    picker.move_up();
                }
                (KeyCode::Down, _) => {
                    picker.move_down();
                }
                (KeyCode::Backspace, _) => {
                    picker.query.pop();
                    picker.refresh_filter();
                }
                (KeyCode::Enter, _) => {
                    if let Some(index) = picker.filtered_indices.get(picker.selected_index).copied()
                    {
                        self.ui_state.selected_file = Some(picker.files[index].path.clone());
                        self.ui_state.mode = AppMode::Monitoring;
                    } else if !picker.query.trim().is_empty() {
                        self.ui_state.selected_file = Some(PathBuf::from(picker.query.clone()));
                        self.ui_state.mode = AppMode::Monitoring;
                    }
                }
                (KeyCode::Esc, _) => {
                    self.running = false;
                }
                (KeyCode::Char(c), KeyModifiers::NONE) => {
                    picker.query.push(c);
                    picker.refresh_filter();
                }
                _ => {}
            }
            return;
        }

        if let AppMode::Help(_) = &self.ui_state.mode {
            if matches!(key.code, KeyCode::Esc) || is_help_key {
                self.ui_state.mode = AppMode::Monitoring;
            }
            return;
        }

        enum SettingsAction {
            Apply,
            Save,
            Cancel,
        }

        let settings_action = if let AppMode::Settings(state) = &mut self.ui_state.mode {
            let mut action = None;
            match (key.code, key.modifiers) {
                (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::NONE) => state.move_up(),
                (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::NONE) => state.move_down(),
                (KeyCode::Left, _) | (KeyCode::Char('h'), KeyModifiers::NONE) => {
                    state.cycle_current(-1)
                }
                (KeyCode::Right, _) | (KeyCode::Char('l'), KeyModifiers::NONE) => {
                    state.cycle_current(1)
                }
                (KeyCode::Char('a'), KeyModifiers::NONE) => action = Some(SettingsAction::Apply),
                (KeyCode::Char('w'), KeyModifiers::NONE) | (KeyCode::Enter, _) => {
                    action = Some(SettingsAction::Save)
                }
                (KeyCode::Esc, _) => action = Some(SettingsAction::Cancel),
                _ => {}
            }
            action
        } else {
            None
        };

        if matches!(self.ui_state.mode, AppMode::Settings(_)) {
            if let Some(action) = settings_action {
                match action {
                    SettingsAction::Apply => {
                        if let AppMode::Settings(state) = &self.ui_state.mode {
                            self.config = state.draft.clone();
                            self.recompute_metric_relevance();
                        }
                    }
                    SettingsAction::Save => {
                        if let AppMode::Settings(state) = &self.ui_state.mode {
                            self.config = state.draft.clone();
                            self.recompute_metric_relevance();
                            if self.config.profile_target == "project" {
                                let project_root = self
                                    .config
                                    .log_file
                                    .as_ref()
                                    .and_then(|p| p.parent().map(std::path::Path::to_path_buf))
                                    .or_else(|| std::env::current_dir().ok());
                                if let Some(root) = project_root
                                    && let Err(err) = self.config.save_project(&root)
                                {
                                    tracing::debug!("failed to save project settings: {err}");
                                }
                            } else if let Err(err) = self.config.save_global() {
                                tracing::debug!("failed to save global settings: {err}");
                            }
                        }
                        self.ui_state.mode = AppMode::Monitoring;
                    }
                    SettingsAction::Cancel => {
                        if let AppMode::Settings(state) = &self.ui_state.mode {
                            self.config = state.original.clone();
                            self.recompute_metric_relevance();
                        }
                        self.ui_state.mode = AppMode::Monitoring;
                    }
                }
            }
            return;
        }

        // Global commands (work in any view)
        match (key.code, key.modifiers) {
            (KeyCode::Char('s'), KeyModifiers::NONE) => {
                self.ui_state.mode =
                    AppMode::Settings(Box::new(SettingsState::from_config(&self.config)));
            }
            _ if is_help_key => {
                self.ui_state.mode = AppMode::Help(Box::new(HelpState::from_config(&self.config)));
            }
            (KeyCode::Tab, _) => {
                let next = (self.ui_state.primary_view.index() + 1) % PrimaryView::COUNT;
                self.ui_state.primary_view = PrimaryView::from_index(next);
            }
            (KeyCode::BackTab, _) => {
                let current = self.ui_state.primary_view.index();
                let next = if current == 0 {
                    PrimaryView::COUNT - 1
                } else {
                    current - 1
                };
                self.ui_state.primary_view = PrimaryView::from_index(next);
            }
            (KeyCode::Char(' '), KeyModifiers::NONE) => {
                let follow_latest = !self.ui_state.graph_viewports[0].follow_latest;
                for vp in &mut self.ui_state.graph_viewports {
                    vp.follow_latest = follow_latest;
                    if follow_latest {
                        vp.offset_samples = 0;
                    }
                }
                self.ui_state.system_viewport.follow_latest = follow_latest;
                if follow_latest {
                    self.ui_state.system_viewport.offset_samples = 0;
                }
            }
            (KeyCode::Char('g'), KeyModifiers::NONE) => {
                for vp in &mut self.ui_state.graph_viewports {
                    vp.follow_latest = true;
                    vp.offset_samples = 0;
                    vp.zoom_level = 0;
                }
                self.ui_state.system_viewport.follow_latest = true;
                self.ui_state.system_viewport.offset_samples = 0;
                self.ui_state.system_viewport.zoom_level = 0;
            }
            _ => {
                // Tab-specific commands
                match self.ui_state.primary_view {
                    PrimaryView::LiveRun => self.handle_key_live_run(key),
                    PrimaryView::Home => self.handle_key_home(key),
                    PrimaryView::RunExplorer => self.handle_key_run_explorer(key),
                    PrimaryView::SystemProcesses => self.handle_key_system_processes(key),
                }
            }
        }
    }

    fn handle_key_live_run(&mut self, key: KeyEvent) {
        let is_vim = self.config.keymap_profile == "vim";
        match (key.code, key.modifiers) {
            // Box focus (tab-level)
            (KeyCode::Char(c @ '1'..='4'), KeyModifiers::NONE) => {
                self.ui_state.focused_box = c as u8 - b'0';
            }
            // Zoom (box-level)
            (KeyCode::Char('-'), KeyModifiers::NONE) => {
                let vp =
                    &mut self.ui_state.graph_viewports[(self.ui_state.focused_box - 1) as usize];
                vp.zoom_level = vp.zoom_level.saturating_sub(1);
                if vp.zoom_level == 0 {
                    vp.offset_samples = 0;
                }
            }
            (KeyCode::Char('='), KeyModifiers::NONE) => {
                let vp =
                    &mut self.ui_state.graph_viewports[(self.ui_state.focused_box - 1) as usize];
                vp.zoom_level = vp
                    .zoom_level
                    .saturating_add(1)
                    .min(Self::VIEWPORT_MAX_ZOOM_LEVEL);
            }
            // Pan (box-level: arrows always, h/l in vim)
            (KeyCode::Left, KeyModifiers::NONE) | (KeyCode::Char('h'), KeyModifiers::NONE)
                if is_vim || matches!(key.code, KeyCode::Left) =>
            {
                let vp =
                    &mut self.ui_state.graph_viewports[(self.ui_state.focused_box - 1) as usize];
                if !vp.follow_latest && vp.zoom_level > 0 {
                    vp.offset_samples = vp.offset_samples.saturating_add(Self::VIEWPORT_PAN_STEP);
                }
            }
            (KeyCode::Right, KeyModifiers::NONE) | (KeyCode::Char('l'), KeyModifiers::NONE)
                if is_vim || matches!(key.code, KeyCode::Right) =>
            {
                let vp =
                    &mut self.ui_state.graph_viewports[(self.ui_state.focused_box - 1) as usize];
                if !vp.follow_latest && vp.zoom_level > 0 {
                    vp.offset_samples = vp.offset_samples.saturating_sub(Self::VIEWPORT_PAN_STEP);
                }
            }
            // Focus cycling (box-level: j/k in vim, up/down always)
            (KeyCode::Down, KeyModifiers::NONE) | (KeyCode::Char('j'), KeyModifiers::NONE)
                if is_vim || matches!(key.code, KeyCode::Down) =>
            {
                let next = if self.ui_state.focused_box >= 4 {
                    1
                } else {
                    self.ui_state.focused_box + 1
                };
                self.ui_state.focused_box = next;
            }
            (KeyCode::Up, KeyModifiers::NONE) | (KeyCode::Char('k'), KeyModifiers::NONE)
                if is_vim || matches!(key.code, KeyCode::Up) =>
            {
                let next = if self.ui_state.focused_box <= 1 {
                    4
                } else {
                    self.ui_state.focused_box - 1
                };
                self.ui_state.focused_box = next;
            }
            _ => {}
        }
    }

    fn handle_key_home(&mut self, key: KeyEvent) {
        match (key.code, key.modifiers) {
            (KeyCode::Char('o'), KeyModifiers::NONE) => {
                self.ui_state.mode = AppMode::Scanning;
            }
            (KeyCode::Char('e'), KeyModifiers::NONE) => {
                self.ui_state.primary_view = PrimaryView::RunExplorer;
                self.refresh_explorer_records();
            }
            (KeyCode::Char('a'), KeyModifiers::NONE) => {
                if let Some(candidate) = self.discovered_processes.first().cloned() {
                    self.attach_process_and_switch(&candidate);
                }
            }
            (KeyCode::Char('s'), KeyModifiers::NONE) => {
                self.ui_state.mode = AppMode::Scanning;
            }
            (KeyCode::Char('r'), KeyModifiers::NONE) => {
                self.load_recent_runs();
            }
            _ => {}
        }
    }

    fn handle_key_run_explorer(&mut self, key: KeyEvent) {
        if self.ui_state.explorer.search_active {
            match key.code {
                KeyCode::Esc => {
                    self.ui_state.explorer.search_active = false;
                }
                KeyCode::Enter => {
                    self.ui_state.explorer.search_active = false;
                    self.refresh_explorer_records();
                }
                KeyCode::Backspace => {
                    self.ui_state.explorer.search_query.pop();
                    self.refresh_explorer_records();
                }
                KeyCode::Char(c) => {
                    self.ui_state.explorer.search_query.push(c);
                    self.refresh_explorer_records();
                }
                _ => {}
            }
            return;
        }

        match (key.code, key.modifiers) {
            (KeyCode::Char('j'), _) | (KeyCode::Down, _) => {
                let max = self.ui_state.explorer.records.len().saturating_sub(1);
                self.ui_state.explorer.selected_idx =
                    (self.ui_state.explorer.selected_idx + 1).min(max);
            }
            (KeyCode::Char('k'), _) | (KeyCode::Up, _) => {
                self.ui_state.explorer.selected_idx =
                    self.ui_state.explorer.selected_idx.saturating_sub(1);
            }
            (KeyCode::Char('/'), KeyModifiers::NONE) => {
                self.ui_state.explorer.search_active = true;
            }
            (KeyCode::Char('f'), KeyModifiers::NONE) => {
                use crate::store::types::RunStatus;
                self.ui_state.explorer.status_filter = match &self.ui_state.explorer.status_filter {
                    None => Some(RunStatus::Active),
                    Some(RunStatus::Active) => Some(RunStatus::Completed),
                    Some(RunStatus::Completed) => Some(RunStatus::Failed),
                    Some(RunStatus::Failed) => None,
                };
                self.refresh_explorer_records();
            }
            (KeyCode::Char('r'), KeyModifiers::NONE) => {
                self.refresh_explorer_records();
            }
            (KeyCode::Enter, _) => {
                if let Some(record) = self
                    .ui_state
                    .explorer
                    .records
                    .get(self.ui_state.explorer.selected_idx)
                {
                    if record.status == crate::store::types::RunStatus::Active {
                        self.ui_state.primary_view = PrimaryView::LiveRun;
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_key_system_processes(&mut self, key: KeyEvent) {
        let is_vim = self.config.keymap_profile == "vim";
        match (key.code, key.modifiers) {
            (KeyCode::Char('j'), _) | (KeyCode::Down, _)
                if is_vim || matches!(key.code, KeyCode::Down) =>
            {
                let max = self.discovered_processes.len().saturating_sub(1);
                self.ui_state.selected_process_idx =
                    (self.ui_state.selected_process_idx + 1).min(max);
            }
            (KeyCode::Char('k'), _) | (KeyCode::Up, _)
                if is_vim || matches!(key.code, KeyCode::Up) =>
            {
                self.ui_state.selected_process_idx =
                    self.ui_state.selected_process_idx.saturating_sub(1);
            }
            (KeyCode::Char('a'), KeyModifiers::NONE) => {
                if let Some(candidate) = self
                    .discovered_processes
                    .get(self.ui_state.selected_process_idx)
                    .cloned()
                {
                    self.attach_process_and_switch(&candidate);
                }
            }
            (KeyCode::Char('r'), KeyModifiers::NONE) => {
                let max = self.discovered_processes.len().saturating_sub(1);
                self.ui_state.selected_process_idx = self.ui_state.selected_process_idx.min(max);
            }
            _ => {}
        }
    }

    pub fn on_tick(&mut self) {
        self.alerts.tick = self.alerts.tick.saturating_add(1);
        let parser_telemetry = parser_telemetry_snapshot();
        self.training.parser_success_count = parser_telemetry.success_count;
        self.training.parser_skipped_count = parser_telemetry.skipped_count;
        self.training.parser_error_count = parser_telemetry.error_count;

        if matches!(self.ui_state.mode, AppMode::Scanning) {
            self.ui_state.scanning_frame = (self.ui_state.scanning_frame + 1) % 4;
        }

        if let Some(last_data) = self.training.last_data_at {
            if last_data.elapsed() > Duration::from_secs(self.config.stale_after_secs) {
                self.training.input_active = false;
            }
        }

        self.evaluate_alerts();
    }

    pub fn training_data_health_state(&self) -> DataHealthState {
        if self.training.input_active {
            DataHealthState::Live
        } else if self.training.last_data_at.is_some() {
            DataHealthState::Stale
        } else {
            DataHealthState::NoData
        }
    }

    pub fn graph_viewport_series(
        &self,
        graph_index: usize,
        history: &VecDeque<u64>,
        width: usize,
    ) -> Vec<u64> {
        let viewport = self
            .ui_state
            .graph_viewports
            .get(graph_index)
            .copied()
            .unwrap_or_default();
        Self::viewport_series(history, viewport, width)
    }

    pub fn system_viewport_series(&self, history: &VecDeque<u64>, width: usize) -> Vec<u64> {
        Self::viewport_series(history, self.ui_state.system_viewport, width)
    }

    pub fn push_metrics(&mut self, m: TrainingMetrics) {
        let capacity = self.config.history_size;

        let invalid_count = Self::count_non_finite_metrics(&m);
        self.training.nan_inf_count += invalid_count;
        if invalid_count > 0 {
            self.training.last_nan_inf_at = Some(Instant::now());
        }

        if let Some(loss) = m.loss
            && loss.is_finite()
        {
            self.training.perplexity_latest = Some(Self::safe_perplexity(loss));

            if Self::is_loss_spike(&self.training.loss_history, loss, 1000.0, 20, 1.2) {
                self.training.loss_spike_count += 1;
                self.training.last_loss_spike_at = Some(Instant::now());
            }
        }

        if let Some(loss) = m.loss {
            let scaled = Self::scale_to_u64(loss, 1000.0);
            Self::push_bounded(&mut self.training.loss_history, scaled, capacity);
            if let Some(step) = m.step {
                Self::push_bounded_pair(
                    &mut self.training.step_loss_points,
                    (step, scaled),
                    capacity,
                );
            }
        }

        if let Some(lr) = m.learning_rate {
            let scaled = Self::scale_to_u64(lr, 1_000_000.0);
            Self::push_bounded(&mut self.training.lr_history, scaled, capacity);
            if let Some(step) = m.step {
                Self::push_bounded_pair(
                    &mut self.training.step_lr_points,
                    (step, scaled),
                    capacity,
                );
            }
        }

        if let Some(step) = m.step {
            Self::push_bounded(&mut self.training.step_history, step, capacity);
            self.training.total_steps = self.training.total_steps.max(step);
        }

        let throughput_value = m
            .tokens_per_second
            .or(m.samples_per_second)
            .or(m.throughput);
        if let Some(throughput) = throughput_value {
            let scaled = Self::scale_to_u64(throughput, 1.0);
            Self::push_bounded(&mut self.training.throughput_history, scaled, capacity);
        }

        if let Some(tokens) = m.tokens {
            Self::push_bounded(&mut self.training.tokens_history, tokens, capacity);
        }

        if let Some(eval_loss) = m.eval_loss {
            let scaled = Self::scale_to_u64(eval_loss, 1000.0);
            Self::push_bounded(&mut self.training.eval_loss_history, scaled, capacity);
        }

        if let Some(grad_norm) = m.grad_norm {
            let scaled = Self::scale_to_u64(grad_norm, 1000.0);
            Self::push_bounded(&mut self.training.grad_norm_history, scaled, capacity);
        }

        if let Some(samples_per_second) = m.samples_per_second {
            let scaled = Self::scale_to_u64(samples_per_second, 1.0);
            Self::push_bounded(
                &mut self.training.samples_per_second_history,
                scaled,
                capacity,
            );
        }

        if let Some(steps_per_second) = m.steps_per_second {
            let scaled = Self::scale_to_u64(steps_per_second, 1000.0);
            Self::push_bounded(
                &mut self.training.steps_per_second_history,
                scaled,
                capacity,
            );
        }

        if let Some(tokens_per_second) = m.tokens_per_second {
            let scaled = Self::scale_to_u64(tokens_per_second, 1.0);
            Self::push_bounded(
                &mut self.training.tokens_per_second_history,
                scaled,
                capacity,
            );
        }

        self.training.input_active = true;
        self.training.last_data_at = Some(Instant::now());

        if self.training.start_time.is_none() {
            self.training.start_time = Some(Instant::now());
        }

        self.training.latest = Some(m);
        self.recompute_metric_relevance();

        self.evaluate_alerts();
    }

    fn recompute_metric_relevance(&mut self) {
        let latest = self.training.latest.as_ref();

        let preferred = if latest.is_some_and(|m| m.tokens_per_second.is_some()) {
            RateMetricPreference::TokensPerSecond
        } else if latest.is_some_and(|m| m.samples_per_second.is_some()) {
            RateMetricPreference::SamplesPerSecond
        } else if latest.is_some_and(|m| m.steps_per_second.is_some()) {
            RateMetricPreference::StepsPerSecond
        } else if !self.training.tokens_per_second_history.is_empty() {
            RateMetricPreference::TokensPerSecond
        } else if !self.training.samples_per_second_history.is_empty() {
            RateMetricPreference::SamplesPerSecond
        } else if !self.training.steps_per_second_history.is_empty() {
            RateMetricPreference::StepsPerSecond
        } else {
            RateMetricPreference::Throughput
        };
        self.training.preferred_rate_metric = preferred;

        self.training.relevance_profile = if latest.is_some_and(|m| m.eval_loss.is_some())
            || !self.training.eval_loss_history.is_empty()
        {
            RelevanceProfile::EvalHeavy
        } else {
            RelevanceProfile::TrainOnly
        };
    }

    fn evaluate_alerts(&mut self) {
        if self.config.alert_rules.is_empty() {
            self.alerts.active.clear();
            self.alerts.cooldown_until.clear();
            return;
        }

        let enabled_rule_ids = self
            .config
            .alert_rules
            .iter()
            .enumerate()
            .filter(|(_, rule)| rule.enabled)
            .map(|(idx, rule)| Self::rule_id(rule, idx))
            .collect::<Vec<_>>();
        self.alerts
            .active
            .retain(|record| enabled_rule_ids.iter().any(|id| id == &record.rule_id));
        self.alerts
            .cooldown_until
            .retain(|rule_id, _| enabled_rule_ids.iter().any(|id| id == rule_id));

        for (idx, rule) in self.config.alert_rules.iter().enumerate() {
            if !rule.enabled {
                continue;
            }

            let rule_id = Self::rule_id(rule, idx);
            let Some(value) = self.alert_value(rule) else {
                continue;
            };

            let current_level = self
                .alerts
                .active
                .iter()
                .find(|record| record.rule_id == rule_id)
                .map(|record| record.level);
            let desired_level = self.level_for_rule(rule, value);
            let next_level = self.apply_hysteresis(rule, current_level, desired_level, value);

            match (current_level, next_level) {
                (None, Some(level)) => {
                    let cooldown = self
                        .alerts
                        .cooldown_until
                        .get(&rule_id)
                        .copied()
                        .unwrap_or(0);
                    if self.alerts.tick < cooldown {
                        continue;
                    }
                    self.alerts.active.push(AlertRecord {
                        rule_id: rule_id.clone(),
                        level,
                        value,
                        message: Self::alert_message(&rule_id, level, value),
                        tick: self.alerts.tick,
                    });
                }
                (Some(_), None) => {
                    if let Some(idx) = self
                        .alerts
                        .active
                        .iter()
                        .position(|record| record.rule_id == rule_id)
                    {
                        let mut resolved = self.alerts.active.remove(idx);
                        resolved.message = format!("resolved at {:.3}", value);
                        resolved.tick = self.alerts.tick;
                        self.alerts.resolved.push(resolved);
                        if self.alerts.resolved.len() > 20 {
                            let drain = self.alerts.resolved.len() - 20;
                            self.alerts.resolved.drain(0..drain);
                        }
                        self.alerts
                            .cooldown_until
                            .insert(rule_id.clone(), self.alerts.tick.saturating_add(30));
                    }
                }
                (Some(current), Some(next)) if current != next => {
                    if let Some(record) = self
                        .alerts
                        .active
                        .iter_mut()
                        .find(|record| record.rule_id == rule_id)
                    {
                        record.level = next;
                        record.value = value;
                        record.tick = self.alerts.tick;
                        record.message = Self::alert_message(&rule_id, next, value);
                    }
                }
                (Some(_), Some(_)) => {
                    if let Some(record) = self
                        .alerts
                        .active
                        .iter_mut()
                        .find(|record| record.rule_id == rule_id)
                    {
                        record.value = value;
                        record.tick = self.alerts.tick;
                    }
                }
                (None, None) => {}
            }
        }
    }

    fn alert_value(&self, rule: &AlertRuleConfig) -> Option<f64> {
        match rule.kind {
            AlertRuleKind::ThroughputDrop => {
                let (history, current, scale) = match self.training.preferred_rate_metric {
                    RateMetricPreference::TokensPerSecond => (
                        &self.training.tokens_per_second_history,
                        self.training
                            .latest
                            .as_ref()
                            .and_then(|m| m.tokens_per_second),
                        1.0,
                    ),
                    RateMetricPreference::SamplesPerSecond => (
                        &self.training.samples_per_second_history,
                        self.training
                            .latest
                            .as_ref()
                            .and_then(|m| m.samples_per_second),
                        1.0,
                    ),
                    RateMetricPreference::StepsPerSecond => (
                        &self.training.steps_per_second_history,
                        self.training
                            .latest
                            .as_ref()
                            .and_then(|m| m.steps_per_second),
                        1000.0,
                    ),
                    RateMetricPreference::Throughput => (
                        &self.training.throughput_history,
                        self.training.latest.as_ref().and_then(|m| m.throughput),
                        1.0,
                    ),
                };
                self.apply_eval_mode(&rule.mode, history, scale, current)
            }
            AlertRuleKind::MemoryPressure => self.apply_eval_mode(
                &rule.mode,
                &self.system.ram_history,
                100.0,
                self.system
                    .latest
                    .as_ref()
                    .map(|s| s.memory_usage_percent()),
            ),
            AlertRuleKind::LossTrendWorsening => {
                let rolling_window = match rule.mode {
                    AlertEvalMode::RollingMean { window } => window.max(1),
                    AlertEvalMode::Current => 1,
                };
                self.loss_trend_slope(rolling_window, 20)
            }
        }
    }

    fn rule_id(rule: &AlertRuleConfig, index: usize) -> String {
        rule.id
            .clone()
            .unwrap_or_else(|| format!("{}#{index}", rule.kind.as_id()))
    }

    fn apply_eval_mode(
        &self,
        mode: &AlertEvalMode,
        history: &VecDeque<u64>,
        scale: f64,
        current: Option<f64>,
    ) -> Option<f64> {
        match mode {
            AlertEvalMode::Current => current,
            AlertEvalMode::RollingMean { window } => {
                if *window == 0 {
                    return current;
                }
                let count = history.len().min(*window);
                if count == 0 {
                    return current;
                }
                let sum: f64 = history
                    .iter()
                    .rev()
                    .take(count)
                    .map(|v| *v as f64 / scale)
                    .sum();
                Some(sum / count as f64)
            }
        }
    }

    fn loss_trend_slope(&self, rolling_window: usize, slope_points: usize) -> Option<f64> {
        if rolling_window == 0 || slope_points < 2 {
            return None;
        }
        let losses = self
            .training
            .loss_history
            .iter()
            .map(|v| *v as f64 / 1000.0)
            .collect::<Vec<_>>();
        if losses.len() < rolling_window + slope_points {
            return None;
        }

        let mut smoothed = Vec::with_capacity(losses.len().saturating_sub(rolling_window) + 1);
        for idx in 0..=losses.len() - rolling_window {
            let window = &losses[idx..idx + rolling_window];
            smoothed.push(window.iter().sum::<f64>() / rolling_window as f64);
        }
        if smoothed.len() < slope_points {
            return None;
        }

        let tail = &smoothed[smoothed.len() - slope_points..];
        let first = tail.first().copied()?;
        let last = tail.last().copied()?;
        Some((last - first) / (slope_points - 1) as f64)
    }

    fn level_for_rule(&self, rule: &AlertRuleConfig, value: f64) -> Option<AlertLevel> {
        match rule.kind {
            AlertRuleKind::ThroughputDrop => {
                if value <= rule.critical {
                    Some(AlertLevel::Critical)
                } else if value <= rule.warning {
                    Some(AlertLevel::Warning)
                } else {
                    None
                }
            }
            AlertRuleKind::MemoryPressure | AlertRuleKind::LossTrendWorsening => {
                if value >= rule.critical {
                    Some(AlertLevel::Critical)
                } else if value >= rule.warning {
                    Some(AlertLevel::Warning)
                } else {
                    None
                }
            }
        }
    }

    fn apply_hysteresis(
        &self,
        rule: &AlertRuleConfig,
        current: Option<AlertLevel>,
        desired: Option<AlertLevel>,
        value: f64,
    ) -> Option<AlertLevel> {
        let band = 0.05;
        match (rule.kind, current, desired) {
            (
                AlertRuleKind::ThroughputDrop,
                Some(AlertLevel::Critical),
                Some(AlertLevel::Warning),
            ) if value <= rule.critical * (1.0 + band) => Some(AlertLevel::Critical),
            (AlertRuleKind::ThroughputDrop, Some(AlertLevel::Warning), None)
                if value <= rule.warning * (1.0 + band) =>
            {
                Some(AlertLevel::Warning)
            }
            (
                AlertRuleKind::MemoryPressure | AlertRuleKind::LossTrendWorsening,
                Some(AlertLevel::Critical),
                Some(AlertLevel::Warning),
            ) if value >= rule.critical * (1.0 - band) => Some(AlertLevel::Critical),
            (
                AlertRuleKind::MemoryPressure | AlertRuleKind::LossTrendWorsening,
                Some(AlertLevel::Warning),
                None,
            ) if value >= rule.warning * (1.0 - band) => Some(AlertLevel::Warning),
            (_, _, next) => next,
        }
    }

    fn alert_message(rule_id: &str, level: AlertLevel, value: f64) -> String {
        let level_text = match level {
            AlertLevel::Warning => "warning",
            AlertLevel::Critical => "critical",
        };
        format!("{rule_id}: {level_text} at {value:.3}")
    }

    pub fn set_run_comparison_snapshot(&mut self, baseline: Vec<TrainingMetrics>) {
        let capacity = self.config.history_size;
        let mut by_step: HashMap<u64, TrainingMetrics> = HashMap::new();
        let mut ordered_without_step = Vec::new();

        for metrics in baseline {
            if let Some(step) = metrics.step {
                by_step.insert(step, metrics);
            } else {
                ordered_without_step.push(metrics);
            }
        }

        self.run_comparison.baseline_loss_history.clear();
        self.run_comparison.baseline_lr_history.clear();
        self.run_comparison.baseline_step_history.clear();
        self.run_comparison.baseline_step_loss_points.clear();
        self.run_comparison.baseline_step_lr_points.clear();
        self.run_comparison.baseline_step_loss_map.clear();
        self.run_comparison.baseline_step_lr_map.clear();

        if !by_step.is_empty() {
            let mut steps = by_step.keys().copied().collect::<Vec<_>>();
            steps.sort_unstable();
            for step in steps {
                if let Some(metrics) = by_step.get(&step) {
                    Self::push_bounded(
                        &mut self.run_comparison.baseline_step_history,
                        step,
                        capacity,
                    );
                    if let Some(loss) = metrics.loss {
                        let scaled = Self::scale_to_u64(loss, 1000.0);
                        Self::push_bounded(
                            &mut self.run_comparison.baseline_loss_history,
                            scaled,
                            capacity,
                        );
                        Self::push_bounded_pair(
                            &mut self.run_comparison.baseline_step_loss_points,
                            (step, scaled),
                            capacity,
                        );
                        self.run_comparison
                            .baseline_step_loss_map
                            .insert(step, scaled);
                    }
                    if let Some(lr) = metrics.learning_rate {
                        let scaled = Self::scale_to_u64(lr, 1_000_000.0);
                        Self::push_bounded(
                            &mut self.run_comparison.baseline_lr_history,
                            scaled,
                            capacity,
                        );
                        Self::push_bounded_pair(
                            &mut self.run_comparison.baseline_step_lr_points,
                            (step, scaled),
                            capacity,
                        );
                        self.run_comparison
                            .baseline_step_lr_map
                            .insert(step, scaled);
                    }
                }
            }
        } else {
            for (idx, metrics) in ordered_without_step.into_iter().enumerate() {
                Self::push_bounded(
                    &mut self.run_comparison.baseline_step_history,
                    idx as u64,
                    capacity,
                );
                if let Some(loss) = metrics.loss {
                    let scaled = Self::scale_to_u64(loss, 1000.0);
                    Self::push_bounded(
                        &mut self.run_comparison.baseline_loss_history,
                        scaled,
                        capacity,
                    );
                }
                if let Some(lr) = metrics.learning_rate {
                    let scaled = Self::scale_to_u64(lr, 1_000_000.0);
                    Self::push_bounded(
                        &mut self.run_comparison.baseline_lr_history,
                        scaled,
                        capacity,
                    );
                }
            }
        }

        self.run_comparison.snapshot_mode = true;
    }

    pub fn run_comparison_snapshot_mode(&self) -> bool {
        self.run_comparison.snapshot_mode
    }

    pub fn run_compare_alignment_by_step(&self) -> Vec<(u64, Option<u64>, Option<u64>)> {
        if self.training.step_loss_points.is_empty()
            || self.run_comparison.baseline_step_loss_map.is_empty()
        {
            return Vec::new();
        }

        let current = self
            .training
            .step_loss_points
            .iter()
            .copied()
            .collect::<HashMap<_, _>>();
        let baseline = &self.run_comparison.baseline_step_loss_map;

        let mut all_steps = current
            .keys()
            .chain(baseline.keys())
            .copied()
            .collect::<Vec<_>>();
        all_steps.sort_unstable();
        all_steps.dedup();

        all_steps
            .into_iter()
            .map(|step| {
                (
                    step,
                    current.get(&step).copied(),
                    baseline.get(&step).copied(),
                )
            })
            .collect()
    }

    pub fn run_compare_fallback_alignment(&self) -> Vec<(Option<u64>, Option<u64>)> {
        let current = self
            .training
            .loss_history
            .iter()
            .copied()
            .collect::<Vec<_>>();
        let baseline = self
            .run_comparison
            .baseline_loss_history
            .iter()
            .copied()
            .collect::<Vec<_>>();
        let max_len = current.len().max(baseline.len());
        if max_len == 0 {
            return Vec::new();
        }

        (0..max_len)
            .map(|idx| {
                let current_idx = if current.is_empty() {
                    None
                } else {
                    Some(idx * current.len().saturating_sub(1) / max_len.saturating_sub(1).max(1))
                };
                let baseline_idx = if baseline.is_empty() {
                    None
                } else {
                    Some(idx * baseline.len().saturating_sub(1) / max_len.saturating_sub(1).max(1))
                };
                (
                    current_idx.and_then(|i| current.get(i).copied()),
                    baseline_idx.and_then(|i| baseline.get(i).copied()),
                )
            })
            .collect()
    }

    pub fn run_compare_latest_loss_delta(&self) -> Option<f64> {
        let latest = self.training.latest.as_ref()?;
        let step = latest.step?;
        let current_loss = latest.loss?;
        if let Some(baseline_scaled) = self.run_comparison.baseline_step_loss_map.get(&step) {
            return Some(current_loss - (*baseline_scaled as f64 / 1000.0));
        }
        None
    }

    pub fn run_compare_latest_lr_delta(&self) -> Option<f64> {
        let latest = self.training.latest.as_ref()?;
        let step = latest.step?;
        let current_lr = latest.learning_rate?;
        let baseline = self
            .run_comparison
            .baseline_step_lr_map
            .get(&step)
            .copied()? as f64
            / 1_000_000.0;
        Some(current_lr - baseline)
    }

    pub fn push_system(&mut self, s: SystemMetrics) {
        let capacity = self.config.history_size;

        self.system.latest = Some(s.clone());

        let cpu_scaled = Self::scale_to_u64(s.cpu_usage_percent(), 100.0);
        Self::push_bounded(&mut self.system.cpu_history, cpu_scaled, capacity);

        let ram_scaled = Self::scale_to_u64(s.memory_usage_percent(), 100.0);
        Self::push_bounded(&mut self.system.ram_history, ram_scaled, capacity);

        if s.has_gpu() && !s.gpus.is_empty() {
            let gpu_scaled = Self::scale_to_u64(s.gpus[0].utilization, 100.0);
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

    fn push_bounded_pair(buf: &mut VecDeque<(u64, u64)>, value: (u64, u64), capacity: usize) {
        buf.push_back(value);
        if buf.len() > capacity {
            buf.pop_front();
        }
    }

    fn scale_to_u64(value: f64, factor: f64) -> u64 {
        if !value.is_finite() || value <= 0.0 || !factor.is_finite() || factor <= 0.0 {
            return 0;
        }

        let clamped = value.clamp(0.0, f64::MAX / factor);
        (clamped * factor) as u64
    }

    fn safe_perplexity(loss: f64) -> f64 {
        loss.clamp(0.0, 50.0).exp()
    }

    fn count_non_finite_metrics(m: &TrainingMetrics) -> u64 {
        [
            m.loss,
            m.learning_rate,
            m.throughput,
            m.eval_loss,
            m.grad_norm,
            m.samples_per_second,
            m.steps_per_second,
            m.tokens_per_second,
        ]
        .iter()
        .filter(|v| v.is_some_and(|n| !n.is_finite()))
        .count() as u64
    }

    fn is_loss_spike(
        history: &VecDeque<u64>,
        current_loss: f64,
        scale: f64,
        window: usize,
        threshold_multiplier: f64,
    ) -> bool {
        let baseline_values: Vec<f64> = history
            .iter()
            .rev()
            .take(window)
            .copied()
            .map(|v| v as f64 / scale)
            .collect();

        if baseline_values.len() < 5 {
            return false;
        }

        let baseline_mean = baseline_values.iter().sum::<f64>() / baseline_values.len() as f64;
        current_loss > baseline_mean * threshold_multiplier
    }

    fn viewport_series(history: &VecDeque<u64>, viewport: ViewportState, width: usize) -> Vec<u64> {
        if history.is_empty() {
            return Vec::new();
        }

        let width = width.max(1);
        let history_len = history.len();
        let zoom_level = viewport.zoom_level.min(Self::VIEWPORT_MAX_ZOOM_LEVEL);

        let window = if zoom_level == 0 {
            history_len
        } else {
            let zoom_divisor = 1usize << zoom_level;
            history_len.div_ceil(zoom_divisor).max(width)
        };
        let max_start = history_len.saturating_sub(window);
        let offset = if viewport.follow_latest || zoom_level == 0 {
            0
        } else {
            viewport.offset_samples.min(max_start)
        };
        let start = max_start.saturating_sub(offset);
        let end = (start + window).min(history_len);

        let sampled: Vec<u64> = history
            .iter()
            .skip(start)
            .take(end - start)
            .copied()
            .collect();
        if sampled.len() <= width {
            return sampled;
        }

        Self::downsample_to_width(&sampled, width)
    }

    fn downsample_to_width(sampled: &[u64], width: usize) -> Vec<u64> {
        if sampled.len() <= width {
            return sampled.to_vec();
        }
        if width <= 1 {
            return vec![sampled[sampled.len() - 1]];
        }
        if width == 2 {
            return vec![sampled[0], sampled[sampled.len() - 1]];
        }

        let mut out = Vec::with_capacity(width);
        out.push(sampled[0]);

        let interior_slots = width - 2;
        let interior_len = sampled.len() - 2;
        let bucket_count = interior_slots.div_ceil(2).max(1);

        for bucket in 0..bucket_count {
            let start = 1 + (bucket * interior_len / bucket_count);
            let end = 1 + ((bucket + 1) * interior_len / bucket_count);
            if start >= end {
                continue;
            }

            let mut min_idx = start;
            let mut max_idx = start;
            for idx in (start + 1)..end {
                if sampled[idx] < sampled[min_idx] {
                    min_idx = idx;
                }
                if sampled[idx] > sampled[max_idx] {
                    max_idx = idx;
                }
            }

            if min_idx <= max_idx {
                out.push(sampled[min_idx]);
                if max_idx != min_idx {
                    out.push(sampled[max_idx]);
                }
            } else {
                out.push(sampled[max_idx]);
                out.push(sampled[min_idx]);
            }
        }

        out.truncate(width - 1);
        while out.len() < width - 1 {
            let idx = out.len() * (sampled.len() - 2) / (width - 2) + 1;
            out.push(sampled[idx.min(sampled.len() - 2)]);
        }

        out.push(sampled[sampled.len() - 1]);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::FileFormat;
    use crate::types::GpuMetrics;
    use std::fs;
    use std::path::PathBuf;
    use std::time::SystemTime;
    use std::time::UNIX_EPOCH;

    fn sample_discovered_files() -> Vec<DiscoveredFile> {
        vec![
            DiscoveredFile {
                path: PathBuf::from("/tmp/a.jsonl"),
                format: FileFormat::Jsonl,
                modified: UNIX_EPOCH,
            },
            DiscoveredFile {
                path: PathBuf::from("/tmp/b.csv"),
                format: FileFormat::Csv,
                modified: UNIX_EPOCH,
            },
        ]
    }

    #[test]
    fn test_app_new_defaults() {
        let app = App::new(Config::default());
        assert!(app.running);
        assert!(app.training.loss_history.is_empty());
        assert!(app.training.lr_history.is_empty());
        assert!(app.training.step_history.is_empty());
        assert!(app.training.throughput_history.is_empty());
        assert!(app.training.tokens_history.is_empty());
        assert!(app.training.eval_loss_history.is_empty());
        assert!(app.training.grad_norm_history.is_empty());
        assert!(app.training.samples_per_second_history.is_empty());
        assert!(app.training.steps_per_second_history.is_empty());
        assert!(app.training.tokens_per_second_history.is_empty());
        assert!(app.training.perplexity_latest.is_none());
        assert_eq!(app.training.loss_spike_count, 0);
        assert_eq!(app.training.nan_inf_count, 0);
        assert!(app.training.last_loss_spike_at.is_none());
        assert!(app.training.last_nan_inf_at.is_none());
        assert_eq!(app.ui_state.primary_view, PrimaryView::LiveRun);
        assert_eq!(app.ui_state.focused_box, 1);
        assert_eq!(app.ui_state.mode, AppMode::Monitoring);
        assert!(app.ui_state.selected_file.is_none());
        assert_eq!(app.ui_state.scanning_frame, 0);
        assert!(app.ui_state.graph_viewports[0].follow_latest);
        assert_eq!(app.ui_state.graph_viewports[0].offset_samples, 0);
        assert_eq!(app.ui_state.graph_viewports[0].zoom_level, 0);
        assert!(app.ui_state.system_viewport.follow_latest);
        assert_eq!(app.ui_state.system_viewport.offset_samples, 0);
        assert_eq!(app.ui_state.system_viewport.zoom_level, 0);
        assert!(app.training.latest.is_none());
        assert!(app.system.latest.is_none());
    }

    #[test]
    fn test_scanning_mode_advances_spinner_on_tick() {
        let mut app = App::new(Config::default());
        app.ui_state.mode = AppMode::Scanning;
        assert_eq!(app.ui_state.scanning_frame, 0);

        app.on_tick();
        assert_eq!(app.ui_state.scanning_frame, 1);

        app.on_tick();
        app.on_tick();
        app.on_tick();
        assert_eq!(app.ui_state.scanning_frame, 0);
    }

    #[test]
    fn test_app_default_mode_is_monitoring() {
        let app = App::new(Config::default());
        assert_eq!(app.ui_state.mode, AppMode::Monitoring);
    }

    #[test]
    fn test_file_picker_state_creation() {
        let files = sample_discovered_files();
        let state = FilePickerState::new(files.clone());

        assert_eq!(state.files, files);
        assert_eq!(state.query, "");
        assert_eq!(state.filtered_indices, vec![0, 1]);
        assert_eq!(state.selected_index, 0);
        assert_eq!(state.input_mode, FilePickerInputMode::Insert);
    }

    #[test]
    fn test_file_picker_vim_starts_in_normal_mode_when_requested() {
        let state = FilePickerState::new_for_keymap(sample_discovered_files(), "vim");
        assert_eq!(state.input_mode, FilePickerInputMode::Normal);
    }

    #[test]
    fn test_file_picker_navigation_down() {
        let mut app = App::new(Config::default());
        app.ui_state.mode = AppMode::FilePicker(FilePickerState::new(sample_discovered_files()));

        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));

        assert!(
            matches!(app.ui_state.mode, AppMode::FilePicker(ref state) if state.selected_index == 1)
        );
    }

    #[test]
    fn test_file_picker_navigation_up() {
        let mut app = App::new(Config::default());
        app.ui_state.mode = AppMode::FilePicker(FilePickerState::new(sample_discovered_files()));

        app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));

        assert!(
            matches!(app.ui_state.mode, AppMode::FilePicker(ref state) if state.selected_index == 1)
        );
    }

    #[test]
    fn test_file_picker_query_input() {
        let mut app = App::new(Config::default());
        app.ui_state.mode = AppMode::FilePicker(FilePickerState::new(sample_discovered_files()));

        app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));

        assert!(matches!(app.ui_state.mode, AppMode::FilePicker(ref state) if state.query == "a"));
    }

    #[test]
    fn test_file_picker_query_fuzzy_match() {
        let mut state = FilePickerState::new(sample_discovered_files());
        state.query = "ajsn".to_string();
        state.refresh_filter();

        assert!(!state.filtered_indices.is_empty());
        let first = state.filtered_indices[0];
        assert_eq!(state.files[first].path, PathBuf::from("/tmp/a.jsonl"));
    }

    #[test]
    fn test_file_picker_backspace() {
        let mut app = App::new(Config::default());
        app.ui_state.mode = AppMode::FilePicker(FilePickerState {
            query: "ab".to_string(),
            ..FilePickerState::new(sample_discovered_files())
        });

        app.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));

        assert!(matches!(app.ui_state.mode, AppMode::FilePicker(ref state) if state.query == "a"));
    }

    #[test]
    fn test_file_picker_enter_selects() {
        let mut app = App::new(Config::default());
        app.ui_state.mode = AppMode::FilePicker(FilePickerState::new(sample_discovered_files()));

        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.ui_state.mode, AppMode::Monitoring);
        assert_eq!(
            app.ui_state.selected_file,
            Some(PathBuf::from("/tmp/a.jsonl"))
        );
    }

    #[test]
    fn test_file_picker_enter_uses_query_path_when_no_matches() {
        let mut app = App::new(Config::default());
        app.ui_state.mode = AppMode::FilePicker(FilePickerState {
            files: vec![],
            query: "/tmp/manual.jsonl".to_string(),
            filtered_indices: vec![],
            selected_index: 0,
            input_mode: FilePickerInputMode::Insert,
        });

        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.ui_state.mode, AppMode::Monitoring);
        assert_eq!(
            app.ui_state.selected_file,
            Some(PathBuf::from("/tmp/manual.jsonl"))
        );
    }

    #[test]
    fn test_file_picker_escape_quits() {
        let mut app = App::new(Config::default());
        app.ui_state.mode = AppMode::FilePicker(FilePickerState::new(sample_discovered_files()));

        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert!(!app.running);
    }

    #[test]
    fn test_tab_key_cycles_primary_view() {
        let mut app = App::new(Config::default());
        app.ui_state.mode = AppMode::Monitoring;
        app.ui_state.primary_view = PrimaryView::LiveRun;

        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.ui_state.primary_view, PrimaryView::RunExplorer);
    }

    #[test]
    fn test_settings_mode_open_navigate_close() {
        let mut app = App::new(Config::default());
        app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));

        assert!(matches!(app.ui_state.mode, AppMode::Settings(_)));

        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));

        let (selected_row, theme_after_cycle) = match &app.ui_state.mode {
            AppMode::Settings(state) => (state.selected_row, state.draft.theme.clone()),
            _ => panic!("expected settings mode"),
        };
        assert_eq!(selected_row, 1);
        assert_eq!(theme_after_cycle, "catppuccin");

        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(app.ui_state.mode, AppMode::Monitoring);
        assert_eq!(app.config.theme, "classic");
    }

    #[test]
    fn test_settings_theme_cycle_normalizes_case_and_whitespace() {
        let config = Config {
            theme: " Nord ".to_string(),
            ..Config::default()
        };
        let mut app = App::new(config);

        app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));

        let theme_after_cycle = match &app.ui_state.mode {
            AppMode::Settings(state) => state.draft.theme.clone(),
            _ => panic!("expected settings mode"),
        };

        assert_eq!(theme_after_cycle, "gruvbox");
    }

    #[test]
    fn test_settings_pinned_rate_metric_cycle_preserves_non_rate_pins() {
        let config = Config {
            pinned_metrics: vec!["eval_loss".to_string()],
            ..Config::default()
        };
        let mut app = App::new(config);

        app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
        for _ in 0..4 {
            app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        }
        app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));

        let pinned = match &app.ui_state.mode {
            AppMode::Settings(state) => state.draft.pinned_metrics.clone(),
            _ => panic!("expected settings mode"),
        };

        assert!(pinned.iter().any(|m| m == "eval_loss"));
        assert!(pinned.iter().any(|m| m == "tokens_per_second"));
    }

    #[test]
    fn test_settings_pinned_rate_mixed_starts_from_none_for_cycle() {
        let config = Config {
            pinned_metrics: vec![
                "tokens_per_second".to_string(),
                "samples_per_second".to_string(),
            ],
            ..Config::default()
        };
        let mut app = App::new(config);

        app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
        for _ in 0..4 {
            app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        }
        app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));

        let pinned = match &app.ui_state.mode {
            AppMode::Settings(state) => state.draft.pinned_metrics.clone(),
            _ => panic!("expected settings mode"),
        };

        assert_eq!(pinned_rate_preset_id(&pinned), "tokens");
    }

    #[test]
    fn test_settings_apply_and_save_routes_to_correct_profile_target() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("epoch-settings-save-{unique}"));
        fs::create_dir_all(&root).expect("test root should be created");

        let config = Config {
            log_file: Some(root.join("train.log")),
            ..Config::default()
        };
        let mut app = App::new(config);

        app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
        if let AppMode::Settings(state) = &mut app.ui_state.mode {
            state.draft.theme = "github".to_string();
            state.draft.profile_target = "project".to_string();
        } else {
            panic!("expected settings mode");
        }

        app.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
        assert_eq!(app.ui_state.mode, AppMode::Monitoring);
        assert_eq!(app.config.theme, "github");

        let saved_path = root.join(".epoch").join("config.toml");
        let saved = fs::read_to_string(&saved_path).expect("project config should be saved");
        assert!(saved.contains("theme = \"github\""));
        assert!(saved.contains("profile_target = \"project\""));

        fs::remove_dir_all(&root).expect("test root should be removed");
    }

    #[test]
    fn test_handle_key_question_toggles_help_mode() {
        let mut app = App::new(Config::default());
        app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT));
        assert!(matches!(app.ui_state.mode, AppMode::Help(_)));

        app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT));
        assert_eq!(app.ui_state.mode, AppMode::Monitoring);
    }

    #[test]
    fn test_handle_key_question_toggles_help_mode_without_shift_modifier() {
        let mut app = App::new(Config::default());
        app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
        assert!(matches!(app.ui_state.mode, AppMode::Help(_)));

        app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
        assert_eq!(app.ui_state.mode, AppMode::Monitoring);
    }

    #[test]
    fn test_help_overlay_close_keys_do_not_quit_app() {
        let mut app = App::new(Config::default());
        app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT));
        assert!(matches!(app.ui_state.mode, AppMode::Help(_)));

        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(app.running);
        assert_eq!(app.ui_state.mode, AppMode::Monitoring);
    }

    #[test]
    fn test_vim_profile_hjkl_maps_to_navigation_in_monitoring() {
        let mut app = App::new(Config {
            keymap_profile: "vim".to_string(),
            ..Config::default()
        });

        assert_eq!(app.ui_state.primary_view, PrimaryView::LiveRun);
        assert_eq!(app.ui_state.focused_box, 1);

        let idx = (app.ui_state.focused_box - 1) as usize;
        app.ui_state.graph_viewports[idx].follow_latest = false;
        app.ui_state.graph_viewports[idx].zoom_level = 1;

        app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        assert_eq!(app.ui_state.focused_box, 2);

        app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
        assert_eq!(app.ui_state.focused_box, 1);

        app.handle_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE));
        assert_eq!(
            app.ui_state.graph_viewports[idx].offset_samples,
            App::VIEWPORT_PAN_STEP
        );

        app.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE));
        assert_eq!(app.ui_state.graph_viewports[idx].offset_samples, 0);
    }

    #[test]
    fn test_vim_profile_does_not_break_filepicker_text_input() {
        let mut app = App::new(Config {
            keymap_profile: "vim".to_string(),
            ..Config::default()
        });
        app.ui_state.mode = AppMode::FilePicker(FilePickerState::new(sample_discovered_files()));

        app.handle_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE));

        let query = match &app.ui_state.mode {
            AppMode::FilePicker(state) => state.query.clone(),
            _ => panic!("expected file picker mode"),
        };
        assert_eq!(query, "hl");
    }

    #[test]
    fn test_vim_filepicker_j_types_in_insert_then_navigates_in_normal() {
        let mut app = App::new(Config {
            keymap_profile: "vim".to_string(),
            ..Config::default()
        });
        app.ui_state.mode = AppMode::FilePicker(FilePickerState::new_for_keymap(
            sample_discovered_files(),
            "vim",
        ));

        app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        let (query, selected_index, mode) = match &app.ui_state.mode {
            AppMode::FilePicker(state) => {
                (state.query.clone(), state.selected_index, state.input_mode)
            }
            _ => panic!("expected file picker mode"),
        };
        assert_eq!(query, "");
        assert_eq!(selected_index, 1);
        assert_eq!(mode, FilePickerInputMode::Normal);

        app.handle_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        let (query, selected_index, mode) = match &app.ui_state.mode {
            AppMode::FilePicker(state) => {
                (state.query.clone(), state.selected_index, state.input_mode)
            }
            _ => panic!("expected file picker mode"),
        };
        assert_eq!(query, "j");
        assert_eq!(selected_index, 0);
        assert_eq!(mode, FilePickerInputMode::Insert);

        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        let mode = match &app.ui_state.mode {
            AppMode::FilePicker(state) => state.input_mode,
            _ => panic!("expected file picker mode"),
        };
        assert_eq!(mode, FilePickerInputMode::Normal);
    }

    #[test]
    fn test_settings_navigation_isolated_from_global_vim_view_switching() {
        let mut app = App::new(Config {
            keymap_profile: "vim".to_string(),
            ..Config::default()
        });
        app.ui_state.primary_view = PrimaryView::LiveRun;

        app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));

        let selected_row = match &app.ui_state.mode {
            AppMode::Settings(state) => state.selected_row,
            _ => panic!("expected settings mode"),
        };

        assert_eq!(selected_row, 1);
        assert_eq!(app.ui_state.primary_view, PrimaryView::LiveRun);
    }

    #[test]
    fn test_settings_arrow_keys_do_not_leak_into_global_viewport_controls() {
        let mut app = App::new(Config::default());
        let idx = (app.ui_state.focused_box - 1) as usize;
        app.ui_state.graph_viewports[idx].follow_latest = false;
        app.ui_state.system_viewport.follow_latest = false;
        app.ui_state.graph_viewports[idx].offset_samples = 7;
        app.ui_state.system_viewport.offset_samples = 9;

        app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));

        assert_eq!(app.ui_state.graph_viewports[idx].offset_samples, 7);
        assert_eq!(app.ui_state.system_viewport.offset_samples, 9);
    }

    #[test]
    fn test_adaptive_layout_hides_absent_unpinned_metrics() {
        let mut config = Config::default();
        config.adaptive_layout = true;
        config.pinned_metrics = vec![];
        let app = App::new(config);

        assert!(!app.should_show_metric_panel("tokens_per_second", false));
        assert!(app.should_show_metric_panel("tokens_per_second", true));
    }

    #[test]
    fn test_user_pinned_metric_remains_visible_under_adaptivity() {
        let mut config = Config::default();
        config.adaptive_layout = true;
        config.pinned_metrics = vec!["tokens_per_second".to_string()];
        let app = App::new(config);

        assert!(app.should_show_metric_panel("tokens_per_second", false));
    }

    #[test]
    fn test_adaptive_layout_never_hides_user_pinned_metrics() {
        let mut config = Config::default();
        config.adaptive_layout = true;
        config.pinned_metrics = vec!["steps_per_second".to_string()];
        let app = App::new(config);

        assert!(app.should_show_metric_panel("steps_per_second", false));
    }

    #[test]
    fn test_metric_relevance_prefers_tokens_when_tokens_present() {
        let mut app = App::new(Config::default());

        app.push_metrics(TrainingMetrics {
            samples_per_second: Some(20.0),
            steps_per_second: Some(0.5),
            tokens_per_second: Some(1500.0),
            ..TrainingMetrics::default()
        });

        assert_eq!(app.preferred_rate_metric_id(), "tokens_per_second");
    }

    #[test]
    fn test_metric_relevance_falls_back_to_samples_or_steps() {
        let mut app = App::new(Config::default());

        app.push_metrics(TrainingMetrics {
            samples_per_second: Some(18.0),
            ..TrainingMetrics::default()
        });
        assert_eq!(app.preferred_rate_metric_id(), "samples_per_second");

        app.push_metrics(TrainingMetrics {
            steps_per_second: Some(0.9),
            ..TrainingMetrics::default()
        });
        assert_eq!(app.preferred_rate_metric_id(), "steps_per_second");
    }

    #[test]
    fn test_hidden_metrics_preserve_history_for_reenable() {
        let mut app = App::new(Config::default());
        app.config.adaptive_layout = true;

        app.push_metrics(TrainingMetrics {
            tokens_per_second: Some(1200.0),
            samples_per_second: Some(12.0),
            ..TrainingMetrics::default()
        });

        assert_eq!(app.preferred_rate_metric_id(), "tokens_per_second");
        assert_eq!(app.training.samples_per_second_history.len(), 1);

        app.config
            .hidden_metrics
            .push("samples_per_second".to_string());
        assert!(!app.should_show_metric_panel("samples_per_second", true));

        app.config.hidden_metrics.clear();
        assert!(app.should_show_metric_panel("samples_per_second", true));

        app.push_metrics(TrainingMetrics {
            samples_per_second: Some(15.0),
            ..TrainingMetrics::default()
        });
        assert_eq!(app.training.samples_per_second_history.len(), 2);
    }

    #[test]
    fn test_metric_relevance_handles_sparse_or_switching_streams() {
        let mut app = App::new(Config::default());

        app.push_metrics(TrainingMetrics {
            steps_per_second: Some(0.4),
            ..TrainingMetrics::default()
        });
        assert_eq!(app.preferred_rate_metric_id(), "steps_per_second");

        app.push_metrics(TrainingMetrics {
            tokens_per_second: Some(900.0),
            ..TrainingMetrics::default()
        });
        assert_eq!(app.preferred_rate_metric_id(), "tokens_per_second");

        app.push_metrics(TrainingMetrics {
            samples_per_second: Some(21.0),
            ..TrainingMetrics::default()
        });
        assert_eq!(app.preferred_rate_metric_id(), "samples_per_second");
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
            eval_loss: None,
            grad_norm: None,
            samples_per_second: None,
            steps_per_second: None,
            tokens_per_second: None,
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
    fn test_tab_cycles_primary_views_forward() {
        let mut app = App::new(Config::default());
        assert_eq!(app.ui_state.primary_view, PrimaryView::LiveRun);

        let tab_key = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        app.handle_key(tab_key);
        assert_eq!(app.ui_state.primary_view, PrimaryView::RunExplorer);

        app.handle_key(tab_key);
        assert_eq!(app.ui_state.primary_view, PrimaryView::SystemProcesses);

        app.handle_key(tab_key);
        assert_eq!(app.ui_state.primary_view, PrimaryView::Home); // wrap

        app.handle_key(tab_key);
        assert_eq!(app.ui_state.primary_view, PrimaryView::LiveRun);
    }

    #[test]
    fn test_tab_cycles_primary_views_backward() {
        let mut app = App::new(Config::default());
        assert_eq!(app.ui_state.primary_view, PrimaryView::LiveRun);

        let backtab_key = KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT);
        app.handle_key(backtab_key);
        assert_eq!(app.ui_state.primary_view, PrimaryView::Home);

        app.handle_key(backtab_key);
        assert_eq!(app.ui_state.primary_view, PrimaryView::SystemProcesses); // wrap
    }

    #[test]
    fn test_number_keys_focus_boxes() {
        let mut app = App::new(Config::default());

        app.handle_key(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE));
        assert_eq!(app.ui_state.focused_box, 1);

        app.handle_key(KeyEvent::new(KeyCode::Char('4'), KeyModifiers::NONE));
        assert_eq!(app.ui_state.focused_box, 4);

        // Keys 5-9 should not change focus
        app.handle_key(KeyEvent::new(KeyCode::Char('5'), KeyModifiers::NONE));
        assert_eq!(app.ui_state.focused_box, 4);
    }

    #[test]
    fn test_tab_preserves_focused_box() {
        let mut app = App::new(Config::default());
        app.ui_state.focused_box = 3;

        app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.ui_state.focused_box, 3);
    }

    #[test]
    fn test_primary_view_count() {
        assert_eq!(PrimaryView::COUNT, 4);
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
    fn test_staleness_threshold_uses_config_value() {
        let mut app = App::new(Config {
            stale_after_secs: 30,
            ..Config::default()
        });
        app.training.last_data_at = Some(Instant::now() - Duration::from_secs(11));
        app.training.input_active = true;

        app.on_tick();
        assert!(app.training.input_active);

        app.training.last_data_at = Some(Instant::now() - Duration::from_secs(31));
        app.on_tick();
        assert!(!app.training.input_active);
    }

    #[test]
    fn test_viewport_live_follow_shows_latest() {
        let mut app = App::new(Config::default());

        for i in 0..100 {
            app.push_metrics(TrainingMetrics {
                step: Some(i),
                ..TrainingMetrics::default()
            });
        }

        let series = app.graph_viewport_series(0, &app.training.step_history, 12);
        assert_eq!(series.len(), 12);
        assert_eq!(series.last().copied(), Some(99));

        app.push_metrics(TrainingMetrics {
            step: Some(100),
            ..TrainingMetrics::default()
        });

        let updated = app.graph_viewport_series(0, &app.training.step_history, 12);
        assert_eq!(updated.last().copied(), Some(100));
    }

    #[test]
    fn test_viewport_pan_clamps_bounds() {
        let mut app = App::new(Config::default());

        for i in 0..50 {
            app.push_metrics(TrainingMetrics {
                step: Some(i),
                ..TrainingMetrics::default()
            });
        }

        app.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        let idx = (app.ui_state.focused_box - 1) as usize;
        app.ui_state.graph_viewports[idx].zoom_level = 1;
        app.ui_state.graph_viewports[idx].offset_samples = usize::MAX;

        let series = app.graph_viewport_series(0, &app.training.step_history, 10);
        assert_eq!(series.len(), 10);
        assert_eq!(series.first().copied(), Some(0));
        assert_eq!(series.last().copied(), Some(24));
    }

    #[test]
    fn test_viewport_zoom_clamps_and_reslices() {
        let mut app = App::new(Config::default());

        for i in 0..256 {
            app.push_metrics(TrainingMetrics {
                step: Some(i),
                ..TrainingMetrics::default()
            });
        }

        let idx = (app.ui_state.focused_box - 1) as usize;
        let baseline = app.graph_viewport_series(idx, &app.training.step_history, 16);
        assert_eq!(baseline.len(), 16);
        assert_eq!(baseline.first().copied(), Some(0));
        assert_eq!(baseline.last().copied(), Some(255));

        for _ in 0..20 {
            app.handle_key(KeyEvent::new(KeyCode::Char('-'), KeyModifiers::NONE));
        }
        assert_eq!(app.ui_state.graph_viewports[idx].zoom_level, 0);

        let zoomed_out = app.graph_viewport_series(idx, &app.training.step_history, 16);
        assert_eq!(zoomed_out.len(), 16);
        assert_eq!(zoomed_out, baseline);

        for _ in 0..20 {
            app.handle_key(KeyEvent::new(KeyCode::Char('='), KeyModifiers::NONE));
        }
        assert_eq!(
            app.ui_state.graph_viewports[idx].zoom_level,
            App::VIEWPORT_MAX_ZOOM_LEVEL
        );

        let zoomed_in = app.graph_viewport_series(idx, &app.training.step_history, 16);
        assert_eq!(zoomed_in.len(), 16);
        assert_ne!(zoomed_in.first(), baseline.first());
        assert_eq!(zoomed_in.last().copied(), Some(255));
    }

    #[test]
    fn test_sampling_is_deterministic_for_same_input() {
        let history = (0..512)
            .map(|i| if i % 3 == 0 { 100 } else { 10 })
            .collect::<VecDeque<_>>();
        let viewport = ViewportState {
            follow_latest: true,
            offset_samples: 0,
            zoom_level: 0,
        };

        let first = App::viewport_series(&history, viewport, 32);
        let second = App::viewport_series(&history, viewport, 32);
        assert_eq!(first, second);
    }

    #[test]
    fn test_sampling_preserves_extrema_per_bucket() {
        let mut history = VecDeque::new();
        for _ in 0..32 {
            history.push_back(10);
            history.push_back(90);
            history.push_back(20);
            history.push_back(80);
        }

        let viewport = ViewportState {
            follow_latest: true,
            offset_samples: 0,
            zoom_level: 0,
        };
        let sampled = App::viewport_series(&history, viewport, 24);

        assert_eq!(sampled.len(), 24);
        assert!(sampled.contains(&10));
        assert!(sampled.contains(&90));
    }

    #[test]
    fn test_zoom_out_high_frequency_stream_no_jitter_regression() {
        let mut history = VecDeque::new();
        for i in 0..400 {
            history.push_back(if i % 2 == 0 { 0 } else { 100 });
        }

        let viewport = ViewportState {
            follow_latest: true,
            offset_samples: 0,
            zoom_level: 0,
        };

        let before = App::viewport_series(&history, viewport, 40);
        history.push_back(55);
        let after = App::viewport_series(&history, viewport, 40);

        assert_eq!(before.len(), 40);
        assert_eq!(after.len(), 40);
        assert_eq!(before.first(), after.first());
        assert_ne!(before.last(), after.last());
    }

    #[test]
    fn test_append_stream_sampling_avoids_stride_jump_regressions() {
        let viewport = ViewportState {
            follow_latest: true,
            offset_samples: 0,
            zoom_level: 0,
        };

        for end in 21..120 {
            let history = (0..end as u64).collect::<VecDeque<_>>();
            let sampled = App::viewport_series(&history, viewport, 20);

            assert_eq!(sampled.len(), 20);
            assert_eq!(sampled.first().copied(), Some(0));
            assert_eq!(sampled.last().copied(), Some(end as u64 - 1));
            assert!(sampled.windows(2).all(|w| w[0] <= w[1]));
        }
    }

    #[test]
    fn test_sampling_handles_minimum_width_without_panic() {
        let history = (10..90).collect::<VecDeque<_>>();
        let viewport = ViewportState {
            follow_latest: true,
            offset_samples: 0,
            zoom_level: 0,
        };

        let width_one = App::viewport_series(&history, viewport, 1);
        assert_eq!(width_one.len(), 1);
        assert_eq!(width_one[0], 89);

        let width_two = App::viewport_series(&history, viewport, 2);
        assert_eq!(width_two, vec![10, 89]);
    }

    #[test]
    fn test_startup_autofit_sets_follow_latest_and_zero_offset() {
        let app = App::new(Config::default());

        assert!(app.ui_state.graph_viewports[0].follow_latest);
        assert_eq!(app.ui_state.graph_viewports[0].offset_samples, 0);
        assert_eq!(app.ui_state.graph_viewports[0].zoom_level, 0);
        assert!(app.ui_state.system_viewport.follow_latest);
        assert_eq!(app.ui_state.system_viewport.offset_samples, 0);
        assert_eq!(app.ui_state.system_viewport.zoom_level, 0);
    }

    #[test]
    fn test_startup_default_is_min_zoom_follow_latest() {
        let mut app = App::new(Config::default());
        for i in 0..100 {
            app.push_metrics(TrainingMetrics {
                step: Some(i),
                ..TrainingMetrics::default()
            });
        }

        let series = app.graph_viewport_series(0, &app.training.step_history, 10);
        assert_eq!(series.len(), 10);
        assert_eq!(series.first().copied(), Some(0));
        assert_eq!(series.last().copied(), Some(99));
    }

    #[test]
    fn test_min_zoom_autofit_disables_pan() {
        let mut app = App::new(Config::default());
        let idx = (app.ui_state.focused_box - 1) as usize;
        app.ui_state.graph_viewports[idx].follow_latest = false;
        app.ui_state.graph_viewports[idx].zoom_level = 0;

        app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));

        assert_eq!(app.ui_state.graph_viewports[idx].offset_samples, 0);
    }

    #[test]
    fn test_min_zoom_pan_input_is_noop() {
        let mut app = App::new(Config {
            keymap_profile: "vim".to_string(),
            ..Config::default()
        });
        let idx = (app.ui_state.focused_box - 1) as usize;
        app.ui_state.graph_viewports[idx].follow_latest = false;
        app.ui_state.graph_viewports[idx].zoom_level = 0;

        app.handle_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE));

        assert_eq!(app.ui_state.graph_viewports[idx].offset_samples, 0);
    }

    #[test]
    fn test_resize_reautofit_only_when_min_zoom_follow_latest() {
        let mut app = App::new(Config::default());
        for i in 0..80 {
            app.push_metrics(TrainingMetrics {
                step: Some(i),
                ..TrainingMetrics::default()
            });
        }

        let wide = app.graph_viewport_series(0, &app.training.step_history, 20);
        let narrow = app.graph_viewport_series(0, &app.training.step_history, 8);

        assert_eq!(wide.first().copied(), Some(0));
        assert_eq!(wide.last().copied(), Some(79));
        assert_eq!(narrow.first().copied(), Some(0));
        assert_eq!(narrow.last().copied(), Some(79));

        app.ui_state.graph_viewports[0].follow_latest = false;
        app.ui_state.graph_viewports[0].zoom_level = 1;
        app.ui_state.graph_viewports[0].offset_samples = 15;
        let before = app.ui_state.graph_viewports[0].offset_samples;
        let _ = app.graph_viewport_series(0, &app.training.step_history, 12);
        let _ = app.graph_viewport_series(0, &app.training.step_history, 6);

        assert_eq!(app.ui_state.graph_viewports[0].offset_samples, before);
    }

    #[test]
    fn test_resize_while_paused_preserves_non_min_viewport() {
        let mut app = App::new(Config::default());
        for i in 0..80 {
            app.push_metrics(TrainingMetrics {
                step: Some(i),
                ..TrainingMetrics::default()
            });
        }

        app.ui_state.graph_viewports[0].follow_latest = false;
        app.ui_state.graph_viewports[0].zoom_level = 2;
        app.ui_state.graph_viewports[0].offset_samples = 11;
        let before = app.ui_state.graph_viewports[0].offset_samples;

        let _ = app.graph_viewport_series(0, &app.training.step_history, 18);
        let _ = app.graph_viewport_series(0, &app.training.step_history, 7);

        assert_eq!(app.ui_state.graph_viewports[0].offset_samples, before);
    }

    #[test]
    fn test_reset_g_restores_min_zoom_autofit_contract() {
        let mut app = App::new(Config::default());
        app.ui_state.graph_viewports[0].follow_latest = false;
        app.ui_state.system_viewport.follow_latest = false;
        app.ui_state.graph_viewports[0].zoom_level = App::VIEWPORT_MAX_ZOOM_LEVEL;
        app.ui_state.system_viewport.zoom_level = App::VIEWPORT_MAX_ZOOM_LEVEL;
        app.ui_state.graph_viewports[0].offset_samples = 42;
        app.ui_state.system_viewport.offset_samples = 42;

        app.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));

        assert!(app.ui_state.graph_viewports[0].follow_latest);
        assert!(app.ui_state.system_viewport.follow_latest);
        assert_eq!(app.ui_state.graph_viewports[0].offset_samples, 0);
        assert_eq!(app.ui_state.system_viewport.offset_samples, 0);
        assert_eq!(app.ui_state.graph_viewports[0].zoom_level, 0);
        assert_eq!(app.ui_state.system_viewport.zoom_level, 0);
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
            eval_loss: Some(0.45),
            grad_norm: Some(1.75),
            samples_per_second: Some(12.0),
            steps_per_second: Some(0.5),
            tokens_per_second: Some(1500.0),
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
        assert_eq!(app.training.throughput_history[0], 1500);

        assert_eq!(app.training.tokens_history.len(), 1);
        assert_eq!(app.training.tokens_history[0], 50000);

        assert_eq!(app.training.eval_loss_history.len(), 1);
        assert_eq!(app.training.eval_loss_history[0], 450);

        assert_eq!(app.training.grad_norm_history.len(), 1);
        assert_eq!(app.training.grad_norm_history[0], 1750);

        assert_eq!(app.training.samples_per_second_history.len(), 1);
        assert_eq!(app.training.samples_per_second_history[0], 12);

        assert_eq!(app.training.steps_per_second_history.len(), 1);
        assert_eq!(app.training.steps_per_second_history[0], 500);

        assert_eq!(app.training.tokens_per_second_history.len(), 1);
        assert_eq!(app.training.tokens_per_second_history[0], 1500);

        assert_eq!(app.training.total_steps, 100);
    }

    #[test]
    fn test_push_metrics_appends_new_core_histories() {
        let mut app = App::new(Config::default());
        app.push_metrics(TrainingMetrics {
            tokens: Some(1200),
            eval_loss: Some(0.75),
            grad_norm: Some(2.0),
            samples_per_second: Some(21.0),
            steps_per_second: Some(0.75),
            tokens_per_second: Some(3000.0),
            ..TrainingMetrics::default()
        });

        assert_eq!(app.training.tokens_history.len(), 1);
        assert_eq!(app.training.eval_loss_history.len(), 1);
        assert_eq!(app.training.grad_norm_history.len(), 1);
        assert_eq!(app.training.samples_per_second_history.len(), 1);
        assert_eq!(app.training.steps_per_second_history.len(), 1);
        assert_eq!(app.training.tokens_per_second_history.len(), 1);
    }

    #[test]
    fn test_new_histories_respect_capacity() {
        let config = Config {
            history_size: 3,
            ..Config::default()
        };
        let mut app = App::new(config);

        for i in 0..10 {
            app.push_metrics(TrainingMetrics {
                tokens: Some(i),
                eval_loss: Some(i as f64),
                grad_norm: Some(i as f64),
                samples_per_second: Some(i as f64),
                steps_per_second: Some(i as f64),
                tokens_per_second: Some(i as f64),
                ..TrainingMetrics::default()
            });
        }

        assert_eq!(app.training.tokens_history.len(), 3);
        assert_eq!(app.training.eval_loss_history.len(), 3);
        assert_eq!(app.training.grad_norm_history.len(), 3);
        assert_eq!(app.training.samples_per_second_history.len(), 3);
        assert_eq!(app.training.steps_per_second_history.len(), 3);
        assert_eq!(app.training.tokens_per_second_history.len(), 3);
    }

    #[test]
    fn test_legacy_throughput_fallback_remains_intact() {
        let mut app = App::new(Config::default());
        app.push_metrics(TrainingMetrics {
            throughput: Some(42.0),
            ..TrainingMetrics::default()
        });

        assert_eq!(app.training.throughput_history.len(), 1);
        assert_eq!(app.training.throughput_history[0], 42);
    }

    #[test]
    fn test_perplexity_derived_from_loss() {
        let mut app = App::new(Config::default());
        app.push_metrics(TrainingMetrics {
            loss: Some(1.0),
            ..TrainingMetrics::default()
        });

        let perplexity = app
            .training
            .perplexity_latest
            .expect("perplexity should be calculated");
        assert!((perplexity - std::f64::consts::E).abs() < 1e-6);
    }

    #[test]
    fn test_loss_spike_counter_increments_on_threshold_cross() {
        let mut app = App::new(Config::default());

        for _ in 0..25 {
            app.push_metrics(TrainingMetrics {
                loss: Some(1.0),
                ..TrainingMetrics::default()
            });
        }

        let before = app.training.loss_spike_count;
        app.push_metrics(TrainingMetrics {
            loss: Some(1.5),
            ..TrainingMetrics::default()
        });
        let after = app.training.loss_spike_count;

        assert_eq!(after, before + 1);
        assert!(app.training.last_loss_spike_at.is_some());
    }

    #[test]
    fn test_nan_inf_counter_tracks_invalid_metrics() {
        let mut app = App::new(Config::default());
        app.push_metrics(TrainingMetrics {
            loss: Some(f64::NAN),
            grad_norm: Some(f64::INFINITY),
            ..TrainingMetrics::default()
        });

        assert_eq!(app.training.nan_inf_count, 2);
        assert!(app.training.last_nan_inf_at.is_some());
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
    fn test_alerts_disabled_when_unconfigured() {
        let mut app = App::new(Config::default());
        app.push_metrics(TrainingMetrics {
            throughput: Some(1.0),
            ..TrainingMetrics::default()
        });
        app.on_tick();
        assert!(app.alerts.active.is_empty());
        assert!(app.alerts.resolved.is_empty());
    }

    #[test]
    fn test_alerts_clear_when_rules_removed_or_disabled() {
        let mut config = Config::default();
        config.alert_rules = vec![AlertRuleConfig {
            id: Some("throughput_drop".to_string()),
            kind: AlertRuleKind::ThroughputDrop,
            mode: AlertEvalMode::Current,
            warning: 100.0,
            critical: 50.0,
            enabled: true,
        }];
        let mut app = App::new(config);

        app.push_metrics(TrainingMetrics {
            throughput: Some(40.0),
            ..TrainingMetrics::default()
        });
        assert_eq!(app.alerts.active.len(), 1);

        app.config.alert_rules.clear();
        app.on_tick();
        assert!(app.alerts.active.is_empty());

        app.config.alert_rules = vec![AlertRuleConfig {
            id: Some("throughput_drop".to_string()),
            kind: AlertRuleKind::ThroughputDrop,
            mode: AlertEvalMode::Current,
            warning: 100.0,
            critical: 50.0,
            enabled: false,
        }];
        app.push_metrics(TrainingMetrics {
            throughput: Some(40.0),
            ..TrainingMetrics::default()
        });
        app.on_tick();
        assert!(app.alerts.active.is_empty());
    }

    #[test]
    fn test_alert_threshold_warning_and_critical_transitions() {
        let mut config = Config::default();
        config.alert_rules = vec![AlertRuleConfig {
            id: Some("throughput_drop".to_string()),
            kind: AlertRuleKind::ThroughputDrop,
            mode: AlertEvalMode::Current,
            warning: 100.0,
            critical: 50.0,
            enabled: true,
        }];
        let mut app = App::new(config);

        app.push_metrics(TrainingMetrics {
            throughput: Some(90.0),
            ..TrainingMetrics::default()
        });
        assert_eq!(app.alerts.active.len(), 1);
        assert_eq!(app.alerts.active[0].level, AlertLevel::Warning);

        app.push_metrics(TrainingMetrics {
            throughput: Some(40.0),
            ..TrainingMetrics::default()
        });
        assert_eq!(app.alerts.active[0].level, AlertLevel::Critical);

        app.push_metrics(TrainingMetrics {
            throughput: Some(140.0),
            ..TrainingMetrics::default()
        });
        assert!(app.alerts.active.is_empty());
        assert_eq!(app.alerts.resolved.len(), 1);
    }

    #[test]
    fn test_alert_threshold_hysteresis_prevents_flapping() {
        let mut config = Config::default();
        config.alert_rules = vec![AlertRuleConfig {
            id: Some("memory_pressure".to_string()),
            kind: AlertRuleKind::MemoryPressure,
            mode: AlertEvalMode::Current,
            warning: 80.0,
            critical: 90.0,
            enabled: true,
        }];
        let mut app = App::new(config);

        app.push_system(SystemMetrics {
            cpu_usage: 50.0,
            memory_used: 82,
            memory_total: 100,
            gpus: vec![],
        });
        app.on_tick();
        assert_eq!(app.alerts.active.len(), 1);

        app.push_system(SystemMetrics {
            cpu_usage: 50.0,
            memory_used: 78,
            memory_total: 100,
            gpus: vec![],
        });
        app.on_tick();

        assert_eq!(app.alerts.active.len(), 1);
        assert!(app.alerts.resolved.is_empty());
    }

    #[test]
    fn test_alert_cooldown_blocks_immediate_refire_then_allows_reentry() {
        let mut config = Config::default();
        config.alert_rules = vec![AlertRuleConfig {
            id: Some("throughput_drop".to_string()),
            kind: AlertRuleKind::ThroughputDrop,
            mode: AlertEvalMode::Current,
            warning: 100.0,
            critical: 50.0,
            enabled: true,
        }];
        let mut app = App::new(config);

        app.push_metrics(TrainingMetrics {
            throughput: Some(40.0),
            ..TrainingMetrics::default()
        });
        assert_eq!(app.alerts.active.len(), 1);

        app.push_metrics(TrainingMetrics {
            throughput: Some(140.0),
            ..TrainingMetrics::default()
        });
        assert!(app.alerts.active.is_empty());
        assert_eq!(app.alerts.resolved.len(), 1);

        app.push_metrics(TrainingMetrics {
            throughput: Some(40.0),
            ..TrainingMetrics::default()
        });
        assert!(app.alerts.active.is_empty());

        for _ in 0..30 {
            app.on_tick();
        }

        app.push_metrics(TrainingMetrics {
            throughput: Some(40.0),
            ..TrainingMetrics::default()
        });
        assert_eq!(app.alerts.active.len(), 1);
        assert_eq!(app.alerts.active[0].rule_id, "throughput_drop");
    }

    #[test]
    fn test_loss_trend_worsening_uses_rolling_mean_slope_formula() {
        let mut config = Config::default();
        config.alert_rules = vec![AlertRuleConfig {
            id: Some("loss_trend_worsening".to_string()),
            kind: AlertRuleKind::LossTrendWorsening,
            mode: AlertEvalMode::RollingMean { window: 10 },
            warning: 0.001,
            critical: 0.003,
            enabled: true,
        }];
        let mut app = App::new(config);

        for i in 0..40 {
            app.push_metrics(TrainingMetrics {
                loss: Some(0.5 + (i as f64 * 0.01)),
                step: Some(i),
                ..TrainingMetrics::default()
            });
        }

        assert!(!app.alerts.active.is_empty());
        assert_eq!(app.alerts.active[0].rule_id, "loss_trend_worsening");
    }

    #[test]
    fn test_run_compare_alignment_by_step() {
        let mut app = App::new(Config::default());
        for i in 1..=3 {
            app.push_metrics(TrainingMetrics {
                step: Some(i),
                loss: Some(i as f64),
                ..TrainingMetrics::default()
            });
        }
        app.set_run_comparison_snapshot(vec![
            TrainingMetrics {
                step: Some(2),
                loss: Some(5.0),
                ..TrainingMetrics::default()
            },
            TrainingMetrics {
                step: Some(3),
                loss: Some(6.0),
                ..TrainingMetrics::default()
            },
        ]);

        let aligned = app.run_compare_alignment_by_step();
        assert!(
            aligned
                .iter()
                .any(|(step, c, b)| *step == 2 && c.is_some() && b.is_some())
        );
    }

    #[test]
    fn test_run_compare_fallback_alignment_when_step_missing() {
        let mut app = App::new(Config::default());
        app.push_metrics(TrainingMetrics {
            loss: Some(1.0),
            ..TrainingMetrics::default()
        });
        app.push_metrics(TrainingMetrics {
            loss: Some(2.0),
            ..TrainingMetrics::default()
        });
        app.set_run_comparison_snapshot(vec![
            TrainingMetrics {
                loss: Some(2.5),
                ..TrainingMetrics::default()
            },
            TrainingMetrics {
                loss: Some(3.5),
                ..TrainingMetrics::default()
            },
        ]);

        let aligned = app.run_compare_fallback_alignment();
        assert_eq!(aligned.len(), 2);
        assert!(aligned[0].0.is_some());
        assert!(aligned[0].1.is_some());
    }

    #[test]
    fn test_run_compare_uses_snapshot_mode_without_follow_tail() {
        let mut app = App::new(Config::default());
        app.set_run_comparison_snapshot(vec![TrainingMetrics {
            step: Some(1),
            loss: Some(1.0),
            ..TrainingMetrics::default()
        }]);
        let before = app.run_comparison.baseline_loss_history.len();

        app.push_metrics(TrainingMetrics {
            step: Some(2),
            loss: Some(0.9),
            ..TrainingMetrics::default()
        });

        assert!(app.run_comparison_snapshot_mode());
        assert_eq!(app.run_comparison.baseline_loss_history.len(), before);
    }

    #[test]
    fn test_run_compare_duplicate_steps_keep_last_seen_deterministically() {
        let mut app = App::new(Config::default());
        app.set_run_comparison_snapshot(vec![
            TrainingMetrics {
                step: Some(10),
                loss: Some(1.0),
                ..TrainingMetrics::default()
            },
            TrainingMetrics {
                step: Some(10),
                loss: Some(2.0),
                ..TrainingMetrics::default()
            },
        ]);
        assert_eq!(app.run_comparison.baseline_step_history.len(), 1);
        assert_eq!(
            app.run_comparison.baseline_loss_history.back().copied(),
            Some(2000)
        );
    }

    #[test]
    fn test_two_tab_key_docs_contract() {
        let entries = keymap_entries("default");
        assert!(
            entries
                .iter()
                .any(|(key, desc)| key == "Tab / Shift+Tab" && desc == "Switch view")
        );
    }

    #[test]
    fn test_min_zoom_alerts_and_compare_coexist_without_panics() {
        let mut config = Config::default();
        config.alert_rules = vec![AlertRuleConfig {
            id: Some("throughput_drop".to_string()),
            kind: AlertRuleKind::ThroughputDrop,
            mode: AlertEvalMode::Current,
            warning: 100.0,
            critical: 50.0,
            enabled: true,
        }];
        let mut app = App::new(config);

        app.set_run_comparison_snapshot(vec![TrainingMetrics {
            step: Some(1),
            loss: Some(1.0),
            ..TrainingMetrics::default()
        }]);
        app.push_metrics(TrainingMetrics {
            step: Some(1),
            loss: Some(0.8),
            throughput: Some(40.0),
            ..TrainingMetrics::default()
        });
        app.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
        app.on_tick();

        let _ = app.graph_viewport_series(0, &app.training.loss_history, 8);
        let _ = app.run_compare_alignment_by_step();
        assert!(app.ui_state.graph_viewports[0].zoom_level == 0);
    }

    #[test]
    fn test_app_new() {
        let app = App::new(Config::default());
        assert!(app.running);
        assert_eq!(app.ui_state.primary_view, PrimaryView::LiveRun);
        assert_eq!(app.ui_state.focused_box, 1);
        assert!(app.training.latest.is_none());
    }

    #[test]
    fn test_app_new_initializes_new_fields() {
        let app = App::new(Config::default());
        assert!(app.run_store.is_none());
        assert!(app.project_root.is_none());
        assert!(app.recent_runs.is_empty());
        assert!(app.discovered_files.is_empty());
        assert!(app.ui_state.explorer.records.is_empty());
        assert_eq!(app.ui_state.explorer.selected_idx, 0);
        assert!(!app.ui_state.explorer.search_active);
        assert!(app.ui_state.explorer.search_query.is_empty());
        assert!(app.ui_state.explorer.status_filter.is_none());
        assert_eq!(app.ui_state.selected_process_idx, 0);
    }

    #[test]
    fn test_home_key_e_switches_to_run_explorer() {
        let mut app = App::new(Config::default());
        app.ui_state.primary_view = PrimaryView::Home;
        app.handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE));
        assert_eq!(app.ui_state.primary_view, PrimaryView::RunExplorer);
    }

    #[test]
    fn test_home_key_o_enters_scanning_mode() {
        let mut app = App::new(Config::default());
        app.ui_state.primary_view = PrimaryView::Home;
        app.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE));
        assert_eq!(app.ui_state.mode, AppMode::Scanning);
    }

    #[test]
    fn test_home_key_r_refreshes_without_store() {
        let mut app = App::new(Config::default());
        app.ui_state.primary_view = PrimaryView::Home;
        app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
        assert!(app.recent_runs.is_empty());
    }

    #[test]
    fn test_explorer_slash_activates_search() {
        let mut app = App::new(Config::default());
        app.ui_state.primary_view = PrimaryView::RunExplorer;
        assert!(!app.ui_state.explorer.search_active);
        app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        assert!(app.ui_state.explorer.search_active);
    }

    #[test]
    fn test_explorer_f_cycles_status_filter() {
        let mut app = App::new(Config::default());
        app.ui_state.primary_view = PrimaryView::RunExplorer;

        use crate::store::types::RunStatus;
        assert!(app.ui_state.explorer.status_filter.is_none());

        app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE));
        assert_eq!(app.ui_state.explorer.status_filter, Some(RunStatus::Active));

        app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE));
        assert_eq!(
            app.ui_state.explorer.status_filter,
            Some(RunStatus::Completed)
        );

        app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE));
        assert_eq!(app.ui_state.explorer.status_filter, Some(RunStatus::Failed));

        app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE));
        assert!(app.ui_state.explorer.status_filter.is_none());
    }

    #[test]
    fn test_explorer_j_moves_cursor() {
        let mut app = App::new(Config::default());
        app.ui_state.primary_view = PrimaryView::RunExplorer;

        use crate::store::types::{RunRecord, RunSourceKind, RunStatus};
        let dummy_record = RunRecord {
            run_id: "r1".to_string(),
            source_fingerprint: "fp".to_string(),
            source_kind: RunSourceKind::LogFile,
            source_locator: None,
            project_root: None,
            display_name: None,
            status: RunStatus::Active,
            command: None,
            cwd: None,
            git_commit: None,
            git_dirty: None,
            started_at_epoch_secs: 0,
            ended_at_epoch_secs: None,
            last_step: None,
            last_updated_epoch_secs: 0,
        };
        app.ui_state.explorer.records = vec![dummy_record.clone(), dummy_record];
        assert_eq!(app.ui_state.explorer.selected_idx, 0);

        app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        assert_eq!(app.ui_state.explorer.selected_idx, 1);

        app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        assert_eq!(app.ui_state.explorer.selected_idx, 1);
    }

    #[test]
    fn test_explorer_k_moves_cursor_up() {
        let mut app = App::new(Config::default());
        app.ui_state.primary_view = PrimaryView::RunExplorer;

        use crate::store::types::{RunRecord, RunSourceKind, RunStatus};
        let dummy = RunRecord {
            run_id: "r1".to_string(),
            source_fingerprint: "fp".to_string(),
            source_kind: RunSourceKind::LogFile,
            source_locator: None,
            project_root: None,
            display_name: None,
            status: RunStatus::Active,
            command: None,
            cwd: None,
            git_commit: None,
            git_dirty: None,
            started_at_epoch_secs: 0,
            ended_at_epoch_secs: None,
            last_step: None,
            last_updated_epoch_secs: 0,
        };
        app.ui_state.explorer.records = vec![dummy.clone(), dummy];
        app.ui_state.explorer.selected_idx = 1;

        app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
        assert_eq!(app.ui_state.explorer.selected_idx, 0);

        app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
        assert_eq!(app.ui_state.explorer.selected_idx, 0);
    }

    #[test]
    fn test_system_processes_j_moves_cursor() {
        use crate::collectors::process::{ProbeStatus, ProcessCandidate};
        let mut app = App::new(Config::default());
        app.ui_state.primary_view = PrimaryView::SystemProcesses;
        app.discovered_processes = vec![
            ProcessCandidate {
                pid: 1,
                command: "a".to_string(),
                cwd: None,
                cpu_milli_percent: 0,
                memory_bytes: 0,
                status: ProbeStatus::Ok,
                pid_reused: false,
            },
            ProcessCandidate {
                pid: 2,
                command: "b".to_string(),
                cwd: None,
                cpu_milli_percent: 0,
                memory_bytes: 0,
                status: ProbeStatus::Ok,
                pid_reused: false,
            },
        ];
        assert_eq!(app.ui_state.selected_process_idx, 0);

        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(app.ui_state.selected_process_idx, 1);

        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(app.ui_state.selected_process_idx, 1);
    }

    #[test]
    fn test_set_discovered_processes_clamps_cursor() {
        use crate::collectors::process::{ProbeStatus, ProcessCandidate};
        let mut app = App::new(Config::default());
        app.ui_state.selected_process_idx = 5;

        app.set_discovered_processes(vec![
            ProcessCandidate {
                pid: 1,
                command: "a".to_string(),
                cwd: None,
                cpu_milli_percent: 0,
                memory_bytes: 0,
                status: ProbeStatus::Ok,
                pid_reused: false,
            },
            ProcessCandidate {
                pid: 2,
                command: "b".to_string(),
                cwd: None,
                cpu_milli_percent: 0,
                memory_bytes: 0,
                status: ProbeStatus::Ok,
                pid_reused: false,
            },
        ]);
        assert_eq!(app.ui_state.selected_process_idx, 1);
    }

    #[test]
    fn test_set_discovered_files_stores_files() {
        use crate::discovery::FileFormat;
        let mut app = App::new(Config::default());
        let files = sample_discovered_files();
        app.set_discovered_files(files.clone());
        assert_eq!(app.discovered_files.len(), 2);
        assert_eq!(app.discovered_files[0].format, FileFormat::Jsonl);
    }

    #[test]
    fn test_keymap_entries_contains_new_bindings() {
        let entries = keymap_entries("default");
        assert!(entries.iter().any(|(k, _)| k == "Home: e"));
        assert!(entries.iter().any(|(k, _)| k == "Explorer: /"));
        assert!(entries.iter().any(|(k, _)| k == "Processes: a"));
    }

    #[test]
    fn test_explorer_search_mode_chars_update_query() {
        let mut app = App::new(Config::default());
        app.ui_state.primary_view = PrimaryView::RunExplorer;
        app.ui_state.explorer.search_active = true;

        app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE));
        assert_eq!(app.ui_state.explorer.search_query, "foo");

        app.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        assert_eq!(app.ui_state.explorer.search_query, "fo");

        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!app.ui_state.explorer.search_active);
    }
}
