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

pub fn render(_frame: &mut Frame, _app: &App) {
    todo!()
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
}
