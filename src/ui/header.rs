use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::widgets::{Paragraph, Tabs};

use crate::app::App;
use crate::ui::theme::resolve_palette_from_config;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let palette = resolve_palette_from_config(&app.config);

    let [info_area, views_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(area);

    let view_titles: Vec<&str> = vec!["Home", "Live Run", "Run Explorer", "System/Processes"];

    let views = Tabs::new(view_titles)
        .select(app.ui_state.primary_view.index())
        .highlight_style(
            ratatui::style::Style::default()
                .fg(palette.accent)
                .add_modifier(ratatui::style::Modifier::BOLD),
        )
        .style(ratatui::style::Style::default().fg(palette.muted))
        .divider(ratatui::text::Span::raw(" | "));
    frame.render_widget(views, views_area);

    let elapsed = app.elapsed();
    let hours = elapsed.as_secs() / 3600;
    let minutes = (elapsed.as_secs() % 3600) / 60;
    let seconds = elapsed.as_secs() % 60;

    let data_health = app.training_data_health_state();
    let viewport_status = if app.ui_state.graph_viewports[0].follow_latest {
        "LIVE"
    } else {
        "PAUSED"
    };

    let [title_area, meta_area] = Layout::horizontal([
        Constraint::Length(5), // "epoch"
        Constraint::Min(0),
    ])
    .areas(info_area);

    let title = Paragraph::new("epoch").style(
        ratatui::style::Style::default()
            .fg(palette.accent)
            .bg(palette.header_bg)
            .add_modifier(ratatui::style::Modifier::BOLD),
    );

    let meta = ratatui::text::Line::from(vec![
        ratatui::text::Span::styled(
            format!("{} ", viewport_status),
            ratatui::style::Style::default().fg(if app.ui_state.graph_viewports[0].follow_latest {
                palette.success
            } else {
                palette.warning
            }),
        ),
        ratatui::text::Span::styled(
            format!(
                "| Data: {} | Parser: {} | Keymap: {} | Elapsed: {:02}:{:02}:{:02}",
                data_health.label(),
                app.config.parser,
                app.config.keymap_profile,
                hours,
                minutes,
                seconds,
            ),
            ratatui::style::Style::default().fg(palette.muted),
        ),
    ]);

    let meta = Paragraph::new(meta)
        .alignment(ratatui::layout::Alignment::Right)
        .style(
            ratatui::style::Style::default()
                .fg(palette.header_fg)
                .bg(palette.header_bg),
        );

    frame.render_widget(title, title_area);
    frame.render_widget(meta, meta_area);
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
        let mut found = false;
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                if buffer.cell((x, y)).unwrap().symbol() == "e"
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
