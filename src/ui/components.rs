use ratatui::Frame;
use ratatui::layout::Alignment;
use ratatui::layout::Constraint;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Paragraph};
use std::collections::VecDeque;
use std::time::Duration;

use crate::ui::theme::ThemePalette;

pub fn render_empty_state(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    message: &str,
    palette: &ThemePalette,
) {
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(palette.muted));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let centered = centered_text_area(inner, message);
    let paragraph = Paragraph::new(message)
        .style(Style::default().fg(palette.muted))
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, centered);
}

pub fn centered_text_area(area: Rect, text: &str) -> Rect {
    let line_count = text.lines().count().max(1) as u16;
    let content_height = line_count.min(area.height.max(1));
    let [_, centered, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(content_height),
        Constraint::Fill(1),
    ])
    .areas(area);
    centered
}

pub fn render_action_bar(frame: &mut Frame, area: Rect, actions: &str, palette: &ThemePalette) {
    let block = Block::default()
        .title("Actions")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(palette.header_fg));
    let paragraph = Paragraph::new(actions)
        .block(block)
        .style(Style::default().fg(palette.accent))
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

pub fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.1} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

pub fn format_step(step: u64) -> String {
    if step < 10000 {
        let s = step.to_string();
        let mut result = String::new();
        for (count, c) in s.chars().rev().enumerate() {
            if count != 0 && count % 3 == 0 {
                result.push(',');
            }
            result.push(c);
        }
        result.chars().rev().collect()
    } else if step < 1_000_000 {
        format!("{:.1}k", step as f64 / 1000.0)
    } else {
        format!("{:.1}M", step as f64 / 1_000_000.0)
    }
}

pub fn format_epoch_date(secs: i64) -> String {
    let days = secs / 86400;
    let sec_in_day = secs % 86400;

    // Naive leap year/month calculation for display purposes
    let year = 1970 + days / 365;
    let mut day_of_year = days % 365;

    let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
    let mut days_in_month = vec![31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    if is_leap {
        days_in_month[1] = 29;
    }

    let mut month = 0;
    for &d in &days_in_month {
        if day_of_year < d {
            break;
        }
        day_of_year -= d;
        month += 1;
    }

    let month_names = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let m_str = month_names.get(month as usize).unwrap_or(&"Jan");
    let d = day_of_year + 1; // 1-indexed

    let h = sec_in_day / 3600;
    let m = (sec_in_day % 3600) / 60;

    format!("{} {:02} {:02}:{:02}", m_str, d, h, m)
}

pub fn format_duration(duration: Duration) -> String {
    let hours = duration.as_secs() / 3600;
    let minutes = (duration.as_secs() % 3600) / 60;
    let seconds = duration.as_secs() % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

pub fn trend_indicator(history: &VecDeque<u64>) -> &'static str {
    if history.len() < 2 {
        return "→";
    }

    let Some(&last) = history.back() else {
        return "→";
    };
    let last = last as f64;
    let count = (history.len() - 1).min(10);

    let sum: u64 = history.iter().rev().skip(1).take(count).sum();
    let avg = sum as f64 / count as f64;

    if last > avg * 1.01 {
        "↑"
    } else if last < avg * 0.99 {
        "↓"
    } else {
        "→"
    }
}

pub fn format_lr_value(lr: f64) -> String {
    format!("{:.1e}", lr)
}

pub fn format_optional_float(value: Option<f64>, decimals: usize) -> String {
    match value {
        Some(v) => format!("{v:.decimals$}"),
        None => "—".to_string(),
    }
}
