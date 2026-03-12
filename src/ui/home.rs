use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::{App, HomeFocusTarget};
use crate::ui::alerts_panel::{AlertPanelData, render_alert_panel};
use crate::ui::components::{centered_text_area, format_duration, format_step};
use crate::ui::theme::resolve_palette_from_config;

const TOP_SUMMARY_PANEL_HEIGHT: u16 = 7;
const RIGHT_COLUMN_TARGET_WIDTH: u16 = 58;
const RIGHT_COLUMN_MIN_WIDTH: u16 = 28;
const LEFT_COLUMN_MIN_WIDTH: u16 = 40;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let palette = resolve_palette_from_config(&app.config);

    let right_width = right_column_width(area.width);

    let horizontal_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(right_width)])
        .split(area);

    let left_col = horizontal_chunks[0];
    let right_col = horizontal_chunks[1];

    let left_col_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(TOP_SUMMARY_PANEL_HEIGHT),
            Constraint::Min(12),
            Constraint::Length(process_panel_height(app)),
        ])
        .split(left_col);

    let right_col_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(TOP_SUMMARY_PANEL_HEIGHT),
            Constraint::Min(0),
        ])
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
    render_alerts(frame, alerts_area, app, &palette);
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
        let message = "No metrics received yet. Focus Runs to browse stored runs.";
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let paragraph = Paragraph::new(message)
            .style(Style::default().fg(palette.muted))
            .alignment(Alignment::Center);
        frame.render_widget(paragraph, centered_text_area(inner, message));
    }
}

fn render_alerts(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    palette: &crate::ui::theme::ThemePalette,
) {
    let (active, resolved) = app.home_alert_records();
    let data = AlertPanelData::from_records(&active, &resolved);
    render_alert_panel(frame, area, &data, palette, "Alerts", false, 5, 3);
}

fn right_column_width(total_width: u16) -> u16 {
    let max_allowed = total_width.saturating_sub(LEFT_COLUMN_MIN_WIDTH);
    let target = RIGHT_COLUMN_TARGET_WIDTH.min(max_allowed);
    let min_width = RIGHT_COLUMN_MIN_WIDTH.min(total_width.saturating_sub(1));
    target.max(min_width)
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
