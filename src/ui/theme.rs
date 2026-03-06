use ratatui::style::Color;

use crate::config::{Config, CustomTheme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemePalette {
    pub header_bg: Color,
    pub header_fg: Color,
    pub accent: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub muted: Color,
    pub gpu_color: Color,
    pub cpu_color: Color,
    pub ram_color: Color,
    pub loss_color: Color,
    pub lr_color: Color,
}

pub const BUILTIN_THEMES: &[&str] = &[
    "classic",
    "catppuccin",
    "github",
    "nord",
    "gruvbox",
    "solarized",
    "dracula",
];

pub const SELECTABLE_THEMES: &[&str] = &[
    "classic",
    "catppuccin",
    "github",
    "nord",
    "gruvbox",
    "solarized",
    "dracula",
    "system",
    "custom",
];

pub fn resolve_palette_from_config(config: &Config) -> ThemePalette {
    resolve_palette_from_theme_and_custom(&config.theme, config.custom_theme.as_ref())
}

pub fn resolve_palette_from_theme_and_custom(
    theme: &str,
    custom_theme: Option<&CustomTheme>,
) -> ThemePalette {
    resolve_palette_from_theme_and_custom_with_env(theme, custom_theme, |name| {
        std::env::var(name).ok()
    })
}

pub fn resolve_palette_from_theme_and_custom_with_env<F>(
    theme: &str,
    custom_theme: Option<&CustomTheme>,
    get_env: F,
) -> ThemePalette
where
    F: Fn(&str) -> Option<String>,
{
    let normalized = theme.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "custom" => custom_palette(custom_theme),
        "system" => system_palette_from_terminal(get_env),
        _ => palette_for_name(&normalized),
    }
}

fn system_palette_from_terminal<F>(get_env: F) -> ThemePalette
where
    F: Fn(&str) -> Option<String>,
{
    if let Some(explicit) = get_env("EPOCH_SYSTEM_THEME")
        && let Some(mapped) = map_system_hint_to_theme(&explicit)
    {
        return palette_for_name(mapped);
    }

    ThemePalette {
        header_bg: Color::Reset,
        header_fg: Color::Reset,
        accent: Color::Cyan,
        success: Color::Green,
        warning: Color::Yellow,
        error: Color::Red,
        muted: Color::DarkGray,
        gpu_color: Color::Green,
        cpu_color: Color::Blue,
        ram_color: Color::Magenta,
        loss_color: Color::Yellow,
        lr_color: Color::Cyan,
    }
}

fn custom_palette(custom_theme: Option<&CustomTheme>) -> ThemePalette {
    let mut palette = palette_for_name("classic");

    let Some(custom_theme) = custom_theme else {
        return palette;
    };

    if let Some(value) = custom_theme.header_bg.as_deref().and_then(parse_color) {
        palette.header_bg = value;
    }
    if let Some(value) = custom_theme.header_fg.as_deref().and_then(parse_color) {
        palette.header_fg = value;
    }
    if let Some(value) = custom_theme.accent.as_deref().and_then(parse_color) {
        palette.accent = value;
    }
    if let Some(value) = custom_theme.success.as_deref().and_then(parse_color) {
        palette.success = value;
    }
    if let Some(value) = custom_theme.warning.as_deref().and_then(parse_color) {
        palette.warning = value;
    }
    if let Some(value) = custom_theme.error.as_deref().and_then(parse_color) {
        palette.error = value;
    }
    if let Some(value) = custom_theme.muted.as_deref().and_then(parse_color) {
        palette.muted = value;
    }
    if let Some(value) = custom_theme.gpu_color.as_deref().and_then(parse_color) {
        palette.gpu_color = value;
    }
    if let Some(value) = custom_theme.cpu_color.as_deref().and_then(parse_color) {
        palette.cpu_color = value;
    }
    if let Some(value) = custom_theme.ram_color.as_deref().and_then(parse_color) {
        palette.ram_color = value;
    }
    if let Some(value) = custom_theme.loss_color.as_deref().and_then(parse_color) {
        palette.loss_color = value;
    }
    if let Some(value) = custom_theme.lr_color.as_deref().and_then(parse_color) {
        palette.lr_color = value;
    }

    palette
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_theme_uses_terminal_ansi_palette_by_default() {
        let resolved = resolve_palette_from_theme_and_custom_with_env("system", None, |_| None);
        assert_eq!(resolved.header_bg, Color::Reset);
        assert_eq!(resolved.header_fg, Color::Reset);
        assert_eq!(resolved.accent, Color::Cyan);
        assert_eq!(resolved.success, Color::Green);
        assert_eq!(resolved.warning, Color::Yellow);
        assert_eq!(resolved.error, Color::Red);
    }

    #[test]
    fn test_system_theme_ignores_desktop_theme_envs_without_override() {
        let resolved =
            resolve_palette_from_theme_and_custom_with_env("system", None, |name| match name {
                "GTK_THEME" => Some("Adwaita-dark".to_string()),
                "APPLE_INTERFACE_STYLE" => Some("Dark".to_string()),
                _ => None,
            });

        assert_eq!(resolved.header_bg, Color::Reset);
        assert_eq!(resolved.header_fg, Color::Reset);
    }

    #[test]
    fn test_system_theme_explicit_override_still_wins() {
        let resolved = resolve_palette_from_theme_and_custom_with_env("system", None, |name| {
            if name == "EPOCH_SYSTEM_THEME" {
                Some("nord".to_string())
            } else {
                None
            }
        });

        assert_eq!(resolved, palette_for_name("nord"));
    }
}

fn map_system_hint_to_theme(value: &str) -> Option<&'static str> {
    let lowered = value.trim().to_ascii_lowercase();
    if lowered.contains("dark") {
        return Some("catppuccin");
    }
    if lowered.contains("light") {
        return Some("github");
    }

    match lowered.as_str() {
        "classic" => Some("classic"),
        "catppuccin" => Some("catppuccin"),
        "github" => Some("github"),
        "nord" => Some("nord"),
        "gruvbox" => Some("gruvbox"),
        "solarized" => Some("solarized"),
        "dracula" => Some("dracula"),
        _ => None,
    }
}

fn parse_color(raw: &str) -> Option<Color> {
    let value = raw.trim();
    if let Some(hex) = value.strip_prefix('#')
        && hex.len() == 6
    {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        return Some(Color::Rgb(r, g, b));
    }

    match value.to_ascii_lowercase().as_str() {
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        "gray" | "grey" => Some(Color::Gray),
        "darkgray" | "darkgrey" => Some(Color::DarkGray),
        "lightred" => Some(Color::LightRed),
        "lightgreen" => Some(Color::LightGreen),
        "lightyellow" => Some(Color::LightYellow),
        "lightblue" => Some(Color::LightBlue),
        "lightmagenta" => Some(Color::LightMagenta),
        "lightcyan" => Some(Color::LightCyan),
        "white" => Some(Color::White),
        _ => None,
    }
}

pub fn palette_for_name(name: &str) -> ThemePalette {
    match name.trim().to_ascii_lowercase().as_str() {
        "classic" => ThemePalette {
            header_bg: Color::Rgb(30, 30, 46),
            header_fg: Color::Rgb(205, 214, 244),
            accent: Color::Rgb(137, 180, 250),
            success: Color::Green,
            warning: Color::Yellow,
            error: Color::Red,
            muted: Color::DarkGray,
            gpu_color: Color::Rgb(166, 227, 161),
            cpu_color: Color::Rgb(137, 180, 250),
            ram_color: Color::Rgb(245, 194, 231),
            loss_color: Color::Rgb(250, 179, 135),
            lr_color: Color::Rgb(148, 226, 213),
        },
        "catppuccin" => ThemePalette {
            header_bg: Color::Rgb(30, 30, 46),
            header_fg: Color::Rgb(205, 214, 244),
            accent: Color::Rgb(137, 180, 250),
            success: Color::Rgb(166, 227, 161),
            warning: Color::Rgb(249, 226, 175),
            error: Color::Rgb(243, 139, 168),
            muted: Color::Rgb(108, 112, 134),
            gpu_color: Color::Rgb(166, 227, 161),
            cpu_color: Color::Rgb(137, 180, 250),
            ram_color: Color::Rgb(245, 194, 231),
            loss_color: Color::Rgb(250, 179, 135),
            lr_color: Color::Rgb(148, 226, 213),
        },
        "github" => ThemePalette {
            header_bg: Color::Rgb(22, 27, 34),
            header_fg: Color::Rgb(230, 237, 243),
            accent: Color::Rgb(47, 129, 247),
            success: Color::Rgb(63, 185, 80),
            warning: Color::Rgb(210, 153, 34),
            error: Color::Rgb(248, 81, 73),
            muted: Color::Rgb(139, 148, 158),
            gpu_color: Color::Rgb(63, 185, 80),
            cpu_color: Color::Rgb(47, 129, 247),
            ram_color: Color::Rgb(163, 113, 247),
            loss_color: Color::Rgb(255, 166, 87),
            lr_color: Color::Rgb(121, 192, 255),
        },
        "nord" => ThemePalette {
            header_bg: Color::Rgb(46, 52, 64),
            header_fg: Color::Rgb(216, 222, 233),
            accent: Color::Rgb(129, 161, 193),
            success: Color::Rgb(163, 190, 140),
            warning: Color::Rgb(235, 203, 139),
            error: Color::Rgb(191, 97, 106),
            muted: Color::Rgb(94, 129, 172),
            gpu_color: Color::Rgb(163, 190, 140),
            cpu_color: Color::Rgb(129, 161, 193),
            ram_color: Color::Rgb(180, 142, 173),
            loss_color: Color::Rgb(208, 135, 112),
            lr_color: Color::Rgb(136, 192, 208),
        },
        "gruvbox" => ThemePalette {
            header_bg: Color::Rgb(40, 40, 40),
            header_fg: Color::Rgb(235, 219, 178),
            accent: Color::Rgb(131, 165, 152),
            success: Color::Rgb(184, 187, 38),
            warning: Color::Rgb(250, 189, 47),
            error: Color::Rgb(251, 73, 52),
            muted: Color::Rgb(146, 131, 116),
            gpu_color: Color::Rgb(184, 187, 38),
            cpu_color: Color::Rgb(131, 165, 152),
            ram_color: Color::Rgb(211, 134, 155),
            loss_color: Color::Rgb(254, 128, 25),
            lr_color: Color::Rgb(142, 192, 124),
        },
        "solarized" => ThemePalette {
            header_bg: Color::Rgb(0, 43, 54),
            header_fg: Color::Rgb(238, 232, 213),
            accent: Color::Rgb(38, 139, 210),
            success: Color::Rgb(133, 153, 0),
            warning: Color::Rgb(181, 137, 0),
            error: Color::Rgb(220, 50, 47),
            muted: Color::Rgb(147, 161, 161),
            gpu_color: Color::Rgb(133, 153, 0),
            cpu_color: Color::Rgb(38, 139, 210),
            ram_color: Color::Rgb(108, 113, 196),
            loss_color: Color::Rgb(203, 75, 22),
            lr_color: Color::Rgb(42, 161, 152),
        },
        "dracula" => ThemePalette {
            header_bg: Color::Rgb(40, 42, 54),
            header_fg: Color::Rgb(248, 248, 242),
            accent: Color::Rgb(189, 147, 249),
            success: Color::Rgb(80, 250, 123),
            warning: Color::Rgb(241, 250, 140),
            error: Color::Rgb(255, 85, 85),
            muted: Color::Rgb(98, 114, 164),
            gpu_color: Color::Rgb(80, 250, 123),
            cpu_color: Color::Rgb(139, 233, 253),
            ram_color: Color::Rgb(255, 121, 198),
            loss_color: Color::Rgb(255, 184, 108),
            lr_color: Color::Rgb(139, 233, 253),
        },
        _ => palette_for_name("classic"),
    }
}
