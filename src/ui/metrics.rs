use std::collections::VecDeque;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph, Sparkline};

use crate::app::{App, DataHealthState};
use crate::ui::graph::render_line_graph;
use crate::ui::theme::resolve_palette_from_config;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let palette = resolve_palette_from_config(&app.config);

    if app.training.latest.is_none() {
        let text = "No training metrics received yet.\nStart a training run and pipe output via --stdin or --log-file";
        let paragraph = Paragraph::new(text)
            .alignment(Alignment::Center)
            .style(Style::default().fg(palette.muted));

        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Fill(1),
                Constraint::Length(2),
                Constraint::Fill(1),
            ])
            .split(area);

        frame.render_widget(paragraph, vertical[1]);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(8),
            Constraint::Length(6),
        ])
        .split(area);

    let Some(latest) = app.training.latest.as_ref() else {
        return;
    };

    let sparkline_width = usize::from(chunks[0].width.saturating_sub(2).max(1));
    let loss_history = app.graph_viewport_series(0, &app.training.loss_history, sparkline_width);
    let current_loss = latest.loss.unwrap_or(0.0);
    let loss_trend = trend_indicator(&app.training.loss_history);

    let loss_title = format!("Loss: {:.4} {}", current_loss, loss_trend);
    let loss_block = Block::default()
        .borders(Borders::ALL)
        .title(loss_title)
        .title_style(
            Style::default()
                .fg(palette.header_fg)
                .add_modifier(Modifier::BOLD),
        );

    if loss_history.is_empty() {
        let para = Paragraph::new("No loss data")
            .block(loss_block)
            .style(Style::default().fg(palette.muted));
        frame.render_widget(para, chunks[0]);
    } else if app.config.graph_mode == "line" {
        render_line_graph(
            frame,
            chunks[0],
            loss_block,
            "Loss",
            &loss_history,
            palette.loss_color,
        );
    } else {
        let sparkline = Sparkline::default()
            .block(loss_block)
            .data(&loss_history)
            .style(Style::default().fg(palette.loss_color));
        frame.render_widget(sparkline, chunks[0]);
    }

    let lr_sparkline_width = usize::from(chunks[1].width.saturating_sub(2).max(1));
    let lr_history = app.graph_viewport_series(2, &app.training.lr_history, lr_sparkline_width);
    let current_lr = latest.learning_rate.unwrap_or(0.0);

    let lr_title = format!("Learning Rate: {}", format_lr_value(current_lr));
    let lr_block = Block::default()
        .borders(Borders::ALL)
        .title(lr_title)
        .title_style(
            Style::default()
                .fg(palette.header_fg)
                .add_modifier(Modifier::BOLD),
        );

    if lr_history.is_empty() {
        let para = Paragraph::new("No LR data")
            .block(lr_block)
            .style(Style::default().fg(palette.muted));
        frame.render_widget(para, chunks[1]);
    } else if app.config.graph_mode == "line" {
        render_line_graph(
            frame,
            chunks[1],
            lr_block,
            "Learning Rate",
            &lr_history,
            palette.lr_color,
        );
    } else {
        let sparkline = Sparkline::default()
            .block(lr_block)
            .data(&lr_history)
            .style(Style::default().fg(palette.lr_color));
        frame.render_widget(sparkline, chunks[1]);
    }

    let summary_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[2]);

    let elapsed = app.elapsed();
    let time_text = if app.training.start_time.is_some() {
        let hours = elapsed.as_secs() / 3600;
        let mins = (elapsed.as_secs() % 3600) / 60;
        let secs = elapsed.as_secs() % 60;
        format!("Running: {:02}:{:02}:{:02}", hours, mins, secs)
    } else {
        "Idle".to_string()
    };

    let (status_text, status_color) = match app.training_data_health_state() {
        DataHealthState::Live => (DataHealthState::Live.label(), palette.success),
        DataHealthState::Stale => (DataHealthState::Stale.label(), palette.warning),
        DataHealthState::NoData => (DataHealthState::NoData.label(), palette.muted),
    };

    let current_step = latest.step.unwrap_or(0);
    let step_text = if app.training.total_steps > 0 && latest.step.is_some() {
        format!(
            "Step: {} / {}",
            format_step(current_step),
            format_step(app.training.total_steps)
        )
    } else {
        format!("Step: {}", format_step(current_step))
    };

    let core_text = format!(
        "{step_text}\nThroughput: {}\nRunning: {time_text}\nStatus: {status_text}",
        format_optional_float(latest.throughput, 1),
    );
    let core_block = Block::default().title("Core").borders(Borders::ALL);
    let core_para = Paragraph::new(core_text)
        .style(Style::default().fg(palette.accent))
        .block(core_block);

    let rate_items = [
        (
            "tokens_per_second",
            "Tokens/s",
            format_optional_float(latest.tokens_per_second, 1),
            latest.tokens_per_second.is_some(),
        ),
        (
            "samples_per_second",
            "Samples/s",
            format_optional_float(latest.samples_per_second, 1),
            latest.samples_per_second.is_some(),
        ),
        (
            "steps_per_second",
            "Steps/s",
            format_optional_float(latest.steps_per_second, 3),
            latest.steps_per_second.is_some(),
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

    let mut points_lines = vec![format!(
        "{}\nTokens: {} | Eval: {} | Grad: {}\nSpikes: {} | NaN/Inf: {}",
        rates_text,
        latest
            .tokens
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".to_string()),
        format_optional_float(latest.eval_loss, 4),
        format_optional_float(latest.grad_norm, 3),
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
        points_lines.push(format!(
            "Compare Loss Δ: {loss_delta} | Compare LR Δ: {lr_delta}"
        ));
    }

    let points_text = points_lines.join("\n");
    let points_block = Block::default().title("Signals").borders(Borders::ALL);
    let points_para = Paragraph::new(points_text)
        .style(
            Style::default()
                .fg(palette.muted)
                .add_modifier(Modifier::BOLD),
        )
        .block(points_block);

    frame.render_widget(
        core_para.style(Style::default().fg(status_color)),
        summary_chunks[0],
    );
    frame.render_widget(points_para, summary_chunks[1]);
}

fn trend_indicator(history: &VecDeque<u64>) -> &'static str {
    if history.len() < 2 {
        return "→";
    }

    let Some(&last) = history.back() else {
        return "→";
    };
    let last = last as f64;
    let count = (history.len() - 1).min(10);

    // Average of the preceding 'count' elements (excluding the very last one)
    let sum: u64 = history.iter().rev().skip(1).take(count).sum();
    let avg = sum as f64 / count as f64;

    if last > avg * 1.01 {
        "↑"
    } else if last < avg * 0.99 {
        "↓"
    } else {
        "→"
    }
}

fn format_step(step: u64) -> String {
    let s = step.to_string();
    let mut result = String::new();
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();

    for (i, c) in chars.into_iter().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result
}

fn format_lr_value(lr: f64) -> String {
    format!("{:.1e}", lr)
}

fn format_optional_float(value: Option<f64>, decimals: usize) -> String {
    match value {
        Some(v) => format!("{v:.decimals$}"),
        None => "—".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::time::Instant;

    use crate::config::Config;
    use crate::types::TrainingMetrics;

    #[test]
    fn test_metrics_empty_state() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new(Config::default());

        terminal.draw(|f| render(f, f.area(), &app)).unwrap();
        let buffer = terminal.backend().buffer();

        let mut found = false;
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                if buffer.cell((x, y)).unwrap().symbol() == "N"
                    && buffer.cell((x + 1, y)).unwrap().symbol() == "o"
                    && buffer.cell((x + 3, y)).unwrap().symbol() == "t"
                {
                    found = true;
                    break;
                }
            }
        }
        assert!(found);
    }

    #[test]
    fn test_metrics_with_data() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());

        for i in 0..20 {
            app.push_metrics(TrainingMetrics {
                loss: Some(1.0 - (i as f64 * 0.04)),
                learning_rate: Some(1e-4),
                step: Some(i * 100),
                throughput: Some(1234.0),
                tokens: Some(1000),
                eval_loss: None,
                grad_norm: None,
                samples_per_second: None,
                steps_per_second: None,
                tokens_per_second: None,
                timestamp: Instant::now(),
            });
        }

        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            terminal.draw(|f| render(f, f.area(), &app)).unwrap();
        }));
        assert!(res.is_ok(), "Render with full data panicked");
    }

    #[test]
    fn test_metrics_partial_data() {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());

        app.push_metrics(TrainingMetrics {
            loss: Some(0.5),
            learning_rate: None,
            step: None,
            throughput: None,
            tokens: None,
            eval_loss: None,
            grad_norm: None,
            samples_per_second: None,
            steps_per_second: None,
            tokens_per_second: None,
            timestamp: Instant::now(),
        });

        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            terminal.draw(|f| render(f, f.area(), &app)).unwrap();
        }));
        assert!(res.is_ok(), "Render with partial data panicked");
    }

    #[test]
    fn test_trend_indicator_decreasing() {
        let mut history = VecDeque::new();
        for &v in &[1000, 900, 800, 700, 600] {
            history.push_back(v);
        }
        assert_eq!(trend_indicator(&history), "↓");
    }

    #[test]
    fn test_trend_indicator_increasing() {
        let mut history = VecDeque::new();
        for &v in &[600, 700, 800, 900, 1000] {
            history.push_back(v);
        }
        assert_eq!(trend_indicator(&history), "↑");
    }

    #[test]
    fn test_trend_indicator_stable() {
        let mut history = VecDeque::new();
        for &v in &[500, 500, 500, 500, 500] {
            history.push_back(v);
        }
        assert_eq!(trend_indicator(&history), "→");
    }

    #[test]
    fn test_trend_indicator_insufficient() {
        let mut history = VecDeque::new();
        history.push_back(500);
        assert_eq!(trend_indicator(&history), "→");
    }

    #[test]
    fn test_format_step() {
        assert_eq!(format_step(1234567), "1,234,567");
        assert_eq!(format_step(1000), "1,000");
    }

    #[test]
    fn test_format_step_small() {
        assert_eq!(format_step(42), "42");
        assert_eq!(format_step(999), "999");
    }

    #[test]
    fn test_format_lr_value() {
        let lr = format_lr_value(0.0001);
        assert!(lr.contains("e"));
        assert_eq!(lr, "1.0e-4");
    }

    #[test]
    fn test_metrics_tab_renders_tokens_when_present() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());

        app.push_metrics(TrainingMetrics {
            loss: Some(0.42),
            tokens: Some(123_456),
            step: Some(50),
            ..TrainingMetrics::default()
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

        assert!(content.contains("Tokens: 123456"));
    }

    #[test]
    fn test_metrics_tab_renders_grad_norm_and_eval_loss() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());

        app.push_metrics(TrainingMetrics {
            loss: Some(0.42),
            eval_loss: Some(0.33),
            grad_norm: Some(1.75),
            tokens_per_second: Some(1500.0),
            samples_per_second: Some(25.0),
            steps_per_second: Some(0.5),
            ..TrainingMetrics::default()
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

        assert!(content.contains("Eval: 0.3300"));
        assert!(content.contains("Grad: 1.750"));
        assert!(content.contains("Tokens/s: 1500.0"));
        assert!(content.contains("Samples/s: 25.0"));
        assert!(content.contains("Steps/s: 0.500"));
    }

    #[test]
    fn test_graph_mode_switch_between_line_and_sparkline() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.push_metrics(TrainingMetrics {
            loss: Some(0.42),
            learning_rate: Some(1e-4),
            step: Some(12),
            ..TrainingMetrics::default()
        });

        app.config.graph_mode = "sparkline".to_string();
        let sparkline_render = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            terminal.draw(|f| render(f, f.area(), &app)).unwrap();
        }));
        assert!(sparkline_render.is_ok());

        app.config.graph_mode = "line".to_string();
        let line_render = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            terminal.draw(|f| render(f, f.area(), &app)).unwrap();
        }));
        assert!(line_render.is_ok());
    }

    #[test]
    fn test_metrics_chart_uses_shared_graph_contract() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());

        for i in 0..80 {
            app.push_metrics(TrainingMetrics {
                loss: Some((i % 10) as f64 * 0.1),
                learning_rate: Some(1e-4),
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
    fn test_run_compare_marks_non_comparable_metrics() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());

        app.push_metrics(TrainingMetrics {
            loss: Some(0.42),
            step: Some(1),
            ..TrainingMetrics::default()
        });
        app.set_run_comparison_snapshot(vec![TrainingMetrics {
            step: Some(1),
            ..TrainingMetrics::default()
        }]);

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

        assert!(content.contains("Compare Loss Δ: n/a") || content.contains("Compare LR Δ: n/a"));
    }
}
