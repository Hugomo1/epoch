use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, LineGauge, Paragraph};

use crate::app::App;
use crate::ui::graph::MetricGraph;
use crate::ui::theme::resolve_palette_from_config;

fn format_loss(v: f64) -> String {
    if v < 1.0 {
        format!("{:.4}", v)
    } else {
        format!("{:.2}", v)
    }
}

fn format_lr(v: f64) -> String {
    format!("{:.1e}", v)
}

fn format_throughput(v: f64) -> String {
    let mut num = v.trunc() as u64;
    if num == 0 {
        return "0 tok/s".to_string();
    }
    let mut res = String::new();
    while num > 0 {
        let rem = num % 1000;
        num /= 1000;
        if num > 0 {
            res = format!(",{:03}{}", rem, res);
        } else {
            res = format!("{}{}", rem, res);
        }
    }
    format!("{} tok/s", res)
}

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let palette = resolve_palette_from_config(&app.config);

    if app.training.latest.is_none() {
        let msg = Paragraph::new("Waiting for training data...")
            .alignment(Alignment::Center)
            .style(Style::default().fg(palette.muted));
        let layout = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .split(area);
        frame.render_widget(msg, layout[1]);
        return;
    }

    let main_chunks = Layout::vertical([
        Constraint::Min(0),
        Constraint::Length(8),
        Constraint::Length(3),
    ])
    .split(area);

    let history_width = usize::from(main_chunks[0].width.saturating_sub(2).max(1));
    let history_vec = app.graph_viewport_series(0, &app.training.loss_history, history_width);

    MetricGraph::new("Loss", &history_vec, palette.loss_color)
        .graph_mode(&app.config.graph_mode)
        .empty_message("No loss data")
        .palette(palette.accent, palette.muted, palette.header_fg)
        .render(frame, main_chunks[0]);

    let Some(latest) = app.training.latest.as_ref() else {
        return;
    };
    let loss_str = latest
        .loss
        .map(format_loss)
        .unwrap_or_else(|| "N/A".to_string());
    let step_str = latest
        .step
        .map(|s| s.to_string())
        .unwrap_or_else(|| "N/A".to_string());
    let lr_str = latest
        .learning_rate
        .map(format_lr)
        .unwrap_or_else(|| "N/A".to_string());
    let tp_str = latest
        .throughput
        .map(format_throughput)
        .unwrap_or_else(|| "N/A".to_string());
    let tokens_str = latest
        .tokens
        .map(|t| t.to_string())
        .unwrap_or_else(|| "N/A".to_string());
    let health_str = app.training_data_health_state().label();

    let mut stats_lines = vec![
        format!("Latest Loss: {loss_str}"),
        format!("Step Count: {step_str}"),
        format!("Learning Rate: {lr_str}"),
    ];
    if app.should_show_metric_panel("throughput", latest.throughput.is_some()) {
        stats_lines.push(format!("Throughput: {tp_str}"));
    }
    if app.should_show_metric_panel("tokens", latest.tokens.is_some()) {
        stats_lines.push(format!("Tokens: {tokens_str}"));
    }
    stats_lines.push(format!("Health: {health_str}"));

    let stats_text = stats_lines.join("\n");
    let stats_widget =
        Paragraph::new(stats_text).block(Block::default().title("Key Stats").borders(Borders::ALL));
    frame.render_widget(stats_widget, main_chunks[1]);

    if let Some(sys) = &app.system.latest {
        let bottom_chunks = Layout::horizontal([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(main_chunks[2]);

        let cpu_ratio = (sys.cpu_usage / 100.0).clamp(0.0, 1.0);
        let cpu_gauge = LineGauge::default()
            .block(Block::default().title("CPU").borders(Borders::ALL))
            .filled_style(Style::default().fg(palette.cpu_color))
            .ratio(cpu_ratio);
        frame.render_widget(cpu_gauge, bottom_chunks[0]);

        let ram_ratio = if sys.memory_total > 0 {
            (sys.memory_used as f64 / sys.memory_total as f64).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let ram_gauge = LineGauge::default()
            .block(Block::default().title("RAM").borders(Borders::ALL))
            .filled_style(Style::default().fg(palette.ram_color))
            .ratio(ram_ratio);
        frame.render_widget(ram_gauge, bottom_chunks[1]);

        if let Some(gpu) = sys.gpus.first() {
            let gpu_ratio = (gpu.utilization / 100.0).clamp(0.0, 1.0);
            let gpu_gauge = LineGauge::default()
                .block(Block::default().title("GPU").borders(Borders::ALL))
                .filled_style(Style::default().fg(palette.gpu_color))
                .ratio(gpu_ratio);
            frame.render_widget(gpu_gauge, bottom_chunks[2]);
        } else {
            let no_gpu = Paragraph::new("N/A")
                .alignment(Alignment::Center)
                .block(Block::default().title("GPU").borders(Borders::ALL));
            frame.render_widget(no_gpu, bottom_chunks[2]);
        }
    } else {
        let msg = Paragraph::new("Collecting system metrics...")
            .alignment(Alignment::Center)
            .style(Style::default().fg(palette.muted));
        let layout = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .split(main_chunks[2]);
        frame.render_widget(msg, layout[1]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::types::{GpuMetrics, SystemMetrics, TrainingMetrics};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn test_dashboard_empty_state() {
        let app = App::new(Config::default());
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render(f, f.area(), &app)).unwrap();
        let buffer = terminal.backend().buffer();

        let mut found = false;
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                if buffer.cell((x, y)).unwrap().symbol() == "W"
                    && buffer.cell((x + 1, y)).unwrap().symbol() == "a"
                    && buffer.cell((x + 2, y)).unwrap().symbol() == "i"
                    && buffer.cell((x + 3, y)).unwrap().symbol() == "t"
                {
                    found = true;
                    break;
                }
            }
        }
        assert!(found, "Should show Waiting message");
    }

    #[test]
    fn test_dashboard_with_training_data() {
        let mut app = App::new(Config::default());
        app.push_metrics(TrainingMetrics {
            loss: Some(0.5),
            learning_rate: Some(1e-4),
            step: Some(100),
            throughput: Some(1234.0),
            ..Default::default()
        });
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render(f, f.area(), &app)).unwrap();
        let buffer = terminal.backend().buffer();

        let mut found = false;
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                if buffer.cell((x, y)).unwrap().symbol() == "L"
                    && buffer.cell((x + 1, y)).unwrap().symbol() == "o"
                    && buffer.cell((x + 2, y)).unwrap().symbol() == "s"
                    && buffer.cell((x + 3, y)).unwrap().symbol() == "s"
                {
                    found = true;
                    break;
                }
            }
        }
        assert!(found, "Should render Loss block");
    }

    #[test]
    fn test_dashboard_with_system_data() {
        let mut app = App::new(Config::default());
        app.push_metrics(TrainingMetrics {
            loss: Some(0.5),
            ..Default::default()
        });
        app.push_system(SystemMetrics {
            cpu_usage: 45.0,
            memory_used: 8_589_934_592,
            memory_total: 17_179_869_184,
            gpus: vec![GpuMetrics {
                name: "RTX 4090".into(),
                utilization: 95.0,
                memory_used: 20_000_000_000,
                memory_total: 24_000_000_000,
                temperature: 72.0,
            }],
        });

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render(f, f.area(), &app)).unwrap();
    }

    #[test]
    fn test_dashboard_no_gpu() {
        let mut app = App::new(Config::default());
        app.push_metrics(TrainingMetrics {
            loss: Some(0.5),
            ..Default::default()
        });
        app.push_system(SystemMetrics {
            cpu_usage: 45.0,
            memory_used: 8_589_934_592,
            memory_total: 17_179_869_184,
            gpus: vec![],
        });

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render(f, f.area(), &app)).unwrap();
        let buffer = terminal.backend().buffer();
        let mut found = false;
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                if buffer.cell((x, y)).unwrap().symbol() == "N"
                    && buffer.cell((x + 1, y)).unwrap().symbol() == "/"
                    && buffer.cell((x + 2, y)).unwrap().symbol() == "A"
                {
                    found = true;
                    break;
                }
            }
        }
        assert!(found, "Should show N/A for missing GPU");
    }

    #[test]
    fn test_format_loss_small() {
        assert_eq!(format_loss(0.0001234), "0.0001");
    }

    #[test]
    fn test_format_loss_large() {
        assert_eq!(format_loss(2.345), "2.35");
    }

    #[test]
    fn test_format_lr() {
        assert!(format_lr(0.0001).contains("e"));
        assert_eq!(format_lr(0.0001), "1.0e-4");
    }

    #[test]
    fn test_format_throughput() {
        let result = format_throughput(1234567.0);
        assert_eq!(result, "1,234,567 tok/s");
    }

    #[test]
    fn test_dashboard_remains_compact_with_new_core_metrics() {
        let mut app = App::new(Config::default());
        app.push_metrics(TrainingMetrics {
            loss: Some(0.5),
            learning_rate: Some(1e-4),
            step: Some(100),
            throughput: Some(1234.0),
            tokens: Some(42_000),
            ..Default::default()
        });

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
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

        assert!(content.contains("Tokens: 42000"));
        assert!(content.contains("Health: Live"));
    }

    #[test]
    fn test_dashboard_graph_mode_switch_between_line_and_sparkline() {
        let mut app = App::new(Config::default());
        app.push_metrics(TrainingMetrics {
            loss: Some(0.5),
            learning_rate: Some(1e-4),
            step: Some(100),
            throughput: Some(1234.0),
            ..Default::default()
        });

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        app.config.graph_mode = "sparkline".to_string();
        terminal.draw(|f| render(f, f.area(), &app)).unwrap();

        app.config.graph_mode = "line".to_string();
        terminal.draw(|f| render(f, f.area(), &app)).unwrap();
    }
}
