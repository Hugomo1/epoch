use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::widgets::Paragraph;

use crate::app::{App, MonitoringRoute};
use crate::ui::components::{format_duration, format_step};
use crate::ui::theme::resolve_palette_from_config;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let palette = resolve_palette_from_config(&app.config);

    let [nav_area, meta_area] =
        Layout::horizontal([Constraint::Min(0), Constraint::Fill(1)]).areas(area);

    let nav_meta = app
        .ui_state
        .monitoring
        .route
        .metadata(app.ui_state.monitoring.focused_panel);
    let nav_text = nav_meta.breadcrumb.unwrap_or(nav_meta.route_label);
    let nav_spans = if let Some(back_hint) = nav_meta.back_hint {
        ratatui::text::Line::from(vec![
            ratatui::text::Span::styled(
                nav_text,
                ratatui::style::Style::default()
                    .fg(palette.accent)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            ),
            ratatui::text::Span::styled(
                format!("  ({back_hint})"),
                ratatui::style::Style::default().fg(palette.muted),
            ),
        ])
    } else {
        ratatui::text::Line::from(ratatui::text::Span::styled(
            nav_text,
            ratatui::style::Style::default()
                .fg(palette.accent)
                .add_modifier(ratatui::style::Modifier::BOLD),
        ))
    };
    let nav = Paragraph::new(nav_spans).style(
        ratatui::style::Style::default()
            .fg(palette.header_fg)
            .bg(palette.header_bg),
    );
    frame.render_widget(nav, nav_area);

    let mut meta_parts = Vec::new();
    match app.ui_state.monitoring.route {
        MonitoringRoute::Home => {
            meta_parts.push(format!(
                "Focus {}:{}",
                app.home_focus_index(),
                app.home_focus_label()
            ));
            meta_parts.push(format!(
                "Visible runs {}",
                app.ui_state.explorer.records.len()
            ));
            meta_parts.push(format!("Active {}", app.active_run_count()));
        }
        MonitoringRoute::RunDetail => {
            meta_parts.push(format!(
                "Focus {}:{}",
                app.ui_state.focused_box,
                app.run_detail_focus_label()
            ));
            if let Some(step) = app.current_run_step() {
                meta_parts.push(format!("Step {}", format_step(step)));
            }
            if let Some(duration) = app.selected_run_elapsed() {
                meta_parts.push(format!("Run {}", format_duration(duration)));
            }
            meta_parts.push(if app.ui_state.graph_viewports[0].follow_latest {
                "Viewport live".to_string()
            } else {
                "Viewport paused".to_string()
            });
        }
    }
    meta_parts.push(format!("Keymap {}", app.config.keymap_profile));

    let meta = ratatui::text::Line::from(ratatui::text::Span::styled(
        meta_parts.join(" | "),
        ratatui::style::Style::default().fg(palette.muted),
    ));

    let meta = Paragraph::new(meta)
        .alignment(ratatui::layout::Alignment::Right)
        .style(
            ratatui::style::Style::default()
                .fg(palette.header_fg)
                .bg(palette.header_bg),
        );

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
    fn test_header_uses_breadcrumb_and_not_epoch_title() {
        let backend = TestBackend::new(80, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new(Config::default());
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer.cell((x, y)).unwrap().symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("\n");

        assert!(content.contains("Run Detail"));
        assert!(content.contains("Focus 1:Core"));
        assert!(!content.contains("epoch"));
    }

    #[test]
    fn test_header_shows_route_breadcrumb_for_run_detail() {
        let backend = TestBackend::new(120, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new(Config::default());

        terminal
            .draw(|frame| {
                render(frame, frame.area(), &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer.cell((x, y)).unwrap().symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("\n");

        assert!(content.contains("Home > Run Detail"));
        assert!(content.contains("Esc:back"));
    }
}
