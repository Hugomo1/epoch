use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::App;
use crate::ui::theme::resolve_palette_from_config;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let palette = resolve_palette_from_config(&app.config);
    let columns = explorer_columns().join(" | ");
    let body = format!(
        "Columns:\n{}\n\nFilters: project, status, date, tags, metric thresholds, git commit, config values\nSearch: fuzzy\nActions: open run, inspect config",
        columns
    );
    let paragraph = Paragraph::new(body)
        .style(Style::default().fg(palette.header_fg))
        .block(Block::default().title("Run Explorer").borders(Borders::ALL));
    frame.render_widget(paragraph, area);
}

pub fn explorer_columns() -> Vec<&'static str> {
    vec![
        "Name",
        "Project",
        "Status",
        "Duration",
        "Best Metric",
        "Current/Final Step",
        "Start Date",
        "Git State",
        "Device Info",
    ]
}

pub fn filter_runs_by_project_status_date(
    rows: &[(String, String, String)],
    project: &str,
    status: &str,
    date: &str,
) -> Vec<(String, String, String)> {
    rows.iter()
        .filter(|(p, s, d)| p == project && s == status && d == date)
        .cloned()
        .collect()
}

pub fn fuzzy_search_runs(rows: &[String], query: &str) -> Vec<String> {
    if query.is_empty() {
        return rows.to_vec();
    }
    let query_lower = query.to_ascii_lowercase();
    rows.iter()
        .filter(|row| row.to_ascii_lowercase().contains(&query_lower))
        .cloned()
        .collect()
}
