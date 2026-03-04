use std::collections::VecDeque;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph, Sparkline};

use crate::app::App;
use crate::ui::{
    HEADER_FG, LOSS_COLOR, LR_COLOR, MUTED, SUCCESS, WARNING, metric_label_style,
    metric_value_style,
};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    if app.training.latest.is_none() {
        let text = "No training metrics received yet.\nStart a training run and pipe output via --stdin or --log-file";
        let paragraph = Paragraph::new(text)
            .alignment(Alignment::Center)
            .style(Style::default().fg(MUTED));

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
            Constraint::Percentage(30),
            Constraint::Percentage(30),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
        ])
        .split(area);

    let latest = app
        .training
        .latest
        .as_ref()
        .expect("Already checked is_none()");

    // 1. Loss Sparkline
    let loss_history: Vec<u64> = app.training.loss_history.iter().copied().collect();
    let current_loss = latest.loss.unwrap_or(0.0);
    let loss_trend = trend_indicator(&app.training.loss_history);

    let loss_title = format!("Loss: {:.4} {}", current_loss, loss_trend);
    let loss_block = Block::default()
        .borders(Borders::ALL)
        .title(loss_title)
        .title_style(metric_label_style());

    if loss_history.is_empty() {
        let para = Paragraph::new("No loss data")
            .block(loss_block)
            .style(Style::default().fg(MUTED));
        frame.render_widget(para, chunks[0]);
    } else {
        let sparkline = Sparkline::default()
            .block(loss_block)
            .data(&loss_history)
            .style(Style::default().fg(LOSS_COLOR));
        frame.render_widget(sparkline, chunks[0]);
    }

    // 2. Learning Rate Sparkline
    let lr_history: Vec<u64> = app.training.lr_history.iter().copied().collect();
    let current_lr = latest.learning_rate.unwrap_or(0.0);

    let lr_title = format!("Learning Rate: {}", format_lr_value(current_lr));
    let lr_block = Block::default()
        .borders(Borders::ALL)
        .title(lr_title)
        .title_style(metric_label_style());

    if lr_history.is_empty() {
        let para = Paragraph::new("No LR data")
            .block(lr_block)
            .style(Style::default().fg(MUTED));
        frame.render_widget(para, chunks[1]);
    } else {
        let sparkline = Sparkline::default()
            .block(lr_block)
            .data(&lr_history)
            .style(Style::default().fg(LR_COLOR));
        frame.render_widget(sparkline, chunks[1]);
    }

    // 3. Stats Row
    let stats_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[2]);

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

    let throughput_text = if let Some(t) = latest.throughput {
        format!("Throughput: {:.1} tok/s", t)
    } else {
        "Throughput: —".to_string()
    };

    let step_block = Block::default().borders(Borders::ALL);
    let step_para = Paragraph::new(step_text)
        .style(metric_value_style())
        .block(step_block);

    let throughput_block = Block::default().borders(Borders::ALL);
    let throughput_para = Paragraph::new(throughput_text)
        .style(metric_value_style())
        .block(throughput_block);

    frame.render_widget(step_para, stats_chunks[0]);
    frame.render_widget(throughput_para, stats_chunks[1]);

    // 4. Summary Row
    let summary_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(chunks[3]);

    let elapsed = app.elapsed();
    let time_text = if app.training.start_time.is_some() {
        let hours = elapsed.as_secs() / 3600;
        let mins = (elapsed.as_secs() % 3600) / 60;
        let secs = elapsed.as_secs() % 60;
        format!("Running: {:02}:{:02}:{:02}", hours, mins, secs)
    } else {
        "Idle".to_string()
    };

    let (status_text, status_color) = if app.training.input_active {
        ("Live", SUCCESS)
    } else if app.training.last_data_at.is_some() {
        ("Stale", WARNING)
    } else {
        ("No data", MUTED)
    };

    let time_block = Block::default().borders(Borders::ALL);
    let time_para = Paragraph::new(time_text)
        .style(Style::default().fg(HEADER_FG))
        .block(time_block);

    let status_block = Block::default().borders(Borders::ALL);
    let status_para = Paragraph::new(status_text)
        .style(
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(status_block);

    let points_text = format!("Points: {}", loss_history.len());
    let points_block = Block::default().borders(Borders::ALL);
    let points_para = Paragraph::new(points_text)
        .style(Style::default().fg(MUTED))
        .alignment(Alignment::Right)
        .block(points_block);

    frame.render_widget(time_para, summary_chunks[0]);
    frame.render_widget(status_para, summary_chunks[1]);
    frame.render_widget(points_para, summary_chunks[2]);
}

fn trend_indicator(history: &VecDeque<u64>) -> &'static str {
    if history.len() < 2 {
        return "→";
    }

    let last = *history.back().expect("history len >= 2") as f64;
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
}
