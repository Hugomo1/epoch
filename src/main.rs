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

    let mut config = epoch::config::Config::load()?;
    config.merge_cli_args(cli.log_file, cli.stdin, cli.parser);

    setup_tracing();

    let mut terminal = setup_terminal().context("failed to setup terminal")?;
    let mut app = epoch::app::App::new(config.clone());

    let (event_tx, mut event_rx) = mpsc::channel(epoch::event::EVENT_CHANNEL_CAPACITY);
    let (metrics_tx, mut metrics_rx) = mpsc::channel(epoch::event::METRICS_CHANNEL_CAPACITY);
    let (system_tx, mut system_rx) = mpsc::channel(epoch::event::SYSTEM_CHANNEL_CAPACITY);

    let _event_handle = epoch::event::spawn_event_reader(event_tx.clone());
    let _tick_handle =
        epoch::event::spawn_tick(event_tx, Duration::from_millis(config.tick_rate_ms));
    let _training_handle = spawn_training_source(metrics_tx, &config)?;
    let _system_handle =
        spawn_system_collector(system_tx, Duration::from_millis(config.tick_rate_ms));

    let run_result: Result<()> = async {
        while app.running {
            terminal.draw(|frame| epoch::ui::render(frame, &app))?;

            tokio::select! {
                Some(event) = event_rx.recv() => app.handle_event(event),
                Some(metrics) = metrics_rx.recv() => app.push_metrics(metrics),
                Some(system) = system_rx.recv() => app.push_system(system),
                _ = tokio::signal::ctrl_c() => app.running = false,
            }
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
    let parser = epoch::collectors::training::create_parser(config)?;

    if config.stdin_mode {
        Ok(epoch::collectors::training::spawn_stdin_reader(parser, tx))
    } else if let Some(ref path) = config.log_file {
        epoch::collectors::training::spawn_file_watcher(path.clone(), parser, tx)
    } else {
        Ok(tokio::spawn(async {}))
    }
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
