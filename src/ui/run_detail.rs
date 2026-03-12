use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::{App, DataHealthState, MonitoringRoute};
use crate::store::types::RunStatus;
use crate::ui::alerts_panel::{AlertPanelData, render_alert_panel};
use crate::ui::components::{
    centered_text_area, format_duration, format_epoch_date, format_lr_value, format_optional_float,
    format_step, trend_indicator,
};
use crate::ui::graph::MetricGraph;
use crate::ui::theme::resolve_palette_from_config;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunSurface<'a> {
    Primary,
    RunDetail {
        selected_run_id: Option<&'a str>,
        compare_run_id: Option<&'a str>,
    },
}

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let surface = match app.ui_state.monitoring.route {
        MonitoringRoute::RunDetail => RunSurface::RunDetail {
            selected_run_id: app.run_detail_selected_run_id(),
            compare_run_id: app.run_detail_compare_run_id(),
        },
        MonitoringRoute::Home => RunSurface::Primary,
    };

    render_for_surface(frame, area, app, surface);
}

pub fn render_for_surface(frame: &mut Frame, area: Rect, app: &App, surface: RunSurface<'_>) {
    let palette = resolve_palette_from_config(&app.config);
    let latest = app.training.latest.as_ref();

    if let RunSurface::RunDetail {
        selected_run_id: None,
        ..
    } = surface
    {
        render_empty_state(
            frame,
            area,
            &palette,
            "No run selected.\nSelect a run on Home and press Enter to open Run Detail.",
        );
        return;
    }

    let content_area = if let RunSurface::RunDetail {
        selected_run_id: Some(run_id),
        ..
    } = surface
    {
        let [context_area, content_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .areas(area);
        let run_name = app
            .selected_run_record()
            .and_then(|record| record.display_name.as_deref())
            .unwrap_or(run_id);
        let run_status = app
            .selected_run_record()
            .map(|record| record.status.as_str())
            .unwrap_or("unknown");
        let run_context = Paragraph::new(format!("Run Detail: {run_name} ({run_status})"))
            .alignment(Alignment::Left)
            .style(Style::default().fg(palette.muted));
        frame.render_widget(run_context, context_area);
        content_area
    } else {
        area
    };

    if matches!(surface, RunSurface::Primary) && app.training.latest.is_none() {
        render_empty_state(
            frame,
            content_area,
            &palette,
            "No training metrics received yet.\nStart a training run and pipe output via --stdin or --log-file",
        );
        return;
    }

    let focused = app.ui_state.focused_box;
    let historical_run_detail =
        matches!(surface, RunSurface::RunDetail { .. }) && !app.run_detail_accepts_live_updates();

    // Main layout: graphs on left (60%), panels on right (40%)
    let [graph_area, panel_area] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(30)])
        .areas(content_area);

    // Graphs layout: vertical stack
    let [loss_area, eval_area, lr_grad_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Min(0),
            Constraint::Percentage(40),
        ])
        .areas(graph_area);

    // LR and Grad Norm side-by-side
    let [lr_area, grad_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .areas(lr_grad_area);

    // Render Loss graph (box 1)
    let loss_data = app.graph_viewport_series(
        0,
        &app.training.loss_history,
        loss_area.width.saturating_sub(2).max(1).into(),
    );
    let current_loss = app
        .training
        .latest
        .as_ref()
        .and_then(|m| m.loss)
        .unwrap_or(0.0);
    let loss_trend = trend_indicator(&app.training.loss_history);
    let loss_title = format!("Loss: {:.4} {}", current_loss, loss_trend);

    MetricGraph::new(&loss_title, &loss_data, palette.loss_color)
        .graph_mode(&app.config.graph_mode)
        .focused(focused == 1)
        .focus_index(Some(1))
        .empty_message("No loss data")
        .palette(palette.accent, palette.muted, palette.header_fg)
        .render(frame, loss_area);

    // Render Eval Loss graph (box 2)
    let eval_data = app.graph_viewport_series(
        1,
        &app.training.eval_loss_history,
        eval_area.width.saturating_sub(2).max(1).into(),
    );
    let current_eval = latest.and_then(|m| m.eval_loss);
    let eval_title = format!("Eval Loss: {}", format_optional_float(current_eval, 4));
    MetricGraph::new(&eval_title, &eval_data, palette.loss_color)
        .graph_mode(&app.config.graph_mode)
        .focused(focused == 2)
        .focus_index(Some(2))
        .empty_message("No eval loss data")
        .palette(palette.accent, palette.muted, palette.header_fg)
        .render(frame, eval_area);

    // Render LR graph (box 3)
    let lr_data = app.graph_viewport_series(
        2,
        &app.training.lr_history,
        lr_area.width.saturating_sub(2).max(1).into(),
    );
    let current_lr = app
        .training
        .latest
        .as_ref()
        .and_then(|m| m.learning_rate)
        .unwrap_or(0.0);
    let lr_title = format!("Learning Rate: {}", format_lr_value(current_lr));

    MetricGraph::new(&lr_title, &lr_data, palette.lr_color)
        .graph_mode(&app.config.graph_mode)
        .focused(focused == 3)
        .focus_index(Some(3))
        .empty_message("No LR data")
        .palette(palette.accent, palette.muted, palette.header_fg)
        .render(frame, lr_area);

    // Render Grad Norm graph (box 4)
    let grad_data = app.graph_viewport_series(
        3,
        &app.training.grad_norm_history,
        grad_area.width.saturating_sub(2).max(1).into(),
    );
    let current_grad = latest.and_then(|m| m.grad_norm);
    let grad_title = format!("Grad Norm: {}", format_optional_float(current_grad, 3));
    MetricGraph::new(&grad_title, &grad_data, palette.lr_color)
        .graph_mode(&app.config.graph_mode)
        .focused(focused == 4)
        .focus_index(Some(4))
        .empty_message("No grad norm data")
        .palette(palette.accent, palette.muted, palette.header_fg)
        .render(frame, grad_area);

    // Panels layout: stability, core, signals, alerts stacked
    let [stability_area, core_area, signals_area, alerts_area] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Min(0),
        ])
        .areas(panel_area);

    render_stability_sidebar(frame, stability_area, app, &palette, historical_run_detail);
    render_core_panel(frame, core_area, app, &palette);
    render_signals_panel(frame, signals_area, app, &palette);
    let (active_alerts, resolved_alerts) = app.run_detail_alert_records();
    let alert_data = AlertPanelData::from_records(&active_alerts, &resolved_alerts);
    render_alert_panel(
        frame,
        alerts_area,
        &alert_data,
        &palette,
        "Alerts",
        false,
        6,
        6,
    );
}

fn render_empty_state(
    frame: &mut Frame,
    area: Rect,
    palette: &crate::ui::theme::ThemePalette,
    text: &str,
) {
    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Center)
        .style(Style::default().fg(palette.muted));
    frame.render_widget(paragraph, centered_text_area(area, text));
}

fn render_stability_sidebar(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    palette: &crate::ui::theme::ThemePalette,
    historical_run_detail: bool,
) {
    let border_color = palette.muted;

    let (system_line, alert_line, parser_line) = if historical_run_detail {
        let selected = app.selected_run_record();
        let status = selected
            .map(|record| record.status.as_str().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let started = selected
            .map(|record| format_epoch_date(record.started_at_epoch_secs))
            .unwrap_or_else(|| "-".to_string());
        let ended = selected
            .and_then(|record| record.ended_at_epoch_secs)
            .map(format_epoch_date)
            .unwrap_or_else(|| "ongoing".to_string());
        let source = selected
            .and_then(|record| record.source_locator.as_deref())
            .unwrap_or("-")
            .to_string();
        let project = selected
            .and_then(|record| record.project_root.as_deref())
            .unwrap_or("-")
            .to_string();

        (
            format!("Run: status {status} | started {started} | ended {ended}"),
            format!("Source: {source} | Project: {project}"),
            "Parser: snapshot mode".to_string(),
        )
    } else {
        let system_line = if let Some(system) = app.system.latest.as_ref() {
            let mem_pct = system.memory_usage_percent();
            let gpu_pct = if system.gpus.is_empty() {
                "n/a".to_string()
            } else {
                format!(
                    "{:.1}%",
                    system.gpus.iter().map(|g| g.utilization).sum::<f64>()
                        / system.gpus.len() as f64
                )
            };
            format!(
                "CPU/RAM/GPU: {:.1}% / {:.1}% / {gpu_pct}",
                system.cpu_usage, mem_pct
            )
        } else {
            "CPU/RAM/GPU: n/a".to_string()
        };

        let alert_line = if app.alerts.active.is_empty() && app.alerts.resolved.is_empty() {
            "Alerts: none".to_string()
        } else {
            format!(
                "Alerts active/resolved: {}/{}",
                app.alerts.active.len(),
                app.alerts.resolved.len()
            )
        };

        let parser_line = format!(
            "Parser ok/skip/err: {}/{}/{}",
            app.training.parser_success_count,
            app.training.parser_skipped_count,
            app.training.parser_error_count,
        );

        (system_line, alert_line, parser_line)
    };

    let text = format!(
        "Perplexity: {}\nLoss spikes: {}\nNaN/Inf count: {}\n{}\n{}\n{}\n\nLast spike: {}\nLast NaN/Inf: {}\nHealth: {}",
        latest_or_dash(app.training.perplexity_latest, 3),
        app.training.loss_spike_count,
        app.training.nan_inf_count,
        parser_line,
        system_line,
        alert_line,
        anomaly_age(app.training.last_loss_spike_at),
        anomaly_age(app.training.last_nan_inf_at),
        app.training_data_health_state().label(),
    );

    let block = Block::default()
        .title("Stability Summary")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Left)
        .block(block)
        .style(Style::default().fg(palette.accent));
    frame.render_widget(paragraph, area);
}

fn render_core_panel(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    palette: &crate::ui::theme::ThemePalette,
) {
    let border_color = palette.muted;

    let latest = app.training.latest.as_ref();

    let time_text = app
        .selected_run_elapsed()
        .map(format_duration)
        .unwrap_or_else(|| "Idle".to_string());

    let (status_text, status_color) =
        if matches!(app.ui_state.monitoring.route, MonitoringRoute::RunDetail) {
            match app
                .selected_run_record()
                .map(|record| record.status.clone())
            {
                Some(RunStatus::Active) => ("active", palette.success),
                Some(RunStatus::Completed) => ("completed", palette.accent),
                Some(RunStatus::Failed) => ("failed", palette.error),
                None => ("unknown", palette.muted),
            }
        } else {
            match app.training_data_health_state() {
                DataHealthState::Live => (DataHealthState::Live.label(), palette.success),
                DataHealthState::Stale => (DataHealthState::Stale.label(), palette.warning),
                DataHealthState::NoData => (DataHealthState::NoData.label(), palette.muted),
            }
        };

    let current_step = app.current_run_step().unwrap_or(0);
    let step_text = if app.training.total_steps > 0 && latest.is_some_and(|m| m.step.is_some()) {
        format!(
            "Step: {} / {}",
            format_step(current_step),
            format_step(app.training.total_steps)
        )
    } else {
        format!("Step: {}", format_step(current_step))
    };

    let throughput = latest.and_then(|m| m.throughput);
    let text = format!(
        "{step_text}\nThroughput: {}\nRunning: {time_text}\nStatus: {status_text}",
        format_optional_float(throughput, 1),
    );

    let block = Block::default()
        .title("Core")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(status_color))
        .block(block);
    frame.render_widget(paragraph, area);
}

fn render_signals_panel(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    palette: &crate::ui::theme::ThemePalette,
) {
    let border_color = palette.muted;

    let latest = app.training.latest.as_ref();

    let rate_items = [
        (
            "tokens_per_second",
            "Tokens/s",
            format_optional_float(latest.and_then(|m| m.tokens_per_second), 1),
            latest.is_some_and(|m| m.tokens_per_second.is_some()),
        ),
        (
            "samples_per_second",
            "Samples/s",
            format_optional_float(latest.and_then(|m| m.samples_per_second), 1),
            latest.is_some_and(|m| m.samples_per_second.is_some()),
        ),
        (
            "steps_per_second",
            "Steps/s",
            format_optional_float(latest.and_then(|m| m.steps_per_second), 3),
            latest.is_some_and(|m| m.steps_per_second.is_some()),
        ),
    ];
    let rates_line = rate_items
        .iter()
        .filter(|(id, _, _, present)| app.should_show_metric_panel(id, *present))
        .map(|(_, label, value, _)| format!("{label}: {value}"))
        .collect::<Vec<_>>();
    let rates_text = if rates_line.is_empty() {
        "Rates: —".to_string()
    } else {
        rates_line.join(" | ")
    };

    let mut lines = vec![format!(
        "{}\nTokens: {} | Eval: {} | Grad: {}\nSpikes: {} | NaN/Inf: {}",
        rates_text,
        latest
            .and_then(|m| m.tokens)
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".to_string()),
        format_optional_float(latest.and_then(|m| m.eval_loss), 4),
        format_optional_float(latest.and_then(|m| m.grad_norm), 3),
        app.training.loss_spike_count,
        app.training.nan_inf_count
    )];

    if app.run_comparison.snapshot_mode {
        let loss_delta = app
            .run_compare_latest_loss_delta()
            .map(|v| format!("{v:+.4}"))
            .unwrap_or_else(|| "n/a".to_string());
        let lr_delta = app
            .run_compare_latest_lr_delta()
            .map(|v| format!("{v:+.2e}"))
            .unwrap_or_else(|| "n/a".to_string());
        lines.push(format!("Compare Loss Δ: {loss_delta} | LR Δ: {lr_delta}"));
    }

    let block = Block::default()
        .title("Signals")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let paragraph = Paragraph::new(lines.join("\n"))
        .style(
            Style::default()
                .fg(palette.muted)
                .add_modifier(Modifier::BOLD),
        )
        .block(block);
    frame.render_widget(paragraph, area);
}

fn latest_or_dash(value: Option<f64>, decimals: usize) -> String {
    match value {
        Some(v) => format!("{v:.decimals$}"),
        None => "-".to_string(),
    }
}

fn anomaly_age(ts: Option<std::time::Instant>) -> String {
    match ts {
        Some(t) => format!("{}s ago", t.elapsed().as_secs()),
        None => "never".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::MonitoringRoute;
    use crate::config::Config;
    use crate::types::TrainingMetrics;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn test_live_empty_state() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.ui_state.monitoring.route = MonitoringRoute::Home;
        terminal.draw(|f| render(f, f.area(), &app)).unwrap();

        let buffer = terminal.backend().buffer();
        let content = buffer_to_string(buffer);
        assert!(content.contains("No training metrics"));
    }

    #[test]
    fn test_run_detail_requires_selected_run() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.ui_state.monitoring.route = MonitoringRoute::RunDetail;

        terminal.draw(|f| render(f, f.area(), &app)).unwrap();
        let buffer = terminal.backend().buffer();
        let content = buffer_to_string(buffer);

        assert!(content.contains("No run selected"));
    }

    #[test]
    fn test_run_detail_with_selected_historical_run_does_not_require_live_input() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.ui_state.monitoring.route = MonitoringRoute::RunDetail;
        app.ui_state.monitoring.run_detail.selected_run_id = Some("run-42".to_string());

        terminal.draw(|f| render(f, f.area(), &app)).unwrap();
        let content = buffer_to_string(terminal.backend().buffer());

        assert!(content.contains("Run Detail: run-42"));
        assert!(!content.contains("No training metrics received yet"));
    }

    #[test]
    fn test_live_with_data() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.ui_state.monitoring.run_detail.selected_run_id = Some("run-1".to_string());
        app.push_metrics(TrainingMetrics {
            loss: Some(0.5),
            learning_rate: Some(1e-4),
            step: Some(100),
            throughput: Some(1200.0),
            eval_loss: Some(0.8),
            grad_norm: Some(1.5),
            tokens_per_second: Some(1400.0),
            samples_per_second: Some(20.0),
            steps_per_second: Some(0.4),
            ..TrainingMetrics::default()
        });

        terminal.draw(|f| render(f, f.area(), &app)).unwrap();
        let buffer = terminal.backend().buffer();
        let content = buffer_to_string(buffer);

        assert!(content.contains("Loss:"));
        assert!(content.contains("Eval Loss"));
        assert!(content.contains("Learning Rate:"));
        assert!(content.contains("Grad Norm"));
        assert!(content.contains("Stability Summary"));
        assert!(content.contains("Core"));
        assert!(content.contains("Signals"));
    }

    #[test]
    fn test_run_detail_uses_selected_run_alert_history_for_finished_run() {
        use crate::store::repository::RunStore;
        use crate::store::types::{RunMetadata, RunRecord, RunSourceKind, RunStatus};

        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.set_store(RunStore::open_in_memory().expect("store should open"));

        let run_id = app
            .run_store
            .as_ref()
            .expect("store should exist")
            .attach_or_create_active_run(
                "fp-alert-history-live",
                RunSourceKind::LogFile,
                RunMetadata {
                    display_name: Some("finished-run".to_string()),
                    project_root: None,
                    command: None,
                    cwd: None,
                    git_commit: None,
                    git_dirty: None,
                    source_locator: Some("/tmp/finished.log".to_string()),
                },
            )
            .expect("attach should work")
            .run_id;

        app.run_store
            .as_ref()
            .expect("store should exist")
            .complete_run(&run_id, RunStatus::Completed)
            .expect("complete should work");
        app.run_store
            .as_ref()
            .expect("store should exist")
            .add_event(
                &run_id,
                "alert.warning",
                Some("stored warning"),
                false,
                crate::store::types::now_epoch_secs(),
                Some(20),
            )
            .expect("event insert should work");

        app.alerts.active.push(crate::app::AlertRecord {
            rule_id: "global".to_string(),
            level: crate::app::AlertLevel::Critical,
            value: 1.0,
            message: "global alert should be hidden".to_string(),
            tick: 1,
        });

        app.ui_state.monitoring.route = MonitoringRoute::RunDetail;
        app.ui_state.monitoring.run_detail.selected_run_id = Some(run_id.clone());
        app.ui_state.explorer.records = vec![RunRecord {
            run_id: run_id.clone(),
            source_fingerprint: "fp-alert-history-live".to_string(),
            source_kind: RunSourceKind::LogFile,
            source_locator: Some("/tmp/finished.log".to_string()),
            project_root: None,
            display_name: Some("finished-run".to_string()),
            status: RunStatus::Completed,
            command: None,
            cwd: None,
            git_commit: None,
            git_dirty: None,
            started_at_epoch_secs: 1,
            ended_at_epoch_secs: Some(2),
            last_step: Some(30),
            last_updated_epoch_secs: 2,
        }];

        terminal.draw(|f| render(f, f.area(), &app)).unwrap();
        let content = buffer_to_string(terminal.backend().buffer());

        assert!(
            content.contains("stored") || content.contains("WARN"),
            "expected stored run alert to render, got: {content}"
        );
        assert!(!content.contains("global alert should be hidden"));
        assert!(!content.contains("No alerts"));
    }

    #[test]
    fn test_live_focus_box_renders() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.push_metrics(TrainingMetrics {
            loss: Some(0.5),
            ..TrainingMetrics::default()
        });
        app.ui_state.focused_box = 1;

        terminal.draw(|f| render(f, f.area(), &app)).unwrap();
    }

    #[test]
    fn test_live_graph_mode_line() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.config.graph_mode = "line".to_string();
        for i in 0..20 {
            app.push_metrics(TrainingMetrics {
                loss: Some(1.0 - (i as f64 * 0.04)),
                learning_rate: Some(1e-4),
                step: Some(i * 100),
                ..TrainingMetrics::default()
            });
        }

        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            terminal.draw(|f| render(f, f.area(), &app)).unwrap();
        }));
        assert!(res.is_ok());
    }

    fn buffer_to_string(buffer: &ratatui::buffer::Buffer) -> String {
        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer.cell((x, y)).unwrap().symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("\n")
    }
}
