use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Paragraph, Sparkline};

use crate::app::App;
use crate::ui::graph::render_line_graph;
use crate::ui::theme::resolve_palette_from_config;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let palette = resolve_palette_from_config(&app.config);

    let Some(latest) = app.training.latest.as_ref() else {
        let text = Paragraph::new("No diagnostics available yet")
            .alignment(Alignment::Center)
            .style(Style::default().fg(palette.muted));
        frame.render_widget(text, area);
        return;
    };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(5)])
        .split(area);

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(36)])
        .split(rows[0]);

    let graphs = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(6)])
        .split(top[0]);

    let eval_width = usize::from(graphs[0].width.saturating_sub(2).max(1));
    let eval_history = app.training_viewport_series(&app.training.eval_loss_history, eval_width);
    let eval_block = Block::default()
        .title("Eval Loss (source: validation, unit: loss)")
        .borders(Borders::ALL);
    if eval_history.is_empty() {
        frame.render_widget(
            Paragraph::new("No eval loss data")
                .block(eval_block)
                .style(Style::default().fg(palette.muted)),
            graphs[0],
        );
    } else if app.config.graph_mode == "line" {
        render_line_graph(
            frame,
            graphs[0],
            eval_block,
            "eval_loss",
            &eval_history,
            palette.loss_color,
        );
    } else {
        frame.render_widget(
            Sparkline::default()
                .block(eval_block)
                .data(&eval_history)
                .style(Style::default().fg(palette.loss_color)),
            graphs[0],
        );
    }

    let grad_width = usize::from(graphs[1].width.saturating_sub(2).max(1));
    let grad_history = app.training_viewport_series(&app.training.grad_norm_history, grad_width);
    let grad_block = Block::default()
        .title("Grad Norm (source: optimizer, unit: norm)")
        .borders(Borders::ALL);
    if grad_history.is_empty() {
        frame.render_widget(
            Paragraph::new("No grad norm data")
                .block(grad_block)
                .style(Style::default().fg(palette.muted)),
            graphs[1],
        );
    } else if app.config.graph_mode == "line" {
        render_line_graph(
            frame,
            graphs[1],
            grad_block,
            "grad_norm",
            &grad_history,
            palette.lr_color,
        );
    } else {
        frame.render_widget(
            Sparkline::default()
                .block(grad_block)
                .data(&grad_history)
                .style(Style::default().fg(palette.lr_color)),
            graphs[1],
        );
    }

    let system_line = if let Some(system) = app.system.latest.as_ref() {
        let mem_pct = system.memory_usage_percent();
        let gpu_pct = if system.gpus.is_empty() {
            "n/a".to_string()
        } else {
            format!(
                "{:.1}%",
                system.gpus.iter().map(|g| g.utilization).sum::<f64>() / system.gpus.len() as f64
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
        let active = app.alerts.active.len();
        let resolved = app.alerts.resolved.len();
        format!("Alerts active/resolved: {active}/{resolved}")
    };

    let summary = Paragraph::new(format!(
        "Perplexity: {}\nLoss spikes: {}\nNaN/Inf count: {}\nParser ok/skip/err: {}/{}/{}\n{}\n{}\n\nLast loss spike: {}\nLast NaN/Inf: {}\nParser mode: {}\nHealth: {}",
        latest_or_dash(app.training.perplexity_latest, 3),
        app.training.loss_spike_count,
        app.training.nan_inf_count,
        app.training.parser_success_count,
        app.training.parser_skipped_count,
        app.training.parser_error_count,
        system_line,
        alert_line,
        anomaly_age(app.training.last_loss_spike_at),
        anomaly_age(app.training.last_nan_inf_at),
        app.config.parser,
        app.training_data_health_state().label(),
    ))
    .alignment(Alignment::Left)
    .block(
        Block::default()
            .title("Stability Summary")
            .borders(Borders::ALL),
    )
    .style(Style::default().fg(palette.accent));
    frame.render_widget(summary, top[1]);

    let throughput_items = [
        (
            "tokens_per_second",
            "Tokens/s",
            "tok/s (throughput)",
            latest.tokens_per_second,
        ),
        (
            "samples_per_second",
            "Samples/s",
            "samples/s (dataloader)",
            latest.samples_per_second,
        ),
        (
            "steps_per_second",
            "Steps/s",
            "steps/s (optimizer)",
            latest.steps_per_second,
        ),
    ];

    let visible_items = throughput_items
        .iter()
        .filter(|(id, _, _, value)| app.should_show_metric_panel(id, value.is_some()))
        .collect::<Vec<_>>();

    if visible_items.is_empty() {
        let empty = Paragraph::new("No throughput metrics")
            .alignment(Alignment::Center)
            .block(Block::default().title("Throughput").borders(Borders::ALL))
            .style(Style::default().fg(palette.muted));
        frame.render_widget(empty, rows[1]);
    } else {
        let constraints =
            vec![Constraint::Ratio(1, visible_items.len() as u32); visible_items.len()];
        let throughput = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(rows[1]);

        for (idx, (_, label, unit, value)) in visible_items.iter().enumerate() {
            let panel = Paragraph::new(format!("{}\nunit: {unit}", latest_or_dash(*value, 3)))
                .alignment(Alignment::Center)
                .block(Block::default().title(*label).borders(Borders::ALL))
                .style(Style::default().fg(palette.accent));
            frame.render_widget(panel, throughput[idx]);
        }
    }
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
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use crate::config::Config;
    use crate::types::TrainingMetrics;

    #[test]
    fn test_advanced_chart_bounds_match_metrics_contract() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());

        for i in 0..80 {
            app.push_metrics(TrainingMetrics {
                eval_loss: Some((i % 7) as f64 * 0.3),
                grad_norm: Some((i % 5) as f64 * 0.7),
                loss: Some(0.5),
                step: Some(i),
                ..TrainingMetrics::default()
            });
        }

        app.config.graph_mode = "line".to_string();
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            terminal.draw(|f| render(f, f.area(), &app)).unwrap();
        }));
        assert!(res.is_ok());
    }

    #[test]
    fn test_diagnostics_shows_active_and_resolved_alerts() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());

        app.push_metrics(TrainingMetrics {
            loss: Some(0.5),
            step: Some(1),
            ..TrainingMetrics::default()
        });
        app.alerts.active.push(crate::app::AlertRecord {
            rule_id: "memory_pressure".to_string(),
            level: crate::app::AlertLevel::Warning,
            value: 80.0,
            message: "memory_pressure: warning at 80.000".to_string(),
            tick: 1,
        });
        app.alerts.resolved.push(crate::app::AlertRecord {
            rule_id: "throughput_drop".to_string(),
            level: crate::app::AlertLevel::Critical,
            value: 10.0,
            message: "resolved at 10.000".to_string(),
            tick: 2,
        });

        terminal.draw(|f| render(f, f.area(), &app)).unwrap();
        let buffer = terminal.backend().buffer();
        let content = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer.cell((x, y)).unwrap().symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("\n");

        assert!(content.contains("Alerts active/resolved: 1/1"));
    }
}
