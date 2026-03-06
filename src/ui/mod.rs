pub mod advanced;
pub mod dashboard;
pub mod file_picker;
pub mod header;
pub mod help;
pub mod metrics;
pub mod settings;
pub mod system;
pub mod theme;

use ratatui::Frame;
use ratatui::style::{Color, Modifier, Style};

use crate::app::{App, AppMode};
use crate::ui::theme::resolve_palette_from_config;

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
    #[strum(serialize = "Advanced")]
    Advanced = 3,
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

    match &app.ui_state.mode {
        AppMode::Scanning => {
            file_picker::render_scanning(frame, content_area, app.ui_state.scanning_frame, app)
        }
        AppMode::FilePicker(state) => file_picker::render_picker(frame, content_area, state, app),
        AppMode::Help(state) => help::render(frame, content_area, state),
        AppMode::Settings(state) => settings::render(frame, content_area, state),
        AppMode::Monitoring => match app.ui_state.selected_tab {
            Tab::Dashboard => dashboard::render(frame, content_area, app),
            Tab::Metrics => metrics::render(frame, content_area, app),
            Tab::System => system::render(frame, content_area, app),
            Tab::Advanced => advanced::render(frame, content_area, app),
        },
    }

    // Render status bar
    render_status_bar(frame, status_area, app);
}

fn render_status_bar(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let palette = resolve_palette_from_config(&app.config);
    let status = if app.running { "Running" } else { "Stopped" };
    let data_health = app.training_data_health_state();
    let data_status = data_health.label();
    let viewport_status = if app.ui_state.training_viewport.follow_latest {
        "LIVE"
    } else {
        "PAUSED"
    };
    let elapsed = app.elapsed();
    let hours = elapsed.as_secs() / 3600;
    let minutes = (elapsed.as_secs() % 3600) / 60;
    let seconds = elapsed.as_secs() % 60;
    let key_hints = match &app.ui_state.mode {
        AppMode::Monitoring => "?:help s:settings Tab:tabs Space:live/pause q:quit",
        AppMode::Settings(_) => {
            "?:help Up/Down:row Left/Right:change a:apply w/Enter:save Esc:cancel"
        }
        AppMode::Help(_) => "?:close Esc:close",
        AppMode::FilePicker(state) => {
            if app.config.keymap_profile == "vim" {
                match state.input_mode {
                    crate::app::FilePickerInputMode::Insert => {
                        "Picker[INSERT] Esc:normal Type:filter Enter:open q:quit"
                    }
                    crate::app::FilePickerInputMode::Normal => {
                        "Picker[NORMAL] i:insert j/k:select Enter:open Esc/q:quit"
                    }
                }
            } else {
                "Type:filter Up/Down:select Enter:open Esc:quit"
            }
        }
        AppMode::Scanning => "Scanning files...",
    };

    let text = format!(
        " [{}] | {} | Parser: {} | Keymap: {} | Data: {} | Elapsed: {:02}:{:02}:{:02} | {}",
        status,
        viewport_status,
        app.config.parser,
        app.config.keymap_profile,
        data_status,
        hours,
        minutes,
        seconds,
        key_hints
    );

    let paragraph = ratatui::widgets::Paragraph::new(text).style(
        ratatui::style::Style::default()
            .fg(palette.header_fg)
            .bg(palette.header_bg),
    );
    frame.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::palette_for_name;
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
        assert_eq!(Tab::from_repr(4), None);
    }

    #[test]
    fn test_tab_iteration_count() {
        let tabs: Vec<Tab> = Tab::iter().collect();
        assert_eq!(tabs.len(), 4);
    }

    #[test]
    fn test_tab_iteration_contains_all() {
        let tabs: Vec<Tab> = Tab::iter().collect();
        assert!(tabs.contains(&Tab::Dashboard));
        assert!(tabs.contains(&Tab::Metrics));
        assert!(tabs.contains(&Tab::System));
        assert!(tabs.contains(&Tab::Advanced));
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
    fn test_tab_display_advanced() {
        assert_eq!(Tab::Advanced.to_string(), "Advanced");
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

    #[test]
    fn test_theme_registry_contains_required_presets() {
        assert!(crate::ui::theme::BUILTIN_THEMES.contains(&"classic"));
        assert!(crate::ui::theme::BUILTIN_THEMES.contains(&"catppuccin"));
        assert!(crate::ui::theme::BUILTIN_THEMES.contains(&"github"));
        assert!(crate::ui::theme::BUILTIN_THEMES.contains(&"nord"));
        assert!(crate::ui::theme::BUILTIN_THEMES.contains(&"gruvbox"));
        assert!(crate::ui::theme::BUILTIN_THEMES.contains(&"solarized"));
        assert!(crate::ui::theme::BUILTIN_THEMES.contains(&"dracula"));
    }

    #[test]
    fn test_default_theme_matches_legacy_classic_palette() {
        let palette = palette_for_name("classic");
        assert_eq!(palette.header_bg, HEADER_BG);
        assert_eq!(palette.header_fg, HEADER_FG);
        assert_eq!(palette.accent, ACCENT);
        assert_eq!(palette.loss_color, LOSS_COLOR);
        assert_eq!(palette.lr_color, LR_COLOR);
    }

    use crate::config::Config;
    use crate::types::TrainingMetrics;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::time::Instant;

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

    #[test]
    fn test_status_bar_includes_parser_mode() {
        let backend = TestBackend::new(100, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.config.parser = "auto".to_string();

        terminal
            .draw(|frame| {
                render_status_bar(frame, frame.area(), &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer.cell((x, y)).unwrap().symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("\n");

        assert!(content.contains("Parser: auto"));
    }

    #[test]
    fn test_status_bar_shows_active_keymap_profile() {
        let backend = TestBackend::new(120, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.config.keymap_profile = "vim".to_string();

        terminal
            .draw(|frame| {
                render_status_bar(frame, frame.area(), &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer.cell((x, y)).unwrap().symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("\n");

        assert!(content.contains("Keymap: vim"));
    }

    #[test]
    fn test_status_bar_shows_help_gateway_hint() {
        let backend = TestBackend::new(180, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new(Config::default());

        terminal
            .draw(|frame| {
                render_status_bar(frame, frame.area(), &app);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer.cell((x, y)).unwrap().symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("\n");

        assert!(content.contains("?:help"));
    }

    #[test]
    fn test_runtime_theme_switch_updates_render_styles() {
        let backend = TestBackend::new(120, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());

        app.config.theme = "classic".to_string();
        terminal
            .draw(|frame| {
                render_status_bar(frame, frame.area(), &app);
            })
            .unwrap();
        let classic_cell = terminal.backend().buffer().cell((0, 0)).unwrap().clone();

        app.config.theme = "nord".to_string();
        terminal
            .draw(|frame| {
                render_status_bar(frame, frame.area(), &app);
            })
            .unwrap();
        let nord_cell = terminal.backend().buffer().cell((0, 0)).unwrap().clone();

        assert_ne!(classic_cell.bg, nord_cell.bg);
        assert_ne!(classic_cell.fg, nord_cell.fg);
    }

    #[test]
    fn test_status_health_state_uses_shared_logic() {
        use crate::app::DataHealthState;

        let mut app = App::new(Config::default());
        assert_eq!(app.training_data_health_state(), DataHealthState::NoData);

        app.training.last_data_at = Some(std::time::Instant::now());
        app.training.input_active = false;
        assert_eq!(app.training_data_health_state(), DataHealthState::Stale);

        app.training.input_active = true;
        assert_eq!(app.training_data_health_state(), DataHealthState::Live);
    }

    #[test]
    fn test_advanced_tab_renders_core_stability_diagnostics() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.ui_state.selected_tab = Tab::Advanced;

        app.push_metrics(TrainingMetrics {
            loss: Some(0.9),
            eval_loss: Some(0.8),
            grad_norm: Some(1.5),
            tokens_per_second: Some(1400.0),
            samples_per_second: Some(20.0),
            steps_per_second: Some(0.4),
            timestamp: Instant::now(),
            ..TrainingMetrics::default()
        });

        terminal.draw(|frame| render(frame, &app)).unwrap();
        let buffer = terminal.backend().buffer();
        let content = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer.cell((x, y)).unwrap().symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("\n");

        assert!(content.contains("Stability Summary"));
        assert!(content.contains("Perplexity"));
        assert!(content.contains("Loss spikes"));
        assert!(content.contains("Parser ok/skip/err"));
    }

    #[test]
    fn test_advanced_tab_labels_units_explicitly() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.ui_state.selected_tab = Tab::Advanced;

        app.push_metrics(TrainingMetrics {
            tokens_per_second: Some(1400.0),
            samples_per_second: Some(20.0),
            steps_per_second: Some(0.4),
            timestamp: Instant::now(),
            ..TrainingMetrics::default()
        });

        terminal.draw(|frame| render(frame, &app)).unwrap();
        let buffer = terminal.backend().buffer();
        let content = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer.cell((x, y)).unwrap().symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("\n");

        assert!(content.contains("Tokens/s"));
        assert!(content.contains("tok/s (throughput)"));
        assert!(content.contains("Samples/s"));
        assert!(content.contains("samples/s (dataloader)"));
        assert!(content.contains("Steps/s"));
        assert!(content.contains("steps/s (optimizer)"));
    }

    #[test]
    fn test_advanced_graph_mode_switch_between_line_and_sparkline() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.ui_state.selected_tab = Tab::Advanced;

        app.push_metrics(TrainingMetrics {
            eval_loss: Some(0.8),
            grad_norm: Some(1.5),
            tokens_per_second: Some(1400.0),
            samples_per_second: Some(20.0),
            steps_per_second: Some(0.4),
            timestamp: Instant::now(),
            ..TrainingMetrics::default()
        });

        app.config.graph_mode = "sparkline".to_string();
        terminal.draw(|frame| render(frame, &app)).unwrap();

        app.config.graph_mode = "line".to_string();
        terminal.draw(|frame| render(frame, &app)).unwrap();
    }
}
