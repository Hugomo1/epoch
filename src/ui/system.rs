use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, LineGauge, Paragraph, Sparkline};

use crate::app::App;
use crate::ui::{CPU_COLOR, GPU_COLOR, HEADER_FG, MUTED, RAM_COLOR, WARNING};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let Some(system) = &app.system.latest else {
        let msg = Paragraph::new("Collecting system metrics...")
            .style(Style::default().fg(MUTED))
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
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(6),
            Constraint::Min(0),
        ])
        .split(area);

    let cpu_ratio = (system.cpu_usage / 100.0).clamp(0.0, 1.0);
    let cpu_color = if system.cpu_usage > 80.0 {
        WARNING
    } else {
        CPU_COLOR
    };

    let cpu_gauge = LineGauge::default()
        .block(Block::default().title("CPU").borders(Borders::ALL))
        .filled_style(Style::default().fg(cpu_color))
        .filled_symbol("█")
        .unfilled_symbol(" ")
        .ratio(cpu_ratio)
        .label(format!("{:.1}%", system.cpu_usage));

    frame.render_widget(cpu_gauge, layout[0]);

    let ram_ratio = (system.memory_used as f64 / system.memory_total.max(1) as f64).clamp(0.0, 1.0);
    let ram_gauge = LineGauge::default()
        .block(Block::default().title("RAM").borders(Borders::ALL))
        .filled_style(Style::default().fg(RAM_COLOR))
        .filled_symbol("█")
        .unfilled_symbol(" ")
        .ratio(ram_ratio)
        .label(format!(
            "{} / {}",
            format_bytes(system.memory_used),
            format_bytes(system.memory_total)
        ));

    frame.render_widget(ram_gauge, layout[1]);

    if system.gpus.is_empty() {
        let block = Block::default()
            .title("GPU: Not available")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(MUTED));
        let p = Paragraph::new("No GPU detected")
            .style(Style::default().fg(MUTED))
            .block(block);
        frame.render_widget(p, layout[2]);
    } else {
        let gpu_constraints = system
            .gpus
            .iter()
            .map(|_| Constraint::Length(3))
            .collect::<Vec<_>>();
        let gpus_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(gpu_constraints)
            .split(layout[2]);

        for (i, gpu) in system.gpus.iter().enumerate() {
            if i >= gpus_layout.len() {
                break;
            }

            let gpu_area = gpus_layout[i];
            let gpu_ratio = (gpu.utilization / 100.0).clamp(0.0, 1.0);

            let block = Block::default()
                .title(format!("GPU {}: {}", i, gpu.name))
                .borders(Borders::ALL);
            let inner_area = block.inner(gpu_area);
            frame.render_widget(block, gpu_area);

            let gpu_inner_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Length(1)])
                .split(inner_area);

            let gpu_gauge = LineGauge::default()
                .filled_style(Style::default().fg(GPU_COLOR))
                .filled_symbol("█")
                .unfilled_symbol(" ")
                .ratio(gpu_ratio)
                .label(format!("{:.1}%", gpu.utilization));
            frame.render_widget(gpu_gauge, gpu_inner_layout[0]);

            let detail_text = format!(
                "{} / {}   {}",
                format_bytes(gpu.memory_used),
                format_bytes(gpu.memory_total),
                format_temp(gpu.temperature)
            );

            let detail = Paragraph::new(detail_text).style(Style::default().fg(HEADER_FG));
            frame.render_widget(detail, gpu_inner_layout[1]);
        }
    }

    let history_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(layout[3]);

    let (mut cpu_data, mut ram_data) = (
        app.system.cpu_history.clone(),
        app.system.ram_history.clone(),
    );
    let cpu_slice: &[u64] = cpu_data.make_contiguous();
    let ram_slice: &[u64] = ram_data.make_contiguous();

    let cpu_sparkline = Sparkline::default()
        .block(Block::default().title("CPU History").borders(Borders::ALL))
        .data(cpu_slice)
        .style(Style::default().fg(CPU_COLOR))
        .max(10000);
    frame.render_widget(cpu_sparkline, history_layout[0]);

    let ram_sparkline = Sparkline::default()
        .block(Block::default().title("RAM History").borders(Borders::ALL))
        .data(ram_slice)
        .style(Style::default().fg(RAM_COLOR))
        .max(10000);
    frame.render_widget(ram_sparkline, history_layout[1]);
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

pub fn format_percent(v: f64) -> String {
    format!("{:.1}%", v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::types::{GpuMetrics, SystemMetrics};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

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
}
