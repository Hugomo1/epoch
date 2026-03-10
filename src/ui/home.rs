use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::{App, HomeFocusTarget};
use crate::ui::components::{format_duration, format_step};
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
            Constraint::Min(12),
            Constraint::Length(process_panel_height(app)),
        ])
        .split(left_col);

    let right_col_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(0)])
        .split(right_col);

    let overview_area = left_col_chunks[0];
    let runs_area = left_col_chunks[1];
    let processes_area = left_col_chunks[2];

    let system_area = right_col_chunks[0];
    let alerts_area = right_col_chunks[1];

    let focus = &app.ui_state.monitoring.home_focus;

    render_overview(
        frame,
        overview_area,
        app,
        &palette,
        *focus == HomeFocusTarget::Overview,
    );
    crate::ui::run_explorer::render_runs_panel(
        frame,
        runs_area,
        app,
        *focus == HomeFocusTarget::Runs,
    );
    crate::ui::system_processes::render_processes_table(
        frame,
        processes_area,
        app,
        &palette,
        *focus == HomeFocusTarget::Processes,
    );
    crate::ui::system_processes::render_resource_strip(frame, system_area, app, &palette);
    render_alerts(
        frame,
        alerts_area,
        app,
        &palette,
        *focus == HomeFocusTarget::Alerts,
    );
}

fn render_overview(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    palette: &crate::ui::theme::ThemePalette,
    is_focused: bool,
) {
    let run_state_title = match app.training_data_health_state() {
        crate::app::DataHealthState::Live => "Current Run",
        crate::app::DataHealthState::Stale => "Latest Run Snapshot",
        crate::app::DataHealthState::NoData => "No Live Run",
    };

    let title = format!("[1] {run_state_title}");

    let mut border_style = Style::default();
    let mut title_style = Style::default();

    if is_focused {
        border_style = border_style.fg(palette.accent).add_modifier(Modifier::BOLD);
        title_style = title_style.fg(palette.accent).add_modifier(Modifier::BOLD);
    } else if app.training.latest.is_some() {
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
        let run_duration = app
            .selected_run_elapsed()
            .map(format_duration)
            .unwrap_or_else(|| "N/A".to_string());
        let active_runs = app.active_run_count();

        let content = vec![
            Line::from(format!("Step: {:<8} Loss: {:<8} LR: {}", step, loss, lr)),
            Line::from(format!(
                "Run time: {:<8} Active runs: {}",
                run_duration, active_runs
            )),
            Line::from(Span::styled(
                if is_focused {
                    "[Press Enter to view the current run]"
                } else {
                    ""
                },
                Style::default().fg(palette.muted),
            )),
        ];

        let paragraph = Paragraph::new(content)
            .block(block)
            .style(Style::default().fg(palette.header_fg));
        frame.render_widget(paragraph, area);
    } else {
        let paragraph =
            Paragraph::new("No metrics received yet. Focus Runs to browse stored runs.")
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
        .title("[4] Alerts")
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
            Span::styled(
                format!("{prefix} "),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(&alert.message, Style::default().fg(color)),
        ]));
    }

    if !app.alerts.active.is_empty() && !app.alerts.resolved.is_empty() {
        lines.push(Line::from(Span::styled(
            "--- resolved ---",
            Style::default().fg(palette.muted),
        )));
    }

    for alert in app.alerts.resolved.iter().rev().take(3) {
        lines.push(Line::from(Span::styled(
            &alert.message,
            Style::default().fg(palette.muted),
        )));
    }

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn process_panel_height(app: &App) -> u16 {
    if app.discovered_processes.is_empty() {
        return 5;
    }

    let visible_rows = app.discovered_processes.len().min(6) as u16;
    (visible_rows + 3).min(10)
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
