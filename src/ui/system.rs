use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, LineGauge, Paragraph};

use crate::app::App;
use crate::ui::graph::MetricGraph;
use crate::ui::theme::resolve_palette_from_config;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let palette = resolve_palette_from_config(&app.config);

    let Some(system) = &app.system.latest else {
        let msg = Paragraph::new("Collecting system metrics...")
            .style(Style::default().fg(palette.muted))
            .alignment(Alignment::Center);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Fill(1),
                Constraint::Length(1),
                Constraint::Fill(1),
            ])
            .split(area);

        frame.render_widget(msg, layout[1]);
        return;
    };

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(4),
            Constraint::Length(6),
        ])
        .split(area);

    let gauges = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(layout[1]);

    let cpu_ratio = (system.cpu_usage / 100.0).clamp(0.0, 1.0);
    let cpu_color = if system.cpu_usage > 80.0 {
        palette.warning
    } else {
        palette.cpu_color
    };

    let cpu_gauge = LineGauge::default()
        .block(Block::default().title("CPU").borders(Borders::ALL))
        .filled_style(Style::default().fg(cpu_color))
        .filled_symbol("█")
        .unfilled_symbol(" ")
        .ratio(cpu_ratio)
        .label(format!("{:.1}%", system.cpu_usage));

    frame.render_widget(cpu_gauge, gauges[0]);

    let ram_ratio = (system.memory_used as f64 / system.memory_total.max(1) as f64).clamp(0.0, 1.0);
    let ram_gauge = LineGauge::default()
        .block(Block::default().title("RAM").borders(Borders::ALL))
        .filled_style(Style::default().fg(palette.ram_color))
        .filled_symbol("█")
        .unfilled_symbol(" ")
        .ratio(ram_ratio)
        .label(format!(
            "{} / {}",
            format_bytes(system.memory_used),
            format_bytes(system.memory_total)
        ));

    frame.render_widget(ram_gauge, gauges[1]);

    let gpu_ratio = if system.gpus.is_empty() {
        0.0
    } else {
        (system.gpus.iter().map(|g| g.utilization).sum::<f64>() / system.gpus.len() as f64 / 100.0)
            .clamp(0.0, 1.0)
    };
    let gpu_label = if system.gpus.is_empty() {
        "N/A".to_string()
    } else {
        format!("{:.1}% avg", gpu_ratio * 100.0)
    };
    let gpu_top = LineGauge::default()
        .block(
            Block::default()
                .title(if system.gpus.is_empty() {
                    "GPU: Not available"
                } else {
                    "GPU"
                })
                .borders(Borders::ALL),
        )
        .filled_style(Style::default().fg(palette.gpu_color))
        .filled_symbol("█")
        .unfilled_symbol(" ")
        .ratio(gpu_ratio)
        .label(gpu_label);
    frame.render_widget(gpu_top, gauges[2]);

    if system.gpus.is_empty() {
        let p = Paragraph::new("No GPU detected")
            .style(Style::default().fg(palette.muted))
            .block(Block::default().title("GPU Details").borders(Borders::ALL));
        frame.render_widget(p, layout[0]);
    } else {
        let max_visible = usize::from(layout[0].height.saturating_sub(2).max(1)).min(2);
        let visible_count = system.gpus.len().min(max_visible);
        let hidden_count = system.gpus.len().saturating_sub(visible_count);

        let avg_util =
            system.gpus.iter().map(|g| g.utilization).sum::<f64>() / system.gpus.len() as f64;

        let mut lines = Vec::with_capacity(visible_count + usize::from(hidden_count > 0));
        for (i, gpu) in system.gpus.iter().take(visible_count).enumerate() {
            let is_outlier = (gpu.utilization - avg_util).abs() >= 30.0;
            let marker = if is_outlier { " [OUTLIER]" } else { "" };
            lines.push(format!(
                "GPU {} {}{marker} | {:.1}% | {}/{} | {}",
                i,
                gpu.name,
                gpu.utilization,
                format_bytes(gpu.memory_used),
                format_bytes(gpu.memory_total),
                format_temp(gpu.temperature)
            ));
        }
        if hidden_count > 0 {
            lines.push(format!("+{} more GPUs", hidden_count));
        }

        let gpu_details = Paragraph::new(lines.join("\n"))
            .style(Style::default().fg(palette.header_fg))
            .block(Block::default().title("GPU Details").borders(Borders::ALL));
        frame.render_widget(gpu_details, layout[0]);
    }

    let history_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(layout[2]);

    let history_width = usize::from(history_layout[0].width.saturating_sub(2).max(1));
    let cpu_data = app.system_viewport_series(&app.system.cpu_history, history_width);
    let ram_data = app.system_viewport_series(&app.system.ram_history, history_width);

    MetricGraph::new("CPU History", &cpu_data, palette.cpu_color)
        .graph_mode(&app.config.graph_mode)
        .empty_message("No CPU data")
        .palette(palette.accent, palette.muted, palette.header_fg)
        .render(frame, history_layout[0]);

    MetricGraph::new("RAM History", &ram_data, palette.ram_color)
        .graph_mode(&app.config.graph_mode)
        .empty_message("No RAM data")
        .palette(palette.accent, palette.muted, palette.header_fg)
        .render(frame, history_layout[1]);
}

fn format_bytes(bytes: u64) -> String {
    let tb = 1_099_511_627_776;
    let gb = 1_073_741_824;
    let mb = 1_048_576;
    let kb = 1024;

    if bytes >= tb {
        format!("{:.1} TB", bytes as f64 / tb as f64)
    } else if bytes >= gb {
        format!("{:.1} GB", bytes as f64 / gb as f64)
    } else if bytes >= mb {
        format!("{:.1} MB", bytes as f64 / mb as f64)
    } else {
        format!("{:.1} KB", bytes as f64 / kb as f64)
    }
}

fn format_temp(celsius: f64) -> String {
    format!("{}°C", celsius as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::types::{GpuMetrics, SystemMetrics};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn format_percent(v: f64) -> String {
        format!("{:.1}%", v)
    }

    #[test]
    fn test_system_empty_state() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new(Config::default());

        terminal
            .draw(|frame| {
                render(frame, frame.area(), &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer.cell((x, y)).unwrap().symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("\n");
        assert!(content.contains("Collecting system metrics..."));
    }

    #[test]
    fn test_system_with_full_data() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
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

        terminal
            .draw(|frame| {
                render(frame, frame.area(), &app);
            })
            .unwrap();
    }

    #[test]
    fn test_system_no_gpu() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.push_system(SystemMetrics {
            cpu_usage: 45.0,
            memory_used: 8_589_934_592,
            memory_total: 17_179_869_184,
            gpus: vec![],
        });

        terminal
            .draw(|frame| {
                render(frame, frame.area(), &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer.cell((x, y)).unwrap().symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("\n");
        assert!(content.contains("Not available"));
    }

    #[test]
    fn test_system_multiple_gpus() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.push_system(SystemMetrics {
            cpu_usage: 45.0,
            memory_used: 8_589_934_592,
            memory_total: 17_179_869_184,
            gpus: vec![
                GpuMetrics {
                    name: "RTX 4090".into(),
                    utilization: 95.0,
                    memory_used: 20_000_000_000,
                    memory_total: 24_000_000_000,
                    temperature: 72.0,
                },
                GpuMetrics {
                    name: "RTX 3080".into(),
                    utilization: 10.0,
                    memory_used: 2_000_000_000,
                    memory_total: 10_000_000_000,
                    temperature: 45.0,
                },
            ],
        });

        terminal
            .draw(|frame| {
                render(frame, frame.area(), &app);
            })
            .unwrap();
    }

    #[test]
    fn test_cpu_warning_color() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.push_system(SystemMetrics {
            cpu_usage: 85.0,
            memory_used: 8_589_934_592,
            memory_total: 17_179_869_184,
            gpus: vec![],
        });

        terminal
            .draw(|frame| {
                render(frame, frame.area(), &app);
            })
            .unwrap();
    }

    #[test]
    fn test_format_bytes_gb() {
        assert_eq!(format_bytes(8_589_934_592), "8.0 GB");
    }

    #[test]
    fn test_format_bytes_mb() {
        assert_eq!(format_bytes(536_870_912), "512.0 MB");
    }

    #[test]
    fn test_format_bytes_tb() {
        assert_eq!(format_bytes(1_099_511_627_776), "1.0 TB");
    }

    #[test]
    fn test_format_temp() {
        assert_eq!(format_temp(72.0), "72°C");
    }

    #[test]
    fn test_format_percent() {
        assert_eq!(format_percent(45.2), "45.2%");
    }

    #[test]
    fn test_system_tab_shows_hidden_gpu_count_indicator() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.push_system(SystemMetrics {
            cpu_usage: 30.0,
            memory_used: 1,
            memory_total: 2,
            gpus: vec![
                GpuMetrics {
                    name: "A".into(),
                    utilization: 10.0,
                    memory_used: 1,
                    memory_total: 2,
                    temperature: 40.0,
                },
                GpuMetrics {
                    name: "B".into(),
                    utilization: 20.0,
                    memory_used: 1,
                    memory_total: 2,
                    temperature: 41.0,
                },
                GpuMetrics {
                    name: "C".into(),
                    utilization: 30.0,
                    memory_used: 1,
                    memory_total: 2,
                    temperature: 42.0,
                },
            ],
        });

        terminal
            .draw(|frame| render(frame, frame.area(), &app))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let content = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer.cell((x, y)).unwrap().symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("\n");

        assert!(content.contains("+1 more GPUs"));
    }

    #[test]
    fn test_system_tab_highlights_gpu_outlier() {
        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.push_system(SystemMetrics {
            cpu_usage: 30.0,
            memory_used: 1,
            memory_total: 2,
            gpus: vec![
                GpuMetrics {
                    name: "A".into(),
                    utilization: 95.0,
                    memory_used: 1,
                    memory_total: 2,
                    temperature: 40.0,
                },
                GpuMetrics {
                    name: "B".into(),
                    utilization: 10.0,
                    memory_used: 1,
                    memory_total: 2,
                    temperature: 41.0,
                },
            ],
        });

        terminal
            .draw(|frame| render(frame, frame.area(), &app))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let content = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer.cell((x, y)).unwrap().symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("\n");

        assert!(content.contains("OUTLIER"));
    }

    #[test]
    fn test_system_graph_mode_switch_between_line_and_sparkline() {
        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.push_system(SystemMetrics {
            cpu_usage: 45.0,
            memory_used: 8_589_934_592,
            memory_total: 17_179_869_184,
            gpus: vec![],
        });
        for i in 0..20 {
            app.system.cpu_history.push_back((i * 300) as u64);
            app.system.ram_history.push_back((i * 200) as u64);
        }

        app.config.graph_mode = "sparkline".to_string();
        terminal
            .draw(|frame| render(frame, frame.area(), &app))
            .unwrap();

        app.config.graph_mode = "line".to_string();
        terminal
            .draw(|frame| render(frame, frame.area(), &app))
            .unwrap();
    }

    #[test]
    fn test_system_chart_uses_shared_graph_contract() {
        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());

        app.push_system(SystemMetrics {
            cpu_usage: 45.0,
            memory_used: 8_589_934_592,
            memory_total: 17_179_869_184,
            gpus: vec![],
        });
        for i in 0..80 {
            app.system.cpu_history.push_back((i * 120) as u64);
            app.system.ram_history.push_back((i * 100) as u64);
        }

        app.config.graph_mode = "line".to_string();
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            terminal.draw(|f| render(f, f.area(), &app)).unwrap();
        }));
        assert!(res.is_ok());
    }
}
