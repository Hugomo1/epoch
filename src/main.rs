use std::io::{self, Stdout};
use std::path::PathBuf;

use clap::Parser;
use color_eyre::Result;
use color_eyre::eyre::Context;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::sync::mpsc;
use tokio::time::Duration;

use epoch::home::service::{empty_snapshot, load_or_build_cached_snapshot, snapshot_cache_path};
use epoch::store::repository::{RunStore, global_store_path, source_fingerprint};
use epoch::store::types::{RunMetadata, RunSourceKind, RunStatus};

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Path to training log file to monitor
    log_file: Option<PathBuf>,

    /// Read training metrics from stdin
    #[arg(long, conflicts_with = "log_file")]
    stdin: bool,

    /// Override log parser (auto, jsonl, csv, regex)
    #[arg(long)]
    parser: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(panic_info);
    }));

    color_eyre::install()?;

    let cli = Cli::parse();

    let project_root = std::env::current_dir()
        .ok()
        .and_then(|cwd| epoch::project_resolution::resolve_project_identity(&cwd, &[], &[], &[]));
    let mut config = epoch::config::Config::load_effective(project_root.as_deref())?;
    config.merge_cli_args(cli.log_file, cli.stdin, cli.parser);

    setup_tracing();

    let mut terminal = setup_terminal().context("failed to setup terminal")?;

    let run_result: Result<()> = async {
        let mut app = epoch::app::App::new(config.clone());

        if !config.stdin_mode
            && config.log_file.is_none()
            && let Some(cache_path) = snapshot_cache_path()
        {
            let _snapshot = load_or_build_cached_snapshot(&cache_path, || {
                empty_snapshot(epoch::store::types::now_epoch_secs())
            });
        }

        if !config.stdin_mode && config.log_file.is_none() {
            app.ui_state.mode = epoch::app::AppMode::Scanning;
        }

        let (event_tx, mut event_rx) = mpsc::channel(epoch::event::EVENT_CHANNEL_CAPACITY);
        let (metrics_tx, mut metrics_rx) = mpsc::channel(epoch::event::METRICS_CHANNEL_CAPACITY);
        let (system_tx, mut system_rx) = mpsc::channel(epoch::event::SYSTEM_CHANNEL_CAPACITY);
        let (process_tx, mut process_rx) = mpsc::channel(8);

        let _event_handle = epoch::event::spawn_event_reader(event_tx.clone());
        let _tick_handle =
            epoch::event::spawn_tick(event_tx, Duration::from_millis(config.tick_rate_ms));

        if matches!(app.ui_state.mode, epoch::app::AppMode::Scanning) {
            let discovered = run_scanning_mode(&mut terminal, &mut app, &mut event_rx).await?;
            app.ui_state.mode = epoch::app::AppMode::FilePicker(
                epoch::app::FilePickerState::new_for_keymap(discovered, &config.keymap_profile),
            );
        }

        if !matches!(app.ui_state.mode, epoch::app::AppMode::Monitoring) {
            run_startup_mode(&mut terminal, &mut app, &mut event_rx).await?;

            if let Some(selected_file) = app.ui_state.selected_file.take() {
                config.log_file = Some(selected_file);
            }
        }

        let run_store = global_store_path().and_then(|path| RunStore::open(&path).ok());
        let mut active_run_id: Option<String> = None;

        if config.stdin_mode {
            if let Some(store) = run_store.as_ref() {
                let cwd = std::env::current_dir()
                    .ok()
                    .map(|path| path.to_string_lossy().to_string());
                let project_root = project_root
                    .as_ref()
                    .map(|path| path.to_string_lossy().to_string());
                let attach = store.attach_or_create_active_run(
                    &source_fingerprint(RunSourceKind::Stdin, Some("stdin"), project_root.as_deref()),
                    RunSourceKind::Stdin,
                    RunMetadata {
                        display_name: Some("stdin session".to_string()),
                        project_root,
                        command: None,
                        cwd,
                        git_commit: None,
                        git_dirty: None,
                        source_locator: Some("stdin".to_string()),
                    },
                );
                if let Ok(result) = attach {
                    active_run_id = Some(result.run_id);
                }
            }
        } else if let Some(path) = config.log_file.as_ref()
            && let Some(store) = run_store.as_ref()
        {
            let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.clone());
            let source_locator = canonical.to_string_lossy().to_string();
            let cwd = std::env::current_dir()
                .ok()
                .map(|cwd_path| cwd_path.to_string_lossy().to_string());
            let project_root_text = project_root
                .as_ref()
                .map(|project_path| project_path.to_string_lossy().to_string());
            let display_name = canonical
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.to_string());

            let attach = store.attach_or_create_active_run(
                &source_fingerprint(
                    RunSourceKind::LogFile,
                    Some(&source_locator),
                    project_root_text.as_deref(),
                ),
                RunSourceKind::LogFile,
                RunMetadata {
                    display_name,
                    project_root: project_root_text,
                    command: None,
                    cwd,
                    git_commit: None,
                    git_dirty: None,
                    source_locator: Some(source_locator),
                },
            );
            if let Ok(result) = attach {
                active_run_id = Some(result.run_id);
            }
        }

        if !app.running {
            if let (Some(store), Some(run_id)) = (run_store.as_ref(), active_run_id.as_deref()) {
                let _ = store.complete_run(run_id, RunStatus::Completed);
            }
            return Ok(());
        }

        if let Some(path) = config.run_comparison_file.clone() {
            let baseline = epoch::collectors::training::parse_snapshot(path.clone(), &config)
                .with_context(|| {
                    format!("failed to parse comparison snapshot: {}", path.display())
                })?;
            app.set_run_comparison_snapshot(baseline);
        }

        let _training_handle = spawn_training_source(metrics_tx, &config)?;
        let _system_handle =
            spawn_system_collector(system_tx, Duration::from_millis(config.tick_rate_ms));
        let _process_handle = spawn_process_collector(
            process_tx,
            Duration::from_millis(config.tick_rate_ms.saturating_mul(4)),
        );

        while app.running {
            terminal.draw(|frame| epoch::ui::render(frame, &app))?;

            tokio::select! {
                Some(event) = event_rx.recv() => app.handle_event(event),
                Some(metrics) = metrics_rx.recv() => {
                    let step = metrics.step;
                    app.push_metrics(metrics);
                    if let (Some(store), Some(run_id), Some(step)) = (run_store.as_ref(), active_run_id.as_deref(), step) {
                        let _ = store.update_last_step(run_id, step);
                    }
                },
                Some(system) = system_rx.recv() => app.push_system(system),
                Some(processes) = process_rx.recv() => app.discovered_processes = processes,
                _ = tokio::signal::ctrl_c() => app.running = false,
            }
        }

        if let (Some(store), Some(run_id)) = (run_store.as_ref(), active_run_id.as_deref()) {
            let _ = store.complete_run(run_id, RunStatus::Completed);
        }

        Ok(())
    }
    .await;

    let restore_result = restore_terminal(&mut terminal);
    run_result?;
    restore_result?;

    Ok(())
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode().context("failed to enable raw mode")?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen)
        .context("failed to enter alternate screen")?;
    Terminal::new(CrosstermBackend::new(stdout)).context("failed to create terminal")
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode().context("failed to disable raw mode")?;
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)
        .context("failed to leave alternate screen")?;
    terminal.show_cursor().context("failed to show cursor")
}

fn setup_tracing() {
    let log_path = directories::ProjectDirs::from("", "", "epoch")
        .map(|dirs| {
            let cache_dir = dirs.cache_dir().to_path_buf();
            let _ = std::fs::create_dir_all(&cache_dir);
            cache_dir.join("epoch.log")
        })
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp/epoch.log"));

    if let Ok(file) = std::fs::File::create(&log_path) {
        let _ = tracing_subscriber::fmt()
            .with_writer(file)
            .with_ansi(false)
            .try_init();
    }
}

fn spawn_training_source(
    tx: mpsc::Sender<epoch::types::TrainingMetrics>,
    config: &epoch::config::Config,
) -> Result<tokio::task::JoinHandle<()>> {
    if config.stdin_mode {
        let parser = epoch::collectors::training::create_parser(config)?;
        Ok(epoch::collectors::training::spawn_stdin_reader(parser, tx))
    } else if let Some(ref path) = config.log_file {
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "trainer_state.json")
        {
            Ok(epoch::collectors::training::spawn_trainer_state_poller(
                path.clone(),
                tx,
                Duration::from_secs(2),
            ))
        } else {
            let parser = epoch::collectors::training::create_parser(config)?;
            epoch::collectors::training::spawn_file_watcher(path.clone(), parser, tx)
        }
    } else {
        Ok(tokio::spawn(async {}))
    }
}

async fn run_startup_mode(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut epoch::app::App,
    event_rx: &mut mpsc::Receiver<epoch::event::Event>,
) -> Result<()> {
    let inactivity_deadline = tokio::time::sleep(Duration::from_secs(1));
    tokio::pin!(inactivity_deadline);

    while app.running && !matches!(app.ui_state.mode, epoch::app::AppMode::Monitoring) {
        terminal.draw(|frame| epoch::ui::render(frame, app))?;

        if let epoch::app::AppMode::FilePicker(state) = &app.ui_state.mode
            && !state.filtered_indices.is_empty()
        {
            tokio::select! {
                Some(event) = event_rx.recv() => {
                    app.handle_event(event);
                    inactivity_deadline.as_mut().reset(tokio::time::Instant::now() + Duration::from_secs(1));
                }
                _ = &mut inactivity_deadline => {
                    if app.ui_state.selected_file.is_none()
                        && let Some(first) = state.filtered_indices.first().copied()
                    {
                        app.ui_state.selected_file = Some(state.files[first].path.clone());
                        app.ui_state.mode = epoch::app::AppMode::Monitoring;
                    }
                }
                _ = tokio::signal::ctrl_c() => app.running = false,
            }
            continue;
        }

        tokio::select! {
            Some(event) = event_rx.recv() => app.handle_event(event),
            _ = tokio::signal::ctrl_c() => app.running = false,
        }
    }

    Ok(())
}

async fn run_scanning_mode(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut epoch::app::App,
    event_rx: &mut mpsc::Receiver<epoch::event::Event>,
) -> Result<Vec<epoch::discovery::DiscoveredFile>> {
    let cwd = std::env::current_dir().context("failed to determine current directory")?;
    let discovery_task =
        tokio::task::spawn_blocking(move || epoch::discovery::discover_training_files(&cwd));
    tokio::pin!(discovery_task);

    while app.running && matches!(app.ui_state.mode, epoch::app::AppMode::Scanning) {
        terminal.draw(|frame| epoch::ui::render(frame, app))?;

        tokio::select! {
            result = &mut discovery_task => {
                let discovered = result.context("discovery task join failed")??;
                return Ok(discovered);
            }
            Some(event) = event_rx.recv() => app.handle_event(event),
            _ = tokio::signal::ctrl_c() => app.running = false,
        }
    }

    Ok(vec![])
}

fn spawn_gpu_collector(
    tx: mpsc::Sender<Vec<epoch::types::GpuMetrics>>,
    interval: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut collector = epoch::collectors::gpu::GpuCollector::new(tx);
        let mut ticker = tokio::time::interval(interval);

        loop {
            ticker.tick().await;
            if let Err(e) = collector.collect().await {
                tracing::debug!("gpu collect error: {e}");
            }
        }
    })
}

fn spawn_system_collector(
    tx: mpsc::Sender<epoch::types::SystemMetrics>,
    interval: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut sys = sysinfo::System::new();
        let mut initialized = false;
        let (gpu_tx, mut gpu_rx) = mpsc::channel::<Vec<epoch::types::GpuMetrics>>(4);
        let _gpu_handle = spawn_gpu_collector(gpu_tx, interval);
        let mut latest_gpus: Vec<epoch::types::GpuMetrics> = vec![];
        let mut ticker = tokio::time::interval(interval);

        loop {
            ticker.tick().await;

            if !initialized {
                sys.refresh_cpu_all();
                sys.refresh_memory();
                tokio::time::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL).await;
                initialized = true;
            }

            sys.refresh_cpu_all();
            sys.refresh_memory();

            while let Ok(gpus) = gpu_rx.try_recv() {
                latest_gpus = gpus;
            }

            let metrics = epoch::types::SystemMetrics {
                cpu_usage: sys.global_cpu_usage() as f64,
                memory_used: sys.used_memory(),
                memory_total: sys.total_memory(),
                gpus: latest_gpus.clone(),
            };

            if tx.send(metrics).await.is_err() {
                break;
            }
        }
    })
}

fn spawn_process_collector(
    tx: mpsc::Sender<Vec<epoch::collectors::process::ProcessCandidate>>,
    interval: Duration,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);

        loop {
            ticker.tick().await;

            let discovered = tokio::task::spawn_blocking(
                epoch::collectors::process::discover_training_like_processes,
            )
            .await
            .unwrap_or_default();

            if tx.send(discovered).await.is_err() {
                break;
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parse() {
        let cli = Cli::parse_from(["epoch"]);
        assert!(cli.log_file.is_none());
        assert!(!cli.stdin);
    }
}
