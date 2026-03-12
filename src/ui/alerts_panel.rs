use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::app::{AlertLevel, AlertRecord};
use crate::ui::components::centered_text_area;
use crate::ui::theme::ThemePalette;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertPanelEntry {
    pub level: AlertLevel,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct AlertPanelData {
    pub active: Vec<AlertPanelEntry>,
    pub resolved: Vec<AlertPanelEntry>,
}

impl AlertPanelData {
    pub fn from_records(active: &[AlertRecord], resolved: &[AlertRecord]) -> Self {
        Self {
            active: active
                .iter()
                .map(|alert| AlertPanelEntry {
                    level: alert.level,
                    message: alert.message.clone(),
                })
                .collect(),
            resolved: resolved
                .iter()
                .map(|alert| AlertPanelEntry {
                    level: alert.level,
                    message: alert.message.clone(),
                })
                .collect(),
        }
    }
}

pub fn render_alert_panel(
    frame: &mut Frame,
    area: Rect,
    data: &AlertPanelData,
    palette: &ThemePalette,
    title: &str,
    is_focused: bool,
    active_limit: usize,
    resolved_limit: usize,
) {
    let mut title_style = Style::default();
    let mut border_style = Style::default();

    if is_focused {
        border_style = border_style.fg(palette.accent).add_modifier(Modifier::BOLD);
        title_style = title_style.fg(palette.accent).add_modifier(Modifier::BOLD);
    } else {
        border_style = border_style.fg(palette.muted);
        title_style = title_style.fg(palette.header_fg);
    }

    let block = Block::default()
        .title(title)
        .title_style(title_style)
        .borders(Borders::ALL)
        .border_style(border_style);

    if data.active.is_empty() && data.resolved.is_empty() {
        let message = "No alerts";
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let empty = Paragraph::new(message)
            .alignment(Alignment::Center)
            .style(Style::default().fg(palette.muted));
        frame.render_widget(empty, centered_text_area(inner, message));
        return;
    }

    let mut lines: Vec<Line> = Vec::new();

    for alert in data.active.iter().take(active_limit) {
        let (prefix, color) = match alert.level {
            AlertLevel::Critical => ("CRIT", palette.error),
            AlertLevel::Warning => ("WARN", palette.warning),
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("{prefix} "),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(&alert.message, Style::default().fg(color)),
        ]));
    }

    if !data.active.is_empty() && !data.resolved.is_empty() {
        lines.push(Line::from(Span::styled(
            "--- resolved ---",
            Style::default().fg(palette.muted),
        )));
    }

    for alert in data.resolved.iter().rev().take(resolved_limit) {
        lines.push(Line::from(Span::styled(
            &alert.message,
            Style::default().fg(palette.muted),
        )));
    }

    let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}
