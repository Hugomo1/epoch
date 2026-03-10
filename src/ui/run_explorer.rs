use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table, TableState};

use crate::app::App;
use crate::store::types::RunStatus;
use crate::ui::components::{format_epoch_date, format_step, truncate};
use crate::ui::theme::{ThemePalette, resolve_palette_from_config};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let palette = resolve_palette_from_config(&app.config);
    let state = &app.ui_state.explorer;

    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(3),
    ])
    .split(area);

    render_filter_bar(frame, chunks[0], state, &palette);
    render_run_table(frame, chunks[1], state, &palette);
    render_detail_strip(frame, chunks[2], state, &palette);
}

fn render_filter_bar(
    frame: &mut Frame,
    area: Rect,
    state: &crate::app::RunExplorerUiState,
    palette: &ThemePalette,
) {
    let status_label = match state.status_filter {
        None => Span::styled("all", Style::default().fg(palette.muted)),
        Some(RunStatus::Active) => Span::styled("active", Style::default().fg(palette.accent)),
        Some(RunStatus::Completed) => {
            Span::styled("completed", Style::default().fg(palette.accent))
        }
        Some(RunStatus::Failed) => Span::styled("failed", Style::default().fg(palette.accent)),
    };

    let query_or_all = if state.search_query.is_empty() {
        Span::styled("all", Style::default().fg(palette.muted))
    } else {
        Span::styled(&state.search_query, Style::default().fg(palette.accent))
    };

    let count_span = Span::styled(
        format!("   {} runs", state.records.len()),
        Style::default().fg(palette.header_fg),
    );

    let line = Line::from(vec![
        Span::styled("  Status: ", Style::default().fg(palette.header_fg)),
        status_label,
        Span::styled("   Search: ", Style::default().fg(palette.header_fg)),
        query_or_all,
        count_span,
    ]);

    let block = Block::default()
        .title("Run Explorer")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(palette.header_fg));

    let p = Paragraph::new(line).block(block);
    frame.render_widget(p, area);
}

fn render_run_table(
    frame: &mut Frame,
    area: Rect,
    state: &crate::app::RunExplorerUiState,
    palette: &ThemePalette,
) {
    if state.records.is_empty() {
        let text =
            "No runs found.\nStart a training run to record your first entry.\nPress r to refresh.";
        let p = Paragraph::new(text)
            .style(Style::default().fg(palette.muted))
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(p, area);
        return;
    }

    let header = Row::new(vec!["St", "Name", "Step", "Started", "Source"])
        .style(Style::default().add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = state
        .records
        .iter()
        .map(|rec| {
            let (status_icon, status_color) = match rec.status {
                RunStatus::Active => ("●", palette.success),
                RunStatus::Completed => ("✓", palette.muted),
                RunStatus::Failed => ("✗", palette.error),
            };

            let name = truncate(
                rec.display_name
                    .as_deref()
                    .unwrap_or(rec.source_locator.as_deref().unwrap_or("unnamed")),
                20,
            );
            let step = rec
                .last_step
                .map(format_step)
                .unwrap_or_else(|| "-".to_string());
            let started = format_epoch_date(rec.started_at_epoch_secs);
            let source = rec.source_kind.as_str();

            Row::new(vec![
                Line::from(Span::styled(status_icon, Style::default().fg(status_color))),
                Line::from(name),
                Line::from(step),
                Line::from(started),
                Line::from(source.to_string()),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(2),
            Constraint::Length(20),
            Constraint::Length(8),
            Constraint::Length(14),
            Constraint::Length(12),
        ],
    )
    .header(header)
    .row_highlight_style(
        Style::default()
            .bg(palette.header_bg)
            .fg(palette.accent)
            .add_modifier(Modifier::BOLD),
    );

    let mut table_state = TableState::default();
    table_state.select(Some(state.selected_idx));

    frame.render_stateful_widget(table, area, &mut table_state);
}

fn render_detail_strip(
    frame: &mut Frame,
    area: Rect,
    state: &crate::app::RunExplorerUiState,
    palette: &ThemePalette,
) {
    if state.search_active {
        let text = format!("Search: {}|", state.search_query);
        let block = Block::default()
            .title("Search")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette.accent));
        let p = Paragraph::new(text)
            .block(block)
            .style(Style::default().fg(palette.accent));
        frame.render_widget(p, area);
    } else if !state.records.is_empty() && state.selected_idx < state.records.len() {
        let rec = &state.records[state.selected_idx];
        let loc = truncate(rec.source_locator.as_deref().unwrap_or(""), 40);
        let id_trunc = if rec.run_id.len() > 8 {
            &rec.run_id[..8]
        } else {
            &rec.run_id
        };
        let text = format!("  {}   |   Run ID: {}", loc, id_trunc);

        let block = Block::default()
            .title("Detail")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette.muted));
        let p = Paragraph::new(text)
            .block(block)
            .style(Style::default().fg(palette.muted));
        frame.render_widget(p, area);
    } else {
        let text = "  /: search   f: filter status   Enter: open active run";
        let block = Block::default()
            .title("Hint")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette.muted));
        let p = Paragraph::new(text)
            .block(block)
            .style(Style::default().fg(palette.muted));
        frame.render_widget(p, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::store::types::{RunRecord, RunSourceKind, RunStatus};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn make_terminal() -> Terminal<TestBackend> {
        Terminal::new(TestBackend::new(120, 40)).unwrap()
    }

    #[test]
    fn test_run_explorer_renders_empty() {
        let app = App::new(Default::default());
        let mut terminal = make_terminal();
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("No runs found"),
            "Expected 'No runs found' in buffer"
        );
    }

    #[test]
    fn test_run_explorer_renders_records() {
        let mut app = App::new(Default::default());
        app.ui_state.explorer.records.push(RunRecord {
            run_id: "test-id-123".to_string(),
            source_fingerprint: "fingerprint".to_string(),
            source_kind: RunSourceKind::Process,
            source_locator: Some("some_script.py".to_string()),
            project_root: None,
            display_name: Some("MyTestRun".to_string()),
            status: RunStatus::Active,
            command: None,
            cwd: None,
            git_commit: None,
            git_dirty: None,
            started_at_epoch_secs: 1600000000,
            ended_at_epoch_secs: None,
            last_step: Some(42),
            last_updated_epoch_secs: 1600000000,
        });

        let mut terminal = make_terminal();
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();
        assert!(content.contains("MyTestRun"), "Expected run name in buffer");
        assert!(
            content.contains("process"),
            "Expected source kind in buffer"
        );
        assert!(
            content.contains("test-id-"),
            "Expected run id in detail strip"
        );
    }

    #[test]
    fn test_run_explorer_search_active() {
        let mut app = App::new(Default::default());
        app.ui_state.explorer.search_active = true;
        app.ui_state.explorer.search_query = "foo".to_string();

        let mut terminal = make_terminal();
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("Search: foo|"),
            "Expected search strip content"
        );
    }
}
