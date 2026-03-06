use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::App;
use crate::ui::theme::resolve_palette_from_config;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let palette = resolve_palette_from_config(&app.config);
    let mut lines = vec!["PID | Command | CWD | CPU | Memory".to_string()];
    for candidate in app.discovered_processes.iter().take(5) {
        lines.push(format!(
            "{} | {} | {} | {} | {}",
            candidate.pid,
            candidate.command,
            candidate.cwd.clone().unwrap_or_else(|| "-".to_string()),
            candidate.cpu_milli_percent,
            candidate.memory_bytes
        ));
    }

    let paragraph = Paragraph::new(lines.join("\n"))
        .style(Style::default().fg(palette.header_fg))
        .block(
            Block::default()
                .title("System / Processes")
                .borders(Borders::ALL),
        );
    frame.render_widget(paragraph, area);
}

pub fn required_columns() -> [&'static str; 5] {
    ["PID", "Command", "CWD", "CPU", "Memory"]
}
