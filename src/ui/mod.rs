pub mod advanced;
pub mod components;
pub mod dashboard;
pub mod file_picker;
pub mod graph;
pub mod header;
pub mod help;
pub mod home;
pub mod live;
pub mod metrics;
pub mod run_explorer;
pub mod settings;
pub mod system;
pub mod system_processes;
pub mod theme;

use ratatui::Frame;
use ratatui::style::{Color, Modifier, Style};

use crate::app::{App, AppMode, MonitoringRoute};
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
pub const MIN_WIDTH: u16 = 80;
pub const MIN_HEIGHT: u16 = 40;

pub fn header_style() -> Style {
    Style::default().fg(HEADER_FG).bg(HEADER_BG)
}

pub fn metric_label_style() -> Style {
    Style::default().fg(HEADER_FG).add_modifier(Modifier::BOLD)
}

pub fn metric_value_style() -> Style {
    Style::default().fg(ACCENT)
}

pub fn phase1_primary_views() -> [MonitoringRoute; 2] {
    [MonitoringRoute::Home, MonitoringRoute::RunDetail]
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

    let [header_area, content_area, status_area] = ratatui::layout::Layout::vertical([
        ratatui::layout::Constraint::Length(1),
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
        AppMode::Monitoring => match app.ui_state.monitoring.route {
            MonitoringRoute::Home => home::render(frame, content_area, app),
            MonitoringRoute::RunDetail => live::render_for_surface(
                frame,
                content_area,
                app,
                live::LiveSurface::RunDetail {
                    selected_run_id: app.run_detail_selected_run_id(),
                    compare_run_id: app.run_detail_compare_run_id(),
                },
            ),
        },
    }

    render_command_bar(frame, status_area, app);
}

fn render_command_bar(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let palette = resolve_palette_from_config(&app.config);
    let bar_style = ratatui::style::Style::default()
        .fg(palette.header_fg)
        .bg(palette.header_bg);

    let (panel_cmds, global_cmds) = match &app.ui_state.mode {
        AppMode::Monitoring => {
            let prefix = match app.ui_state.monitoring.route {
                MonitoringRoute::Home => {
                    format!("[{}:{}]", app.home_focus_index(), app.home_focus_label())
                }
                MonitoringRoute::RunDetail => {
                    format!(
                        "[{}:{}]",
                        app.ui_state.focused_box,
                        app.run_detail_focus_label()
                    )
                }
            };
            (
                format!("{prefix} {}", app.active_panel_commands()),
                app.monitoring_global_commands(),
            )
        }
        AppMode::Settings(_) => (
            "Up/Down:row  Left/Right:change  a:apply  w/Enter:save  Esc:cancel".to_string(),
            String::new(),
        ),
        AppMode::Help(_) => ("?:close  Esc:close".to_string(), String::new()),
        AppMode::FilePicker(state) => {
            let cmds = if app.config.keymap_profile == "vim" {
                match state.input_mode {
                    crate::app::FilePickerInputMode::Insert => {
                        " Picker[INSERT] Esc:normal Type:filter Enter:open".to_string()
                    }
                    crate::app::FilePickerInputMode::Normal => {
                        " Picker[NORMAL] i:insert j/k:select Enter:open Esc:quit".to_string()
                    }
                }
            } else {
                "Type:filter  Up/Down:select  Enter:open  Esc:quit".to_string()
            };
            (cmds, String::new())
        }
        AppMode::Scanning => ("Scanning files...".to_string(), String::new()),
    };

    let [panel_area, global_area] = ratatui::layout::Layout::horizontal([
        ratatui::layout::Constraint::Fill(1),
        ratatui::layout::Constraint::Fill(1),
    ])
    .areas(area);

    let panel_widget = ratatui::widgets::Paragraph::new(panel_cmds).style(bar_style);
    let global_widget = ratatui::widgets::Paragraph::new(global_cmds)
        .alignment(ratatui::layout::Alignment::Right)
        .style(bar_style);

    frame.render_widget(panel_widget, panel_area);
    frame.render_widget(global_widget, global_area);
}

pub fn active_commands_for_view(app: &App) -> String {
    app.active_panel_commands()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::PanelFocus;
    use crate::ui::theme::palette_for_name;

    #[test]
    fn test_monitoring_routes_include_home_and_detail() {
        let routes = phase1_primary_views();
        assert_eq!(routes, [MonitoringRoute::Home, MonitoringRoute::RunDetail]);
    }

    #[test]
    fn test_style_functions_return_non_default() {
        let header = header_style();
        assert_ne!(header, Style::default());
    }

    #[test]
    fn test_min_dimensions_exist() {
        assert_eq!(MIN_WIDTH, 80);
        assert_eq!(MIN_HEIGHT, 40);
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
    fn test_render_command_bar_content() {
        let backend = TestBackend::new(180, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.running = true;
        terminal
            .draw(|frame| {
                render_command_bar(frame, frame.area(), &app);
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
        assert!(content.contains("Tab/Shift+Tab:cycle"));
    }

    #[test]
    fn test_command_bar_shows_help_gateway_hint() {
        let backend = TestBackend::new(180, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new(Config::default());

        terminal
            .draw(|frame| {
                render_command_bar(frame, frame.area(), &app);
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
    fn test_command_bar_home_shows_real_shortcuts() {
        let backend = TestBackend::new(180, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.ui_state.monitoring.route = MonitoringRoute::Home;

        terminal
            .draw(|frame| {
                render_command_bar(frame, frame.area(), &app);
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

        assert!(content.contains("Enter:view current run") || content.contains("r:refresh runs"));
        assert!(content.contains("r:refresh"));
    }

    #[test]
    fn test_command_bar_run_explorer_search_mode() {
        let backend = TestBackend::new(180, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.ui_state.monitoring.route = MonitoringRoute::Home;
        app.ui_state.monitoring.focused_panel = Some(PanelFocus::Runs);
        app.ui_state.explorer.search_active = true;

        terminal
            .draw(|frame| {
                render_command_bar(frame, frame.area(), &app);
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

        assert!(content.contains("Type:search"));
        assert!(content.contains("Backspace:erase"));
        assert!(content.contains("Enter:apply"));
        assert!(content.contains("Esc:close"));
    }

    #[test]
    fn test_command_bar_system_processes_removes_track_hint() {
        let backend = TestBackend::new(180, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.ui_state.monitoring.route = MonitoringRoute::Home;
        app.ui_state.monitoring.focused_panel = Some(PanelFocus::Processes);

        terminal
            .draw(|frame| {
                render_command_bar(frame, frame.area(), &app);
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

        assert!(content.contains("Up/Down:select"));
        assert!(content.contains("Enter:attach process"));
        assert!(content.contains("r:refresh"));
        assert!(!content.contains("t:track"));
    }

    #[test]
    fn test_active_commands_for_view_matches_monitoring_views() {
        let mut app = App::new(Config::default());

        app.ui_state.monitoring.route = MonitoringRoute::Home;
        assert_eq!(active_commands_for_view(&app), "r:refresh runs");

        app.ui_state.monitoring.route = MonitoringRoute::Home;
        app.ui_state.monitoring.focused_panel = Some(PanelFocus::Runs);
        app.ui_state.monitoring.home_focus = crate::app::HomeFocusTarget::Runs;
        assert_eq!(
            active_commands_for_view(&app),
            "Up/Down:select  /:search  f:filter  Enter:view run  r:refresh"
        );

        app.ui_state.explorer.search_active = true;
        assert_eq!(
            active_commands_for_view(&app),
            "Type:search  Backspace:erase  Enter:apply  Esc:close"
        );

        app.ui_state.monitoring.route = MonitoringRoute::Home;
        app.ui_state.monitoring.focused_panel = Some(PanelFocus::Processes);
        app.ui_state.monitoring.home_focus = crate::app::HomeFocusTarget::Processes;
        app.ui_state.explorer.search_active = false;
        assert_eq!(
            active_commands_for_view(&app),
            "Up/Down:select  Enter:attach process  r:refresh"
        );
    }

    #[test]
    fn test_runtime_theme_switch_updates_render_styles() {
        let backend = TestBackend::new(120, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());

        app.config.theme = "classic".to_string();
        terminal
            .draw(|frame| {
                render_command_bar(frame, frame.area(), &app);
            })
            .unwrap();
        let classic_cell = terminal.backend().buffer().cell((0, 0)).unwrap().clone();

        app.config.theme = "nord".to_string();
        terminal
            .draw(|frame| {
                render_command_bar(frame, frame.area(), &app);
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
    fn test_live_view_renders_all_graph_boxes() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.ui_state.monitoring.run_detail.selected_run_id = Some("run-1".to_string());

        app.push_metrics(TrainingMetrics {
            loss: Some(0.5),
            eval_loss: Some(0.8),
            learning_rate: Some(1e-4),
            grad_norm: Some(1.5),
            tokens_per_second: Some(1400.0),
            samples_per_second: Some(20.0),
            steps_per_second: Some(0.4),
            step: Some(100),
            tokens: Some(42000),
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

        assert!(content.contains("Loss:"));
        assert!(content.contains("Eval Loss"));
        assert!(content.contains("Learning Rate:"));
        assert!(content.contains("Grad Norm"));
        assert!(content.contains("Stability Summary"));
        assert!(content.contains("Core"));
        assert!(content.contains("Signals"));
    }

    #[test]
    fn test_live_view_focus_box_highlights() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.ui_state.monitoring.run_detail.selected_run_id = Some("run-1".to_string());
        app.push_metrics(TrainingMetrics {
            loss: Some(0.5),
            learning_rate: Some(1e-4),
            step: Some(100),
            timestamp: Instant::now(),
            ..TrainingMetrics::default()
        });

        app.ui_state.focused_box = 1;
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let buffer1 = terminal.backend().buffer().clone();

        app.ui_state.focused_box = 3;
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let buffer2 = terminal.backend().buffer().clone();

        app.ui_state.focused_box = 2;
        terminal.draw(|frame| render(frame, &app)).unwrap();
        let buffer3 = terminal.backend().buffer().clone();

        assert_ne!(buffer1, buffer2);
        assert_ne!(buffer2, buffer3);
    }

    #[test]
    fn test_live_view_graph_mode_switch() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.ui_state.monitoring.run_detail.selected_run_id = Some("run-1".to_string());

        app.push_metrics(TrainingMetrics {
            loss: Some(0.5),
            eval_loss: Some(0.8),
            learning_rate: Some(1e-4),
            grad_norm: Some(1.5),
            timestamp: Instant::now(),
            ..TrainingMetrics::default()
        });

        app.config.graph_mode = "sparkline".to_string();
        terminal.draw(|frame| render(frame, &app)).unwrap();

        app.config.graph_mode = "line".to_string();
        terminal.draw(|frame| render(frame, &app)).unwrap();

        // Should not panic
        assert!(true);
    }

    #[test]
    fn test_live_view_focus_index_in_titles() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(Config::default());
        app.ui_state.monitoring.run_detail.selected_run_id = Some("run-1".to_string());
        app.push_metrics(TrainingMetrics {
            loss: Some(0.5),
            learning_rate: Some(1e-4),
            step: Some(100),
            timestamp: Instant::now(),
            ..TrainingMetrics::default()
        });

        app.ui_state.focused_box = 1;
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

        assert!(content.contains("Loss:"));
    }
}
