use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table, HighlightSpacing};
use std::time::SystemTime;

use crate::app::{App, HomeFocusTarget};
use crate::ui::components::truncate;
use crate::ui::theme::resolve_palette_from_config;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let palette = resolve_palette_from_config(&app.config);

    let horizontal_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    let left_col = horizontal_chunks[0];
    let right_col = horizontal_chunks[1];

    let left_col_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(left_col);

    let right_col_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Length(8),
            Constraint::Percentage(60),
        ])
        .split(right_col);

    let overview_area = left_col_chunks[0];
    let runs_area = left_col_chunks[1];
    let processes_area = left_col_chunks[2];

    let files_area = right_col_chunks[0];
    let system_area = right_col_chunks[1];
    let alerts_area = right_col_chunks[2];

    let focus = &app.ui_state.monitoring.home_focus;

    render_overview(frame, overview_area, app, &palette, *focus == HomeFocusTarget::Overview);
    render_runs(frame, runs_area, app, &palette, *focus == HomeFocusTarget::Runs);
    render_processes(frame, processes_area, app, &palette, *focus == HomeFocusTarget::Processes);
    render_files(frame, files_area, app, &palette, *focus == HomeFocusTarget::Files);
    render_system_summary(frame, system_area, app, &palette);
    render_alerts(frame, alerts_area, app, &palette, *focus == HomeFocusTarget::Alerts);
}

fn render_overview(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    palette: &crate::ui::theme::ThemePalette,
    is_focused: bool,
) {
    let has_active = app.training.latest.is_some();

    let title = if has_active {
        "▶  Active Run"
    } else {
        "○  No Active Run"
    };
    
    let mut border_style = Style::default();
    let mut title_style = Style::default();
    
    if is_focused {
        border_style = border_style.fg(palette.accent).add_modifier(Modifier::BOLD);
        title_style = title_style.fg(palette.accent).add_modifier(Modifier::BOLD);
    } else if has_active {
        border_style = border_style.fg(palette.success);
        title_style = title_style.fg(palette.header_fg);
    } else {
        border_style = border_style.fg(palette.muted);
        title_style = title_style.fg(palette.header_fg);
    }

    let block = Block::default()
        .title(title)
        .title_style(title_style)
        .borders(Borders::ALL)
        .border_style(border_style);

    if let Some(latest) = &app.training.latest {
        let step = latest
            .step
            .map(format_step)
            .unwrap_or_else(|| "N/A".to_string());
        let loss = latest
            .loss
            .map(|v| format!("{:.4}", v))
            .unwrap_or_else(|| "N/A".to_string());
            
        let lr = latest
            .learning_rate
            .map(|v| format!("{:.2e}", v))
            .unwrap_or_else(|| "N/A".to_string());

        let content = vec![
            Line::from(format!("Step: {:<8} Loss: {:<8} LR: {}", step, loss, lr)),
            Line::from(Span::styled(
                if is_focused { "[Press Enter to view Live Metrics]" } else { "" },
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

fn render_runs(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    palette: &crate::ui::theme::ThemePalette,
    is_focused: bool,
) {
    let mut title_style = Style::default();
    let mut border_style = Style::default();
    
    if is_focused {
        border_style = border_style.fg(palette.accent).add_modifier(Modifier::BOLD);
        title_style = title_style.fg(palette.accent).add_modifier(Modifier::BOLD);
    } else {
        border_style = border_style.fg(palette.muted);
        title_style = title_style.fg(palette.header_fg);
    }

    let block = Block::default()
        .title("Recent Runs")
        .title_style(title_style)
        .borders(Borders::ALL)
        .border_style(border_style);

    if app.recent_runs.is_empty() {
        let content = "No runs recorded yet.\nRun epoch with a training log to record your first run.";
        let paragraph = Paragraph::new(content)
            .block(block)
            .style(Style::default().fg(palette.muted))
            .alignment(Alignment::Center);
        frame.render_widget(paragraph, area);
        return;
    }

    let selected_idx = app
        .ui_state
        .monitoring
        .selected_run_id
        .as_ref()
        .and_then(|id| app.recent_runs.iter().position(|r| &r.run_id == id));

    let header = Row::new(vec!["St", "Name", "Step", "Date"])
        .style(Style::default().fg(palette.header_fg).add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = app
        .recent_runs
        .iter()
        .enumerate()
        .map(|(i, run)| {
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

            let is_selected = Some(i) == selected_idx;
            let mut style = Style::default();
            
            if is_selected && is_focused {
                style = style.fg(palette.header_bg).bg(palette.accent);
            } else if is_selected {
                style = style.add_modifier(Modifier::REVERSED);
            } else {
                style = style.fg(palette.header_fg);
            }

            Row::new(vec![
                Line::from(Span::styled(icon, Style::default().fg(if is_selected { style.fg.unwrap_or(icon_color) } else { icon_color }))),
                Line::from(name),
                Line::from(step),
                Line::from(date),
            ])
            .style(style)
        })
        .collect();

    let widths = [
        Constraint::Length(2),
        Constraint::Min(10),
        Constraint::Length(8),
        Constraint::Length(12),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .highlight_spacing(HighlightSpacing::Always);

    frame.render_widget(table, area);
}

fn render_processes(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    palette: &crate::ui::theme::ThemePalette,
    is_focused: bool,
) {
    let count = app.discovered_processes.len();
    let title = format!("⚡  Processes ({})", count);
    
    let mut title_style = Style::default();
    let mut border_style = Style::default();
    
    if is_focused {
        border_style = border_style.fg(palette.accent).add_modifier(Modifier::BOLD);
        title_style = title_style.fg(palette.accent).add_modifier(Modifier::BOLD);
    } else if count > 0 {
        border_style = border_style.fg(palette.warning);
        title_style = title_style.fg(palette.header_fg);
    } else {
        border_style = border_style.fg(palette.muted);
        title_style = title_style.fg(palette.header_fg);
    }

    let block = Block::default()
        .title(title)
        .title_style(title_style)
        .borders(Borders::ALL)
        .border_style(border_style);

    if count == 0 {
        let paragraph = Paragraph::new("No training processes detected")
            .block(block)
            .style(Style::default().fg(palette.muted))
            .alignment(Alignment::Center);
        frame.render_widget(paragraph, area);
        return;
    }

    let selected_idx = app
        .ui_state
        .monitoring
        .selected_pid
        .and_then(|pid| app.discovered_processes.iter().position(|p| p.pid == pid));

    let header = Row::new(vec!["PID", "Command", "CPU%"])
        .style(Style::default().fg(palette.header_fg).add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = app
        .discovered_processes
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let cmd = truncate(&p.command, 25);
            let cpu = p.cpu_milli_percent / 10;
            
            let is_selected = Some(i) == selected_idx;
            let mut style = Style::default();
            
            if is_selected && is_focused {
                style = style.fg(palette.header_bg).bg(palette.accent);
            } else if is_selected {
                style = style.add_modifier(Modifier::REVERSED);
            } else {
                style = style.fg(palette.header_fg);
            }

            Row::new(vec![
                Line::from(format!("{}", p.pid)),
                Line::from(cmd),
                Line::from(format!("{}%", cpu)),
            ])
            .style(style)
        })
        .collect();

    let widths = [
        Constraint::Length(7),
        Constraint::Min(10),
        Constraint::Length(6),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .highlight_spacing(HighlightSpacing::Always);

    frame.render_widget(table, area);
}

fn render_files(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    palette: &crate::ui::theme::ThemePalette,
    is_focused: bool,
) {
    let count = app.discovered_files.len();
    let title = format!("📁  Active Files ({})", count);
    
    let mut title_style = Style::default();
    let mut border_style = Style::default();
    
    if is_focused {
        border_style = border_style.fg(palette.accent).add_modifier(Modifier::BOLD);
        title_style = title_style.fg(palette.accent).add_modifier(Modifier::BOLD);
    } else if count > 0 {
        border_style = border_style.fg(palette.accent);
        title_style = title_style.fg(palette.header_fg);
    } else {
        border_style = border_style.fg(palette.muted);
        title_style = title_style.fg(palette.header_fg);
    }

    let block = Block::default()
        .title(title)
        .title_style(title_style)
        .borders(Borders::ALL)
        .border_style(border_style);

    if count == 0 {
        let paragraph = Paragraph::new("No training files detected")
            .block(block)
            .style(Style::default().fg(palette.muted))
            .alignment(Alignment::Center);
        frame.render_widget(paragraph, area);
        return;
    }

    let mut lines = Vec::new();
    for f in app.discovered_files.iter().take(8) {
        let filename = f.path.file_name().unwrap_or_default().to_string_lossy();
        let name = truncate(&filename, 30);
        let age = elapsed_pretty(f.modified);
        lines.push(Line::from(vec![
            Span::styled(format!("{:<30} ", name), Style::default().fg(palette.header_fg)),
            Span::styled(age, Style::default().fg(palette.muted)),
        ]));
    }

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn render_system_summary(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    palette: &crate::ui::theme::ThemePalette,
) {
    let block = Block::default()
        .title("System Summary")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(palette.muted));

    if let Some(system) = app.system.latest.as_ref() {
        let mem_pct = system.memory_usage_percent();
        
        let cpu_line = Line::from(vec![
            Span::styled("CPU: ", Style::default().fg(palette.cpu_color)),
            Span::styled(format!("{:.1}%", system.cpu_usage), Style::default().fg(palette.header_fg)),
        ]);

        let ram_line = Line::from(vec![
            Span::styled("RAM: ", Style::default().fg(palette.ram_color)),
            Span::styled(format!("{:.1}%", mem_pct), Style::default().fg(palette.header_fg)),
        ]);

        let mut lines = vec![cpu_line, ram_line];

        if !system.gpus.is_empty() {
            lines.push(Line::from(""));
            for (i, gpu) in system.gpus.iter().enumerate() {
                lines.push(Line::from(vec![
                    Span::styled(format!("GPU {}: ", i), Style::default().fg(palette.gpu_color)),
                    Span::styled(format!("{:.1}% util, {:.1}% mem", gpu.utilization, if gpu.memory_total > 0 { (gpu.memory_used as f64 / gpu.memory_total as f64) * 100.0 } else { 0.0 }), Style::default().fg(palette.header_fg)),
                ]));
            }
        }

        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
    } else {
        let paragraph = Paragraph::new("No system metrics available")
            .block(block)
            .style(Style::default().fg(palette.muted))
            .alignment(Alignment::Center);
        frame.render_widget(paragraph, area);
    }
}

fn render_alerts(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    palette: &crate::ui::theme::ThemePalette,
    is_focused: bool,
) {
    let mut title_style = Style::default();
    let mut border_style = Style::default();
    
    if is_focused {
        border_style = border_style.fg(palette.accent).add_modifier(Modifier::BOLD);
        title_style = title_style.fg(palette.accent).add_modifier(Modifier::BOLD);
    } else {
        border_style = border_style.fg(palette.muted);
        title_style = title_style.fg(palette.header_fg);
    }

    let block = Block::default()
        .title("Alerts")
        .title_style(title_style)
        .borders(Borders::ALL)
        .border_style(border_style);

    if app.alerts.active.is_empty() && app.alerts.resolved.is_empty() {
        let empty = Paragraph::new("  ✓  No alerts")
            .alignment(Alignment::Left)
            .block(block)
            .style(Style::default().fg(palette.success));
        frame.render_widget(empty, area);
        return;
    }

    let mut lines: Vec<Line> = Vec::new();

    for alert in app.alerts.active.iter().take(5) {
        let (prefix, color) = match alert.level {
            crate::app::AlertLevel::Critical => ("CRIT", palette.error),
            crate::app::AlertLevel::Warning => ("WARN", palette.warning),
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{prefix} "), Style::default().fg(color).add_modifier(Modifier::BOLD)),
            Span::styled(&alert.message, Style::default().fg(color)),
        ]));
    }

    if !app.alerts.active.is_empty() && !app.alerts.resolved.is_empty() {
        lines.push(Line::from(Span::styled("--- resolved ---", Style::default().fg(palette.muted))));
    }

    for alert in app.alerts.resolved.iter().rev().take(3) {
        lines.push(Line::from(Span::styled(&alert.message, Style::default().fg(palette.muted))));
    }

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
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
