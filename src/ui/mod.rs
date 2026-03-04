pub mod dashboard;
pub mod header;
pub mod metrics;
pub mod system;

use ratatui::Frame;
use ratatui::style::{Color, Modifier, Style};

use crate::app::App;

// Base palette — dark terminal friendly
pub const HEADER_BG: Color = Color::Rgb(30, 30, 46);
pub const HEADER_FG: Color = Color::Rgb(205, 214, 244);
pub const ACCENT: Color = Color::Rgb(137, 180, 250);
pub const SUCCESS: Color = Color::Green;
pub const WARNING: Color = Color::Yellow;
pub const ERROR: Color = Color::Red;
pub const MUTED: Color = Color::DarkGray;
pub const GPU_COLOR: Color = Color::Rgb(166, 227, 161);
pub const CPU_COLOR: Color = Color::Rgb(137, 180, 250);
pub const RAM_COLOR: Color = Color::Rgb(245, 194, 231);
pub const LOSS_COLOR: Color = Color::Rgb(250, 179, 135);
pub const LR_COLOR: Color = Color::Rgb(148, 226, 213);

// Minimum terminal size
pub const MIN_WIDTH: u16 = 60;
pub const MIN_HEIGHT: u16 = 20;

// Tab enum with strum derives
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::EnumIter, strum::FromRepr, strum::Display)]
pub enum Tab {
    #[strum(serialize = "Dashboard")]
    Dashboard = 0,
    #[strum(serialize = "Metrics")]
    Metrics = 1,
    #[strum(serialize = "System")]
    System = 2,
}

// Semantic style helper functions
pub fn header_style() -> Style {
    Style::default().fg(HEADER_FG).bg(HEADER_BG)
}

pub fn tab_active_style() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}

pub fn tab_inactive_style() -> Style {
    Style::default().fg(MUTED)
}

pub fn metric_label_style() -> Style {
    Style::default().fg(HEADER_FG).add_modifier(Modifier::BOLD)
}

pub fn metric_value_style() -> Style {
    Style::default().fg(ACCENT)
}

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Minimum size check
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        let msg = format!("Terminal too small (need {}×{})", MIN_WIDTH, MIN_HEIGHT);
        let paragraph =
            ratatui::widgets::Paragraph::new(msg).alignment(ratatui::layout::Alignment::Center);
        // Center vertically
        let vertical = ratatui::layout::Layout::vertical([
            ratatui::layout::Constraint::Fill(1),
            ratatui::layout::Constraint::Length(1),
            ratatui::layout::Constraint::Fill(1),
        ])
        .split(area);
        frame.render_widget(paragraph, vertical[1]);
        return;
    }

    // Main layout: header (3) | content (fill) | status bar (1)
    let [header_area, content_area, status_area] = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Length(3),
        ratatui::layout::Constraint::Min(0),
        ratatui::layout::Constraint::Length(1),
    ])
    .areas(area);

    // Render header
    header::render(frame, header_area, app);

    // Render active tab content
    match app.ui_state.selected_tab {
        Tab::Dashboard => dashboard::render(frame, content_area, app),
        Tab::Metrics => metrics::render(frame, content_area, app),
        Tab::System => system::render(frame, content_area, app),
    }

    // Render status bar
    render_status_bar(frame, status_area, app);
}

fn render_status_bar(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let status = if app.running { "Running" } else { "Stopped" };
    let data_status = if app.training.input_active {
        "Live"
    } else if app.training.last_data_at.is_some() {
        "Stale"
    } else {
        "No data"
    };
    let elapsed = app.elapsed();
    let hours = elapsed.as_secs() / 3600;
    let minutes = (elapsed.as_secs() % 3600) / 60;
    let seconds = elapsed.as_secs() % 60;

    let text = format!(
        " [{}] | Data: {} | Elapsed: {:02}:{:02}:{:02} | q to quit",
        status, data_status, hours, minutes, seconds
    );

    let paragraph = ratatui::widgets::Paragraph::new(text)
        .style(ratatui::style::Style::default().fg(HEADER_FG).bg(HEADER_BG));
    frame.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use strum::IntoEnumIterator;

    #[test]
    fn test_tab_from_repr_dashboard() {
        assert_eq!(Tab::from_repr(0), Some(Tab::Dashboard));
    }

    #[test]
    fn test_tab_from_repr_metrics() {
        assert_eq!(Tab::from_repr(1), Some(Tab::Metrics));
    }

    #[test]
    fn test_tab_from_repr_system() {
        assert_eq!(Tab::from_repr(2), Some(Tab::System));
    }

    #[test]
    fn test_tab_from_repr_invalid() {
        assert_eq!(Tab::from_repr(3), None);
    }

    #[test]
    fn test_tab_iteration_count() {
        let tabs: Vec<Tab> = Tab::iter().collect();
        assert_eq!(tabs.len(), 3);
    }

    #[test]
    fn test_tab_iteration_contains_all() {
        let tabs: Vec<Tab> = Tab::iter().collect();
        assert!(tabs.contains(&Tab::Dashboard));
        assert!(tabs.contains(&Tab::Metrics));
        assert!(tabs.contains(&Tab::System));
    }

    #[test]
    fn test_tab_display_dashboard() {
        assert_eq!(Tab::Dashboard.to_string(), "Dashboard");
    }

    #[test]
    fn test_tab_display_metrics() {
        assert_eq!(Tab::Metrics.to_string(), "Metrics");
    }

    #[test]
    fn test_tab_display_system() {
        assert_eq!(Tab::System.to_string(), "System");
    }

    #[test]
    fn test_style_functions_return_non_default() {
        let header = header_style();
        assert_ne!(header, Style::default());

        let active = tab_active_style();
        assert_ne!(active, Style::default());

        let inactive = tab_inactive_style();
        assert_ne!(inactive, Style::default());

        let label = metric_label_style();
        assert_ne!(label, Style::default());

        let value = metric_value_style();
        assert_ne!(value, Style::default());
    }

    #[test]
    fn test_min_dimensions_exist() {
        assert!(MIN_WIDTH > 0);
        assert!(MIN_HEIGHT > 0);
        assert_eq!(MIN_WIDTH, 60);
        assert_eq!(MIN_HEIGHT, 20);
    }

    #[test]
    fn test_color_constants_exist() {
        let _ = HEADER_BG;
        let _ = HEADER_FG;
        let _ = ACCENT;
        let _ = SUCCESS;
        let _ = WARNING;
        let _ = ERROR;
        let _ = MUTED;
        let _ = GPU_COLOR;
        let _ = CPU_COLOR;
        let _ = RAM_COLOR;
        let _ = LOSS_COLOR;
        let _ = LR_COLOR;
    }

    use crate::config::Config;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn test_render_minimum_size() {
        let backend = TestBackend::new(60, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new(Config::default());
        terminal
            .draw(|frame| {
                render(frame, &app);
            })
            .unwrap();
    }

    #[test]
    fn test_render_too_small() {
        let backend = TestBackend::new(40, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new(Config::default());
        terminal
            .draw(|frame| {
                render(frame, &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let mut found = false;
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                if buffer.cell((x, y)).unwrap().symbol() == "T"
                    && buffer.cell((x + 1, y)).unwrap().symbol() == "e"
                    && buffer.cell((x + 2, y)).unwrap().symbol() == "r"
                {
                    found = true;
                    break;
                }
            }
        }
        assert!(found);
    }

    #[test]
    fn test_render_status_bar_content() {
        let backend = TestBackend::new(80, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.running = true;
        terminal
            .draw(|frame| {
                render_status_bar(frame, frame.area(), &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let mut found = false;
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                if buffer.cell((x, y)).unwrap().symbol() == "R"
                    && buffer.cell((x + 1, y)).unwrap().symbol() == "u"
                    && buffer.cell((x + 2, y)).unwrap().symbol() == "n"
                {
                    found = true;
                    break;
                }
            }
        }
        assert!(found);
    }
}
