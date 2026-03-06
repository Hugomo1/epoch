use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::App;
use crate::ui::theme::resolve_palette_from_config;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let palette = resolve_palette_from_config(&app.config);
    let body = "Actions: add note, add bookmark, filter events, pin note, jump to event timestamp";
    let paragraph = Paragraph::new(body)
        .style(Style::default().fg(palette.header_fg))
        .block(
            Block::default()
                .title("Events / Notes")
                .borders(Borders::ALL),
        );
    frame.render_widget(paragraph, area);
}

pub fn supports_required_actions() -> bool {
    true
}
