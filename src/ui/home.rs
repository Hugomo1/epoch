use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::App;
use crate::home::service::default_actions;
use crate::ui::theme::resolve_palette_from_config;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let palette = resolve_palette_from_config(&app.config);
    let sections = home_sections();
    let actions = default_actions()
        .into_iter()
        .map(|action| {
            if action.enabled {
                action.label
            } else {
                format!(
                    "{} ({})",
                    action.label,
                    action.disabled_reason.unwrap_or_default()
                )
            }
        })
        .collect::<Vec<_>>()
        .join(" | ");

    let content = format!(
        "{}\n\nQuick Actions:\n{}\n\nCurrent parser: {}",
        sections.join("\n"),
        actions,
        app.config.parser
    );

    let paragraph = Paragraph::new(content)
        .alignment(Alignment::Left)
        .style(Style::default().fg(palette.header_fg))
        .block(Block::default().title("Home").borders(Borders::ALL));
    frame.render_widget(paragraph, area);
}

pub fn home_sections() -> Vec<&'static str> {
    vec![
        "Active Runs",
        "Recent Runs",
        "Recent Projects",
        "Alerts Needing Attention",
        "Available Checkpoints",
        "Discovered Processes",
    ]
}
