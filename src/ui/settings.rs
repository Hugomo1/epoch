use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::SettingsState;
use crate::ui::theme::resolve_palette_from_config;

pub fn render(frame: &mut Frame, area: Rect, state: &SettingsState) {
    let palette = resolve_palette_from_config(&state.draft);
    let popup = centered_rect(72, 78, area);
    frame.render_widget(Clear, popup);

    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(1),
    ])
    .split(popup);

    let parser = row_text(
        0,
        state.selected_row,
        "Parser",
        &state.draft.parser,
        &palette,
    );
    frame.render_widget(parser, chunks[0]);

    let theme = row_text(1, state.selected_row, "Theme", &state.draft.theme, &palette);
    frame.render_widget(theme, chunks[1]);

    let graph_mode = row_text(
        2,
        state.selected_row,
        "Graph Mode",
        &state.draft.graph_mode,
        &palette,
    );
    frame.render_widget(graph_mode, chunks[2]);

    let adaptive_layout = row_text(
        3,
        state.selected_row,
        "Adaptive Layout",
        if state.draft.adaptive_layout {
            "on"
        } else {
            "off"
        },
        &palette,
    );
    frame.render_widget(adaptive_layout, chunks[3]);

    let pinned_label = pinned_rate_metric_label(&state.draft.pinned_metrics);
    let pinned_metric = row_text(
        4,
        state.selected_row,
        "Pinned Rate Metric",
        &pinned_label,
        &palette,
    );
    frame.render_widget(pinned_metric, chunks[4]);

    let keymap = row_text(
        5,
        state.selected_row,
        "Keymap",
        &state.draft.keymap_profile,
        &palette,
    );
    frame.render_widget(keymap, chunks[5]);

    let target = row_text(
        6,
        state.selected_row,
        "Profile Target",
        &state.draft.profile_target,
        &palette,
    );
    frame.render_widget(target, chunks[6]);

    let footer = Paragraph::new(
        "Up/Down: select | Left/Right: change | a: apply | w/Enter: save | Esc: cancel",
    )
    .alignment(Alignment::Center)
    .style(Style::default().fg(palette.muted))
    .block(Block::default().title("Settings").borders(Borders::ALL));
    frame.render_widget(footer, chunks[7]);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);

    let horizontal = Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vertical[1]);

    horizontal[1]
}

fn pinned_rate_metric_label(pinned_metrics: &[String]) -> String {
    let tokens = pinned_metrics.iter().any(|m| m == "tokens_per_second");
    let samples = pinned_metrics.iter().any(|m| m == "samples_per_second");
    let steps = pinned_metrics.iter().any(|m| m == "steps_per_second");

    match (tokens, samples, steps) {
        (false, false, false) => "none".to_string(),
        (true, false, false) => "tokens".to_string(),
        (false, true, false) => "samples".to_string(),
        (false, false, true) => "steps".to_string(),
        (true, true, true) => "all".to_string(),
        _ => "mixed".to_string(),
    }
}

fn row_text(
    row: usize,
    selected_row: usize,
    label: &str,
    value: &str,
    palette: &crate::ui::theme::ThemePalette,
) -> Paragraph<'static> {
    let marker = if row == selected_row { ">" } else { " " };
    Paragraph::new(format!("{marker} {label}: {value}"))
        .style(Style::default().fg(if row == selected_row {
            palette.accent
        } else {
            palette.header_fg
        }))
        .block(Block::default().borders(Borders::ALL))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::config::Config;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn test_settings_render_contains_profile_and_theme_controls() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).expect("terminal should be created");
        let mut app = App::new(Config::default());
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('s'),
            crossterm::event::KeyModifiers::NONE,
        ));

        let state = match &app.ui_state.mode {
            crate::app::AppMode::Settings(state) => state,
            _ => panic!("expected settings mode"),
        };

        terminal
            .draw(|frame| {
                render(frame, frame.area(), state);
            })
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

        assert!(content.contains("Settings"));
        assert!(content.contains("Theme"));
        assert!(content.contains("Adaptive Layout"));
        assert!(content.contains("Pinned Rate Metric"));
        assert!(content.contains("Profile Target"));
    }
}
