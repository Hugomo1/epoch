use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};

use crate::app::{App, FilePickerInputMode, FilePickerState};
use crate::discovery::FileFormat;
use crate::ui::theme::resolve_palette_from_config;

pub fn render_scanning(frame: &mut Frame, area: Rect, scanning_frame: usize, app: &App) {
    let palette = resolve_palette_from_config(&app.config);
    let popup = centered_rect(70, 30, area);
    frame.render_widget(Clear, popup);

    let spinner = ['|', '/', '-', '\\'][scanning_frame % 4];

    let message = Paragraph::new(format!("{} Scanning for training files...", spinner))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .title("Discovery")
                .borders(Borders::ALL)
                .style(Style::default().fg(palette.header_fg)),
        );
    frame.render_widget(message, popup);
}

pub fn render_picker(frame: &mut Frame, area: Rect, state: &FilePickerState, app: &App) {
    let palette = resolve_palette_from_config(&app.config);
    let popup = centered_rect(85, 70, area);
    frame.render_widget(Clear, popup);

    let [query_area, list_area, footer_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .areas(popup);

    let mode_label = if app.config.keymap_profile == "vim" {
        match state.input_mode {
            FilePickerInputMode::Insert => "INSERT",
            FilePickerInputMode::Normal => "NORMAL",
        }
    } else {
        "INSERT"
    };

    let query = Paragraph::new(format!("Query [{mode_label}]: {}", state.query)).block(
        Block::default()
            .title("Select Training File")
            .borders(Borders::ALL)
            .style(Style::default().fg(palette.header_fg)),
    );
    frame.render_widget(query, query_area);

    let items = if state.filtered_indices.is_empty() {
        vec![ListItem::new("No matching files")]
    } else {
        state
            .filtered_indices
            .iter()
            .map(|index| {
                let file = &state.files[*index];
                let label = format!("{} [{}]", file.path.display(), format_label(file.format));
                ListItem::new(label)
            })
            .collect()
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title(format!("{} files", state.filtered_indices.len()))
                .borders(Borders::ALL)
                .style(Style::default().fg(palette.header_fg)),
        )
        .highlight_style(
            Style::default()
                .fg(palette.accent)
                .add_modifier(Modifier::BOLD),
        );

    let mut list_state = ratatui::widgets::ListState::default();
    if state.filtered_indices.is_empty() {
        list_state.select(None);
    } else {
        list_state.select(Some(state.selected_index));
    }
    frame.render_stateful_widget(list, list_area, &mut list_state);

    let footer_text = if app.config.keymap_profile == "vim" {
        match state.input_mode {
            FilePickerInputMode::Insert => {
                "i:insert | Esc:normal | Type to filter | Enter:open | q:quit"
            }
            FilePickerInputMode::Normal => {
                "i:insert | j/k or Up/Down:select | Enter:open | Esc/q:quit"
            }
        }
    } else {
        "Type to filter | Up/Down to navigate | Enter to continue | Esc/q to quit"
    };
    let footer = Paragraph::new(footer_text).style(Style::default().fg(palette.muted));
    frame.render_widget(footer, footer_area);
}

fn format_label(format: FileFormat) -> &'static str {
    match format {
        FileFormat::Jsonl => "jsonl",
        FileFormat::Csv => "csv",
        FileFormat::HfTrainerState => "trainer_state",
    }
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
