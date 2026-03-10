use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Row, Table, TableState},
};

use crate::app::App;
use crate::collectors::process::ProbeStatus;
use crate::store::types::system_processes_columns;
use crate::ui::components::{format_bytes, truncate};
use crate::ui::theme::{ThemePalette, resolve_palette_from_config};
use crate::ui::{CPU_COLOR, GPU_COLOR, RAM_COLOR};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let palette = resolve_palette_from_config(&app.config);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(0)])
        .split(area);

    render_resource_strip(frame, chunks[0], app, &palette);
    render_processes_table(frame, chunks[1], app, &palette, false);
}

pub fn render_resource_strip(frame: &mut Frame, area: Rect, app: &App, palette: &ThemePalette) {
    let block = Block::default()
        .title(" System Resources ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(palette.muted));

    let cpu_pct = app
        .system
        .latest
        .as_ref()
        .map(|s| s.cpu_usage_percent())
        .unwrap_or(0.0);
    let ram_pct = app
        .system
        .latest
        .as_ref()
        .map(|s| s.memory_usage_percent())
        .unwrap_or(0.0);

    let gpu_util = app
        .system
        .latest
        .as_ref()
        .and_then(|s| s.gpus.first())
        .map(|g| g.utilization);

    let has_gpu = app
        .system
        .latest
        .as_ref()
        .map(|s| !s.gpus.is_empty())
        .unwrap_or(false);

    let cpu_history = app.system_viewport_series(&app.system.cpu_history, 20);
    let ram_history = app.system_viewport_series(&app.system.ram_history, 20);
    let gpu_history = app.system_viewport_series(&app.system.gpu_history, 20);

    let cpu_line = make_resource_line("CPU", cpu_pct, &cpu_history, CPU_COLOR);
    let ram_line = make_resource_line("RAM", ram_pct, &ram_history, RAM_COLOR);
    let gpu_line = if has_gpu {
        make_resource_line("GPU", gpu_util.unwrap_or(0.0), &gpu_history, GPU_COLOR)
    } else {
        Line::from(vec![Span::raw(" GPU  [────────────────────] N/A")])
            .style(Style::default().fg(palette.muted))
    };

    let p = Paragraph::new(vec![cpu_line, ram_line, gpu_line])
        .block(block)
        .style(Style::default().fg(palette.header_fg));
    frame.render_widget(p, area);
}

fn make_resource_line(
    label: &str,
    pct: f64,
    history: &[u64],
    color: ratatui::style::Color,
) -> Line<'static> {
    let pct_clamped = pct.clamp(0.0, 100.0);
    let filled = (pct_clamped / 100.0 * 20.0).round() as usize;
    let filled = filled.min(20);
    let empty = 20usize.saturating_sub(filled);

    let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));

    let sparkline_chars = " ▁▂▃▄▅▆▇█";
    let mut spark = String::new();

    let tail = if history.len() > 20 {
        &history[history.len() - 20..]
    } else {
        history
    };

    for &val in tail {
        let val = val.min(100);
        let idx = (val * 8 / 100) as usize;
        spark.push(sparkline_chars.chars().nth(idx).unwrap_or(' '));
    }

    Line::from(vec![
        Span::styled(
            format!(" {:<4} [{}] {:>5.1}%  ", label, bar, pct),
            Style::default().fg(color),
        ),
        Span::styled(spark, Style::default().fg(color)),
    ])
}

pub fn render_processes_table(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    palette: &ThemePalette,
    is_focused: bool,
) {
    let count = app.discovered_processes.len();
    let title = if count > 0 {
        format!(" ⚡ Processes ({}) ", count)
    } else {
        " ⚡ Processes ".to_string()
    };

    let mut border_style = Style::default();
    let mut title_style = Style::default();

    if is_focused {
        border_style = border_style.fg(palette.accent).add_modifier(Modifier::BOLD);
        title_style = title_style.fg(palette.accent).add_modifier(Modifier::BOLD);
    } else if count > 0 {
        border_style = border_style.fg(palette.warning);
        title_style = title_style.fg(palette.header_fg);
    } else {
        border_style = border_style.fg(palette.muted);
        title_style = title_style.fg(palette.header_fg);
    }

    let block = Block::default()
        .title(title)
        .title_style(title_style)
        .borders(Borders::ALL)
        .border_style(border_style);

    if count == 0 {
        let p = Paragraph::new(
            "No training processes detected.\nEpoch monitors Python training processes automatically.",
        )
        .block(block)
        .style(Style::default().fg(palette.muted))
        .alignment(Alignment::Center);
        frame.render_widget(p, area);
        return;
    }

    let mut rows = Vec::new();
    for candidate in app.discovered_processes.iter().take(20) {
        let status = match candidate.status {
            ProbeStatus::Ok => "OK",
            ProbeStatus::PermissionDenied => "⊘",
            ProbeStatus::Gone => "⊗",
        };
        let cwd = candidate.cwd.as_deref().unwrap_or("-");

        rows.push(Row::new(vec![
            candidate.pid.to_string(),
            truncate(&candidate.command, 50).to_string(),
            truncate(cwd, 20).to_string(),
            format!("{:.1}%", candidate.cpu_milli_percent as f64 / 10.0),
            format_bytes(candidate.memory_bytes),
            status.to_string(),
        ]));
    }

    let header = Row::new(vec!["PID", "Command", "CWD", "CPU", "Memory", "Status"]).style(
        Style::default()
            .fg(palette.header_fg)
            .add_modifier(Modifier::BOLD),
    );

    let widths = [
        Constraint::Length(6),
        Constraint::Min(0),
        Constraint::Length(20),
        Constraint::Length(8),
        Constraint::Length(10),
        Constraint::Length(8),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(
            Style::default()
                .bg(palette.header_bg)
                .fg(palette.accent)
                .add_modifier(Modifier::BOLD),
        );

    let mut state = TableState::default();
    let selected_idx = app
        .ui_state
        .monitoring
        .selected_pid
        .and_then(|pid| app.discovered_processes.iter().position(|p| p.pid == pid));
    state.select(selected_idx);

    frame.render_stateful_widget(table, area, &mut state);
}

pub fn required_columns() -> [&'static str; 5] {
    system_processes_columns()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn make_terminal() -> Terminal<TestBackend> {
        Terminal::new(TestBackend::new(120, 40)).unwrap()
    }

    #[test]
    fn test_system_processes_renders_without_panic() {
        let app = App::new(Default::default());
        let mut terminal = make_terminal();
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &app);
            })
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let content: String = buffer.content().iter().map(|c| c.symbol()).collect();
        assert!(content.contains("CPU"), "Expected 'CPU' in buffer");
        assert!(content.contains("GPU  [────────────────────] N/A"));
        assert!(content.contains("No training processes detected"));
    }
}
