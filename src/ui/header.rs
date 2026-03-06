use ratatui::Frame;
use ratatui::layout::Rect;

use crate::app::App;
use crate::ui::theme::resolve_palette_from_config;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let palette = resolve_palette_from_config(&app.config);

    let [title_area, tabs_area] = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Length(2),
    ])
    .areas(area);

    // Title with elapsed time right-aligned
    let elapsed = app.elapsed();
    let hours = elapsed.as_secs() / 3600;
    let minutes = (elapsed.as_secs() % 3600) / 60;
    let seconds = elapsed.as_secs() % 60;
    let title_text = ratatui::text::Line::from(vec![
        ratatui::text::Span::styled(
            "Epoch",
            ratatui::style::Style::default()
                .fg(palette.accent)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ),
        ratatui::text::Span::raw(" "),
        ratatui::text::Span::styled(
            format!("{:02}:{:02}:{:02}", hours, minutes, seconds),
            ratatui::style::Style::default().fg(palette.muted),
        ),
    ]);
    let title_paragraph = ratatui::widgets::Paragraph::new(title_text).style(
        ratatui::style::Style::default()
            .fg(palette.header_fg)
            .bg(palette.header_bg),
    );
    frame.render_widget(title_paragraph, title_area);

    let [views_area, subtabs_area] = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Length(1),
        ratatui::layout::Constraint::Length(1),
    ])
    .areas(tabs_area);

    let view_titles = vec![
        "[5] Home",
        "[6] Live Run",
        "[7] Run Explorer",
        "[8] Events/Notes",
        "[9] System/Processes",
    ];
    let view_index = match app.ui_state.primary_view {
        crate::app::PrimaryView::Home => 0,
        crate::app::PrimaryView::LiveRun => 1,
        crate::app::PrimaryView::RunExplorer => 2,
        crate::app::PrimaryView::EventsNotes => 3,
        crate::app::PrimaryView::SystemProcesses => 4,
    };
    let views = ratatui::widgets::Tabs::new(view_titles)
        .select(view_index)
        .highlight_style(
            ratatui::style::Style::default()
                .fg(palette.accent)
                .add_modifier(ratatui::style::Modifier::BOLD),
        )
        .style(ratatui::style::Style::default().fg(palette.muted))
        .divider(ratatui::text::Span::raw(" | "));
    frame.render_widget(views, views_area);

    let tab_titles = vec!["[1] Main", "[2] Diagnostics"];
    let tabs = ratatui::widgets::Tabs::new(tab_titles)
        .select(app.ui_state.selected_tab as usize)
        .highlight_style(
            ratatui::style::Style::default()
                .fg(palette.accent)
                .add_modifier(ratatui::style::Modifier::BOLD),
        )
        .style(ratatui::style::Style::default().fg(palette.muted))
        .divider(ratatui::text::Span::raw(" | "));
    frame.render_widget(tabs, subtabs_area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn test_header_render_no_panic() {
        let backend = TestBackend::new(80, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new(Config::default());
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &app);
            })
            .unwrap();
    }

    #[test]
    fn test_header_shows_title() {
        let backend = TestBackend::new(80, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new(Config::default());
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        // check if buffer contains Epoch
        let mut found = false;
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                if buffer.cell((x, y)).unwrap().symbol() == "E"
                    && buffer.cell((x + 1, y)).unwrap().symbol() == "p"
                {
                    found = true;
                    break;
                }
            }
        }
        assert!(found);
    }
}
