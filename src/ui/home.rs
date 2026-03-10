use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table};
use std::time::SystemTime;

use crate::app::App;
use crate::ui::components::truncate;
use crate::ui::theme::resolve_palette_from_config;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let palette = resolve_palette_from_config(&app.config);

    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(area);

    let main_area = vertical_chunks[0];
    let alert_area = vertical_chunks[1];

    let horizontal_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(main_area);

    let left_col = horizontal_chunks[0];
    let right_col = horizontal_chunks[1];

    let left_col_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Min(0),
            Constraint::Length(5),
        ])
        .split(left_col);

    let active_run_box = left_col_chunks[0];
    let process_box = left_col_chunks[1];
    let files_box = left_col_chunks[2];

    let right_col_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(right_col);

    let recent_runs_area = right_col_chunks[0];
    let quick_actions_area = right_col_chunks[1];

    render_active_run(frame, active_run_box, app, &palette);
    render_processes(frame, process_box, app, &palette);
    render_files(frame, files_box, app, &palette);
    render_recent_runs(frame, recent_runs_area, app, &palette);
    render_quick_actions(frame, quick_actions_area, app, &palette);
    render_alerts(frame, alert_area, app, &palette);
}

fn render_active_run(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    palette: &crate::ui::theme::ThemePalette,
) {
    let has_active = app.training.latest.is_some();

    let title = if has_active {
        "▶  Active Run"
    } else {
        "○  No Active Run"
    };
    let border_color = if has_active {
        palette.success
    } else {
        palette.muted
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    if let Some(latest) = &app.training.latest {
        let step = latest
            .step
            .map(format_step)
            .unwrap_or_else(|| "N/A".to_string());
        let loss = latest
            .loss
            .map(|v| format!("{:.4}", v))
            .unwrap_or_else(|| "N/A".to_string());

        let content = vec![
            Line::from(format!("Step: {:<8} Loss: {}", step, loss)),
            Line::from(Span::styled(
                "[→ Tab or 2 for Live Run]",
                Style::default().fg(palette.muted),
            )),
        ];

        let paragraph = Paragraph::new(content)
            .block(block)
            .style(Style::default().fg(palette.header_fg));
        frame.render_widget(paragraph, area);
    } else {
        let paragraph = Paragraph::new("No metrics received yet")
            .block(block)
            .style(Style::default().fg(palette.muted))
            .alignment(Alignment::Center);
        frame.render_widget(paragraph, area);
    }
}

fn render_processes(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    palette: &crate::ui::theme::ThemePalette,
) {
    let count = app.discovered_processes.len();
    let title = format!("⚡  Processes ({})", count);
    let border_color = if count > 0 {
        palette.warning
    } else {
        palette.muted
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    if count == 0 {
        let paragraph = Paragraph::new("No training processes detected")
            .block(block)
            .style(Style::default().fg(palette.muted))
            .alignment(Alignment::Center);
        frame.render_widget(paragraph, area);
        return;
    }

    let mut lines = Vec::new();
    for p in app.discovered_processes.iter().take(4) {
        let cmd = truncate(&p.command, 25);
        let cpu = p.cpu_milli_percent / 10;
        lines.push(Line::from(format!(" {:<7} {:<25} {}%", p.pid, cmd, cpu)));
    }

    let paragraph = Paragraph::new(lines)
        .block(block)
        .style(Style::default().fg(palette.header_fg));
    frame.render_widget(paragraph, area);
}

fn render_files(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    palette: &crate::ui::theme::ThemePalette,
) {
    let count = app.discovered_files.len();
    let title = format!("📁  Active Files ({})", count);
    let border_color = if count > 0 {
        palette.accent
    } else {
        palette.muted
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    if count == 0 {
        let paragraph = Paragraph::new("No training files detected")
            .block(block)
            .style(Style::default().fg(palette.muted))
            .alignment(Alignment::Center);
        frame.render_widget(paragraph, area);
        return;
    }

    let mut lines = Vec::new();
    for f in app.discovered_files.iter().take(3) {
        let filename = f.path.file_name().unwrap_or_default().to_string_lossy();
        let name = truncate(&filename, 25);
        let age = elapsed_pretty(f.modified);
        lines.push(Line::from(format!(" {:<25} {}", name, age)));
    }

    let paragraph = Paragraph::new(lines)
        .block(block)
        .style(Style::default().fg(palette.header_fg));
    frame.render_widget(paragraph, area);
}

fn render_recent_runs(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    palette: &crate::ui::theme::ThemePalette,
) {
    let block = Block::default()
        .title("Recent Runs")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(palette.header_fg));

    if app.recent_runs.is_empty() {
        let content =
            "No runs recorded yet.\nRun epoch with a training log to record your first run.";
        let paragraph = Paragraph::new(content)
            .block(block)
            .style(Style::default().fg(palette.muted))
            .alignment(Alignment::Center);
        frame.render_widget(paragraph, area);
        return;
    }

    let header = Row::new(vec!["St", "Name", "Step", "Date"])
        .style(Style::default().add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = app
        .recent_runs
        .iter()
        .take(5)
        .map(|run| {
            let (icon, icon_color) = match run.status {
                crate::store::types::RunStatus::Active => ("●", palette.success),
                crate::store::types::RunStatus::Completed => ("✓", palette.muted),
                crate::store::types::RunStatus::Failed => ("✗", palette.error),
            };

            let name = truncate(run.display_name.as_deref().unwrap_or("unnamed"), 18);
            let step = run
                .last_step
                .map(format_step)
                .unwrap_or_else(|| "-".to_string());
            let date = format_epoch_date(run.started_at_epoch_secs);

            Row::new(vec![
                Line::from(Span::styled(icon, Style::default().fg(icon_color))),
                Line::from(name),
                Line::from(step),
                Line::from(date),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(2),
        Constraint::Length(18),
        Constraint::Length(8),
        Constraint::Length(12),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .style(Style::default().fg(palette.header_fg));

    frame.render_widget(table, area);
}

fn render_quick_actions(
    frame: &mut Frame,
    area: Rect,
    _app: &App,
    palette: &crate::ui::theme::ThemePalette,
) {
    let block = Block::default()
        .title("Quick Actions")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(palette.header_fg));

    let actions = [
        ("o", "Open file..."),
        ("a", "Attach to process"),
        ("e", "Explore all runs"),
        ("s", "Scan directory"),
        ("r", "Refresh"),
    ];

    let mut lines = Vec::new();
    for (key, desc) in actions {
        lines.push(Line::from(vec![
            Span::raw(" ["),
            Span::styled(key, Style::default().fg(palette.accent)),
            Span::raw("]  "),
            Span::styled(desc, Style::default().fg(palette.header_fg)),
        ]));
    }

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn render_alerts(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    palette: &crate::ui::theme::ThemePalette,
) {
    if app.alerts.active.is_empty() {
        let line = Line::from(Span::styled(
            "  ✓  No alerts",
            Style::default().fg(palette.success),
        ));
        let paragraph = Paragraph::new(vec![line]).style(Style::default().fg(palette.success));
        frame.render_widget(paragraph, area);
    } else {
        let count = app.alerts.active.len();
        let first_msg = truncate(&app.alerts.active[0].message, 60);
        let text = format!("  ⚠  {} alert(s): {}", count, first_msg);
        let line = Line::from(Span::styled(text, Style::default().fg(palette.error)));
        let paragraph = Paragraph::new(vec![line]).style(Style::default().fg(palette.error));
        frame.render_widget(paragraph, area);
    }
}

fn elapsed_pretty(modified: SystemTime) -> String {
    let duration = SystemTime::now()
        .duration_since(modified)
        .unwrap_or_default()
        .as_secs();
    if duration < 60 {
        format!("{}s ago", duration)
    } else if duration < 3600 {
        format!("{}m ago", duration / 60)
    } else {
        format!("{}h ago", duration / 3600)
    }
}

fn format_epoch_date(secs: i64) -> String {
    let dt = std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs as u64);
    let sys_time = dt
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let days = sys_time / 86400;

    let mut d = days;
    let mut y = 1970;
    loop {
        let leap = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
            1
        } else {
            0
        };
        let days_in_year = 365 + leap;
        if d < days_in_year {
            break;
        }
        d -= days_in_year;
        y += 1;
    }

    let leap = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
        1
    } else {
        0
    };
    let month_days = [31, 28 + leap, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let month_names = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    let mut m = 0;
    for (i, &md) in month_days.iter().enumerate() {
        if d < md {
            m = i;
            break;
        }
        d -= md;
    }

    let day = d + 1;

    let seconds_in_day = sys_time % 86400;
    let hours = seconds_in_day / 3600;
    let minutes = (seconds_in_day % 3600) / 60;

    format!("{} {:02} {:02}:{:02}", month_names[m], day, hours, minutes)
}

fn format_step(step: u64) -> String {
    if step < 10000 {
        let s = step.to_string();
        let bytes = s.as_bytes();
        let mut result = String::new();

        for (count, &b) in bytes.iter().rev().enumerate() {
            if count > 0 && count % 3 == 0 {
                result.push(',');
            }
            result.push(b as char);
        }
        result.chars().rev().collect()
    } else if step < 1_000_000 {
        format!("{:.1}k", step as f64 / 1000.0)
    } else {
        format!("{:.1}m", step as f64 / 1_000_000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn make_terminal() -> Terminal<TestBackend> {
        Terminal::new(TestBackend::new(120, 40)).unwrap()
    }

    #[test]
    fn test_home_renders_without_panic() {
        let app = crate::app::App::new(Default::default());
        let mut terminal = make_terminal();
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &app);
            })
            .unwrap();
    }
}
