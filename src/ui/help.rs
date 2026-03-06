use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::HelpState;
use crate::ui::theme::resolve_palette_from_theme_and_custom;

pub fn render(frame: &mut Frame, area: Rect, state: &HelpState) {
    let palette = resolve_palette_from_theme_and_custom(&state.theme, state.custom_theme.as_ref());

    let chunks = Layout::vertical([Constraint::Min(0), Constraint::Length(2)]).split(area);

    let mut lines = vec!["Key Bindings".to_string(), "".to_string()];
    for (key, desc) in &state.entries {
        lines.push(format!("{key:<18} {desc}"));
    }

    let body = Paragraph::new(lines.join("\n"))
        .block(Block::default().title("Help").borders(Borders::ALL))
        .style(Style::default().fg(palette.header_fg));
    frame.render_widget(body, chunks[0]);

    let footer = Paragraph::new("Press ? or Esc to close")
        .alignment(Alignment::Center)
        .style(Style::default().fg(palette.muted));
    frame.render_widget(footer, chunks[1]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::config::Config;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn test_help_overlay_renders_active_keymap_bindings() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("terminal should be created");
        let mut app = App::new(Config::default());
        app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT));

        let state = match &app.ui_state.mode {
            crate::app::AppMode::Help(state) => state,
            _ => panic!("expected help mode"),
        };

        terminal
            .draw(|frame| render(frame, frame.area(), state))
            .expect("render should succeed");

        let buffer = terminal.backend().buffer();
        let content = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| {
                        buffer
                            .cell((x, y))
                            .expect("cell should exist")
                            .symbol()
                            .to_string()
                    })
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("\n");

        assert!(content.contains("Key Bindings"));
        assert!(content.contains("Toggle help overlay"));
        assert!(content.contains("Press ? or Esc to close"));
    }

    #[test]
    fn test_help_overlay_uses_keymap_source_of_truth() {
        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).expect("terminal should be created");
        let state = HelpState {
            entries: vec![("x".to_string(), "Custom Action".to_string())],
            theme: "classic".to_string(),
            custom_theme: None,
        };

        terminal
            .draw(|frame| render(frame, frame.area(), &state))
            .expect("render should succeed");

        let buffer = terminal.backend().buffer();
        let content = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| {
                        buffer
                            .cell((x, y))
                            .expect("cell should exist")
                            .symbol()
                            .to_string()
                    })
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("\n");

        assert!(content.contains("Custom Action"));
    }

    #[test]
    fn test_help_overlay_matches_runtime_keymap_contract() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("terminal should be created");
        let mut app = App::new(Config::default());
        app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT));

        let state = match &app.ui_state.mode {
            crate::app::AppMode::Help(state) => state,
            _ => panic!("expected help mode"),
        };

        terminal
            .draw(|frame| render(frame, frame.area(), state))
            .expect("render should succeed");

        let buffer = terminal.backend().buffer();
        let content = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| {
                        buffer
                            .cell((x, y))
                            .expect("cell should exist")
                            .symbol()
                            .to_string()
                    })
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("\n");

        assert!(content.contains("1/2 (3/4 legacy)"));
        assert!(content.contains("- / ="));
    }
}
