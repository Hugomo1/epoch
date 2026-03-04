use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, LineGauge, Paragraph, Sparkline};

use crate::app::App;
use crate::ui::{CPU_COLOR, GPU_COLOR, LOSS_COLOR, MUTED, RAM_COLOR};

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
    if app.training.latest.is_none() {
        let msg = Paragraph::new("Waiting for training data...")
            .alignment(Alignment::Center)
            .style(Style::default().fg(MUTED));
        let layout = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .split(area);
        frame.render_widget(msg, layout[1]);
        return;
    }

    let main_chunks =
        Layout::vertical([Constraint::Percentage(60), Constraint::Percentage(40)]).split(area);

    let top_chunks = Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(main_chunks[0]);

    let history_slice = app.training.loss_history.as_slices();
    let mut history_vec = Vec::with_capacity(app.training.loss_history.len());
    history_vec.extend_from_slice(history_slice.0);
    history_vec.extend_from_slice(history_slice.1);

    let sparkline = Sparkline::default()
        .block(Block::default().title("Loss").borders(Borders::ALL))
        .data(&history_vec)
        .style(Style::default().fg(LOSS_COLOR));
    frame.render_widget(sparkline, top_chunks[0]);

    let latest = app.training.latest.as_ref().unwrap();
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

    let stats_text = format!(
        "Latest Loss: {}\nStep Count: {}\nLearning Rate: {}\nThroughput: {}",
        loss_str, step_str, lr_str, tp_str
    );
    let stats_widget =
        Paragraph::new(stats_text).block(Block::default().title("Key Stats").borders(Borders::ALL));
    frame.render_widget(stats_widget, top_chunks[1]);

    if let Some(sys) = &app.system.latest {
        let bottom_chunks = Layout::horizontal([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(main_chunks[1]);

        let cpu_ratio = (sys.cpu_usage / 100.0).clamp(0.0, 1.0);
        let cpu_gauge = LineGauge::default()
            .block(Block::default().title("CPU").borders(Borders::ALL))
            .filled_style(Style::default().fg(CPU_COLOR))
            .ratio(cpu_ratio);
        frame.render_widget(cpu_gauge, bottom_chunks[0]);

        let ram_ratio = if sys.memory_total > 0 {
            (sys.memory_used as f64 / sys.memory_total as f64).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let ram_gauge = LineGauge::default()
            .block(Block::default().title("RAM").borders(Borders::ALL))
            .filled_style(Style::default().fg(RAM_COLOR))
            .ratio(ram_ratio);
        frame.render_widget(ram_gauge, bottom_chunks[1]);

        if let Some(gpu) = sys.gpus.first() {
            let gpu_ratio = (gpu.utilization / 100.0).clamp(0.0, 1.0);
            let gpu_gauge = LineGauge::default()
                .block(Block::default().title("GPU").borders(Borders::ALL))
                .filled_style(Style::default().fg(GPU_COLOR))
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
            .style(Style::default().fg(MUTED));
        let layout = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .split(main_chunks[1]);
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
}
