use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, HighlightSpacing, Paragraph, Row, Table, TableState};

use crate::app::App;
use crate::store::types::RunStatus;
use crate::ui::components::{centered_text_area, format_epoch_date, format_step, truncate};
use crate::ui::theme::{ThemePalette, resolve_palette_from_config};

pub fn render_runs_panel(frame: &mut Frame, area: Rect, app: &App, is_focused: bool) {
    let palette = resolve_palette_from_config(&app.config);
    let state = &app.ui_state.explorer;

    let mut title_style = Style::default();
    let mut border_style = Style::default();
    let active_count = state
        .records
        .iter()
        .filter(|record| matches!(record.status, RunStatus::Active))
        .count();

    if is_focused {
        border_style = border_style.fg(palette.accent).add_modifier(Modifier::BOLD);
        title_style = title_style.fg(palette.accent).add_modifier(Modifier::BOLD);
    } else {
        border_style = border_style.fg(palette.muted);
        title_style = title_style.fg(palette.header_fg);
    }

    let block = Block::default()
        .title(format!(
            "[2] Runs ({}, {} active)",
            state.records.len(),
            active_count
        ))
        .title_style(title_style)
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(inner_area);

    render_filter_bar(frame, chunks[0], state, &palette);
    render_run_table(frame, chunks[1], state, &palette, is_focused);
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

    // Use cached active count if available, otherwise fall back to computing
    let active_count = if state.active_count > 0 {
        state.active_count
    } else {
        state
            .records
            .iter()
            .filter(|record| matches!(record.status, RunStatus::Active))
            .count()
    };

    let count_span = Span::styled(
        format!("   {} runs, {} active", state.records.len(), active_count),
        Style::default().fg(palette.header_fg),
    );

    let line = Line::from(vec![
        Span::styled("  Status: ", Style::default().fg(palette.header_fg)),
        status_label,
        Span::styled("   Search: ", Style::default().fg(palette.header_fg)),
        query_or_all,
        count_span,
    ]);

    let p = Paragraph::new(line);
    frame.render_widget(p, area);
}

fn render_run_table(
    frame: &mut Frame,
    area: Rect,
    state: &crate::app::RunExplorerUiState,
    palette: &ThemePalette,
    is_focused: bool,
) {
    if state.records.is_empty() {
        let text =
            "No runs found.\nStart a training run to record your first entry.\nPress r to refresh.";
        let p = Paragraph::new(text)
            .style(Style::default().fg(palette.muted))
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(p, centered_text_area(area, text));
        return;
    }

    let header = Row::new(vec!["St", "Name", "Step", "Started", "Source"])
        .style(Style::default().add_modifier(Modifier::BOLD));

    // Use cached processed records if available, otherwise fall back to computing on-the-fly
    let rows: Vec<Row> = if !state.processed_records.is_empty()
        && state.processed_records.len() == state.records.len()
    {
        state
            .processed_records
            .iter()
            .enumerate()
            .map(|(i, processed)| {
                let is_selected = i == state.selected_idx;
                let mut style = Style::default();

                if is_selected && is_focused {
                    style = style.fg(palette.header_bg).bg(palette.accent);
                } else if is_selected {
                    style = style.add_modifier(Modifier::REVERSED);
                } else {
                    style = style.fg(palette.header_fg);
                }

                Row::new(vec![
                    Line::from(Span::styled(
                        processed.status_icon,
                        Style::default().fg(if is_selected {
                            style.fg.unwrap_or(processed.status_color)
                        } else {
                            processed.status_color
                        }),
                    )),
                    Line::from(processed.name.clone()),
                    Line::from(processed.step.clone()),
                    Line::from(processed.started.clone()),
                    Line::from(processed.source.clone()),
                ])
                .style(style)
            })
            .collect()
    } else {
        // Fallback to original implementation if cache is invalid
        state
            .records
            .iter()
            .enumerate()
            .map(|(i, rec)| {
                let (status_icon, status_color) = match rec.status {
                    RunStatus::Active => ("●", palette.success),
                    RunStatus::Completed => ("✓", palette.muted),
                    RunStatus::Failed => ("✗", palette.error),
                };

                let name = truncate(&run_display_name(rec), 20);
                let step = rec
                    .last_step
                    .map(format_step)
                    .unwrap_or_else(|| "-".to_string());
                let started = format_epoch_date(rec.started_at_epoch_secs);
                let source = rec.source_kind.as_str();

                let is_selected = i == state.selected_idx;
                let mut style = Style::default();

                if is_selected && is_focused {
                    style = style.fg(palette.header_bg).bg(palette.accent);
                } else if is_selected {
                    style = style.add_modifier(Modifier::REVERSED);
                } else {
                    style = style.fg(palette.header_fg);
                }

                Row::new(vec![
                    Line::from(Span::styled(
                        status_icon,
                        Style::default().fg(if is_selected {
                            style.fg.unwrap_or(status_color)
                        } else {
                            status_color
                        }),
                    )),
                    Line::from(name),
                    Line::from(step),
                    Line::from(started),
                    Line::from(source.to_string()),
                ])
                .style(style)
            })
            .collect()
    };

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
    .highlight_spacing(HighlightSpacing::Always);

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
        let p = Paragraph::new(text).style(Style::default().fg(palette.accent));
        frame.render_widget(p, area);
    } else if state.rename_active {
        let text = format!("Rename: {}|", state.rename_buffer);
        let p = Paragraph::new(text).style(Style::default().fg(palette.accent));
        frame.render_widget(p, area);
    } else if let Some(run_id) = state.pending_delete_run_id.as_deref() {
        let text = format!("Delete run {run_id}? Press Enter to confirm or Esc to cancel.");
        let p = Paragraph::new(text).style(Style::default().fg(palette.warning));
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

        let p = Paragraph::new(text).style(Style::default().fg(palette.muted));
        frame.render_widget(p, area);
    } else {
        let text =
            "  /: search   f: filter status   n: rename   d: delete   Enter: view selected run";
        let p = Paragraph::new(text).style(Style::default().fg(palette.muted));
        frame.render_widget(p, area);
    }
}

pub fn run_display_name(rec: &crate::store::types::RunRecord) -> String {
    if let Some(name) = rec.display_name.as_deref().map(str::trim)
        && !name.is_empty()
    {
        return name.to_string();
    }

    if let Some(source_locator) = rec.source_locator.as_deref()
        && !source_locator.is_empty()
    {
        let path = std::path::Path::new(source_locator);
        if let Some(file_name) = path.file_name().and_then(|name| name.to_str())
            && !file_name.is_empty()
        {
            return file_name.to_string();
        }
        return source_locator.to_string();
    }

    rec.run_id.chars().take(8).collect()
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
                render_runs_panel(frame, frame.area(), &app, true);
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
                render_runs_panel(frame, frame.area(), &app, true);
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
                render_runs_panel(frame, frame.area(), &app, true);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("Search: foo|"),
            "Expected search strip content"
        );
    }

    #[test]
    fn test_run_explorer_fallback_name_uses_source_filename() {
        let mut app = App::new(Default::default());
        app.ui_state.explorer.records.push(RunRecord {
            run_id: "run-id-123456".to_string(),
            source_fingerprint: "fingerprint".to_string(),
            source_kind: RunSourceKind::LogFile,
            source_locator: Some("/tmp/training-output.jsonl".to_string()),
            project_root: None,
            display_name: None,
            status: RunStatus::Completed,
            command: None,
            cwd: None,
            git_commit: None,
            git_dirty: None,
            started_at_epoch_secs: 1600000000,
            ended_at_epoch_secs: Some(1600000100),
            last_step: Some(100),
            last_updated_epoch_secs: 1600000100,
        });

        let mut terminal = make_terminal();
        terminal
            .draw(|frame| {
                render_runs_panel(frame, frame.area(), &app, true);
            })
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("training-output.jsonl"),
            "Expected source filename fallback name"
        );
    }
}
