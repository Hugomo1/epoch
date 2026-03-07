use color_eyre::{
    Result,
    eyre::{Context, eyre},
};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use toml::Value;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct CustomTheme {
    pub header_bg: Option<String>,
    pub header_fg: Option<String>,
    pub accent: Option<String>,
    pub success: Option<String>,
    pub warning: Option<String>,
    pub error: Option<String>,
    pub muted: Option<String>,
    pub gpu_color: Option<String>,
    pub cpu_color: Option<String>,
    pub ram_color: Option<String>,
    pub loss_color: Option<String>,
    pub lr_color: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlertRuleKind {
    LossTrendWorsening,
    ThroughputDrop,
    MemoryPressure,
}

impl AlertRuleKind {
    pub fn as_id(&self) -> &'static str {
        match self {
            Self::LossTrendWorsening => "loss_trend_worsening",
            Self::ThroughputDrop => "throughput_drop",
            Self::MemoryPressure => "memory_pressure",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AlertEvalMode {
    Current,
    RollingMean { window: usize },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct AlertRuleConfig {
    pub id: Option<String>,
    pub kind: AlertRuleKind,
    pub mode: AlertEvalMode,
    pub warning: f64,
    pub critical: f64,
    pub enabled: bool,
}

impl Default for AlertRuleConfig {
    fn default() -> Self {
        Self {
            id: None,
            kind: AlertRuleKind::ThroughputDrop,
            mode: AlertEvalMode::Current,
            warning: 0.0,
            critical: 0.0,
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub tick_rate_ms: u64,
    pub history_size: usize,
    pub stale_after_secs: u64,
    pub parser: String,
    pub theme: String,
    pub graph_mode: String,
    pub adaptive_layout: bool,
    pub pinned_metrics: Vec<String>,
    pub hidden_metrics: Vec<String>,
    pub keymap_profile: String,
    pub profile_target: String,
    pub custom_theme: Option<CustomTheme>,
    pub alert_rules: Vec<AlertRuleConfig>,
    pub run_comparison_file: Option<PathBuf>,
    pub regex_pattern: Option<String>,
    pub log_file: Option<PathBuf>,
    pub stdin_mode: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct ConfigPartial {
    tick_rate_ms: Option<u64>,
    history_size: Option<usize>,
    stale_after_secs: Option<u64>,
    parser: Option<String>,
    theme: Option<String>,
    graph_mode: Option<String>,
    adaptive_layout: Option<bool>,
    pinned_metrics: Option<Vec<String>>,
    hidden_metrics: Option<Vec<String>>,
    keymap_profile: Option<String>,
    profile_target: Option<String>,
    custom_theme: Option<Option<CustomTheme>>,
    alert_rules: Option<Vec<AlertRuleConfig>>,
    run_comparison_file: Option<Option<PathBuf>>,
    regex_pattern: Option<Option<String>>,
    log_file: Option<Option<PathBuf>>,
    stdin_mode: Option<bool>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tick_rate_ms: 250,
            history_size: 300,
            stale_after_secs: 10,
            parser: "auto".to_string(),
            theme: "classic".to_string(),
            graph_mode: "sparkline".to_string(),
            adaptive_layout: true,
            pinned_metrics: Vec::new(),
            hidden_metrics: Vec::new(),
            keymap_profile: "default".to_string(),
            profile_target: "global".to_string(),
            custom_theme: None,
            alert_rules: Vec::new(),
            run_comparison_file: None,
            regex_pattern: None,
            log_file: None,
            stdin_mode: false,
        }
    }
}

impl Config {
    fn normalize_metric_ids(ids: Vec<String>) -> Vec<String> {
        let mut out = Vec::new();
        for id in ids {
            let normalized = id.trim().to_ascii_lowercase();
            if normalized.is_empty() || out.iter().any(|existing| existing == &normalized) {
                continue;
            }
            out.push(normalized);
        }
        out
    }

    fn global_config_path() -> Option<PathBuf> {
        directories::ProjectDirs::from("", "", "epoch")
            .map(|dirs| dirs.config_dir().join("config.toml"))
    }

    fn project_config_path(root: &Path) -> PathBuf {
        root.join(".epoch").join("config.toml")
    }

    fn apply_partial(&mut self, partial: ConfigPartial) {
        if let Some(v) = partial.tick_rate_ms {
            self.tick_rate_ms = v;
        }
        if let Some(v) = partial.history_size {
            self.history_size = v;
        }
        if let Some(v) = partial.stale_after_secs {
            self.stale_after_secs = v;
        }
        if let Some(v) = partial.parser {
            self.parser = v;
        }
        if let Some(v) = partial.theme {
            self.theme = v;
        }
        if let Some(v) = partial.graph_mode {
            self.graph_mode = v;
        }
        if let Some(v) = partial.adaptive_layout {
            self.adaptive_layout = v;
        }
        if let Some(v) = partial.pinned_metrics {
            self.pinned_metrics = Self::normalize_metric_ids(v);
        }
        if let Some(v) = partial.hidden_metrics {
            self.hidden_metrics = Self::normalize_metric_ids(v);
        }
        if let Some(v) = partial.keymap_profile {
            self.keymap_profile = v;
        }
        if let Some(v) = partial.profile_target {
            self.profile_target = v;
        }
        if let Some(v) = partial.custom_theme {
            self.custom_theme = v;
        }
        if let Some(v) = partial.alert_rules {
            self.alert_rules = v;
        }
        if let Some(v) = partial.run_comparison_file {
            self.run_comparison_file = v;
        }
        if let Some(v) = partial.regex_pattern {
            self.regex_pattern = v;
        }
        if let Some(v) = partial.log_file {
            self.log_file = v;
        }
        if let Some(v) = partial.stdin_mode {
            self.stdin_mode = v;
        }
    }

    fn load_partial_from_path(path: &Path) -> Result<Option<ConfigPartial>> {
        if !path.exists() {
            return Ok(None);
        }

        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;
        let mut value: Value = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse TOML config: {}", path.display()))?;
        let custom_theme = Self::extract_custom_theme_override(&mut value);
        let mut partial: ConfigPartial = value
            .try_into()
            .with_context(|| format!("Failed to parse TOML config fields: {}", path.display()))?;
        if let Some(theme) = custom_theme {
            partial.custom_theme = Some(Some(theme));
        }

        Ok(Some(partial))
    }

    fn extract_custom_theme_override(value: &mut Value) -> Option<CustomTheme> {
        let table = value.as_table_mut()?;
        let raw = table.remove("custom_theme")?;

        let Some(custom_table) = raw.as_table() else {
            tracing::debug!("invalid custom_theme ignored: expected table");
            return None;
        };

        fn pick_string(table: &toml::value::Table, key: &str) -> Option<String> {
            match table.get(key) {
                Some(v) => match v.as_str() {
                    Some(s) => Some(s.to_string()),
                    None => {
                        tracing::debug!("invalid custom_theme.{key} ignored: expected string");
                        None
                    }
                },
                None => None,
            }
        }

        Some(CustomTheme {
            header_bg: pick_string(custom_table, "header_bg"),
            header_fg: pick_string(custom_table, "header_fg"),
            accent: pick_string(custom_table, "accent"),
            success: pick_string(custom_table, "success"),
            warning: pick_string(custom_table, "warning"),
            error: pick_string(custom_table, "error"),
            muted: pick_string(custom_table, "muted"),
            gpu_color: pick_string(custom_table, "gpu_color"),
            cpu_color: pick_string(custom_table, "cpu_color"),
            ram_color: pick_string(custom_table, "ram_color"),
            loss_color: pick_string(custom_table, "loss_color"),
            lr_color: pick_string(custom_table, "lr_color"),
        })
    }

    fn load_effective_with_paths(
        global_path: Option<&Path>,
        project_path: Option<&Path>,
    ) -> Result<Self> {
        let mut config = Config::default();

        if let Some(path) = global_path
            && let Some(partial) = Self::load_partial_from_path(path)?
        {
            config.apply_partial(partial);
        }

        if let Some(path) = project_path
            && let Some(partial) = Self::load_partial_from_path(path)?
        {
            config.apply_partial(partial);
        }

        Ok(config)
    }

    /// Load configuration from XDG config directory (~/.config/epoch/config.toml)
    /// Returns default config if file doesn't exist.
    pub fn load() -> Result<Self> {
        Self::load_effective_with_paths(Self::global_config_path().as_deref(), None)
    }

    pub fn load_effective(project_root: Option<&Path>) -> Result<Self> {
        let project_path = project_root.map(Self::project_config_path);
        Self::load_effective_with_paths(
            Self::global_config_path().as_deref(),
            project_path.as_deref(),
        )
    }

    pub fn save_atomic(path: &Path, cfg: &Self) -> Result<()> {
        let parent = path
            .parent()
            .ok_or_else(|| eyre!("Config path must have a parent directory"))?;
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create config directory: {}", parent.display()))?;

        let serialized =
            toml::to_string_pretty(cfg).context("Failed to serialize config to TOML")?;

        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .context("System clock is before UNIX_EPOCH")?
            .as_nanos();
        let tmp_path = parent.join(format!(".config.toml.{unique}.tmp"));

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp_path)
            .with_context(|| {
                format!("Failed to create temp config file: {}", tmp_path.display())
            })?;

        file.write_all(serialized.as_bytes())
            .with_context(|| format!("Failed to write temp config file: {}", tmp_path.display()))?;
        file.sync_all()
            .with_context(|| format!("Failed to sync temp config file: {}", tmp_path.display()))?;
        drop(file);

        if let Err(err) = std::fs::rename(&tmp_path, path) {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(err).with_context(|| {
                format!(
                    "Failed to atomically rename config file to: {}",
                    path.display()
                )
            });
        }

        #[cfg(not(target_os = "windows"))]
        {
            let dir = std::fs::File::open(parent).with_context(|| {
                format!(
                    "Failed to open config directory for fsync: {}",
                    parent.display()
                )
            })?;
            dir.sync_all().with_context(|| {
                format!("Failed to sync config directory: {}", parent.display())
            })?;
        }

        Ok(())
    }

    pub fn save_global(&self) -> Result<()> {
        let path = Self::global_config_path()
            .ok_or_else(|| eyre!("Could not determine global config path"))?;
        Self::save_atomic(&path, self)
    }

    pub fn save_project(&self, root: &Path) -> Result<()> {
        let path = Self::project_config_path(root);
        Self::save_atomic(&path, self)
    }

    /// Merge CLI arguments into config (CLI takes precedence)
    pub fn merge_cli_args(
        &mut self,
        log_file: Option<PathBuf>,
        stdin: bool,
        parser: Option<String>,
    ) {
        if let Some(lf) = log_file {
            self.log_file = Some(lf);
        }
        if stdin {
            self.stdin_mode = true;
        }
        if let Some(p) = parser {
            self.parser = p;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("epoch-{prefix}-{unique}"));
        fs::create_dir_all(&root).expect("temp root should be created");
        root
    }

    #[test]
    fn test_config_defaults() {
        let config = Config::default();
        assert_eq!(config.tick_rate_ms, 250);
        assert_eq!(config.history_size, 300);
        assert_eq!(config.stale_after_secs, 10);
        assert_eq!(config.parser, "auto");
        assert_eq!(config.theme, "classic");
        assert_eq!(config.graph_mode, "sparkline");
        assert_eq!(config.keymap_profile, "default");
        assert_eq!(config.profile_target, "global");
    }

    #[test]
    fn test_config_defaults_expanded() {
        let config = Config::default();
        assert_eq!(config.tick_rate_ms, 250);
        assert_eq!(config.history_size, 300);
        assert_eq!(config.stale_after_secs, 10);
        assert_eq!(config.parser, "auto");
        assert_eq!(config.theme, "classic");
        assert_eq!(config.graph_mode, "sparkline");
        assert_eq!(config.keymap_profile, "default");
        assert_eq!(config.profile_target, "global");
        assert!(config.pinned_metrics.is_empty());
        assert!(config.hidden_metrics.is_empty());
        assert!(config.regex_pattern.is_none());
        assert!(config.log_file.is_none());
        assert!(!config.stdin_mode);
    }

    #[test]
    fn test_config_parse_toml() {
        let toml_str = r#"
            tick_rate_ms = 100
            history_size = 500
            stale_after_secs = 20
            parser = "jsonl"
            theme = "nord"
            graph_mode = "line"
            keymap_profile = "vim"
            profile_target = "project"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.tick_rate_ms, 100);
        assert_eq!(config.history_size, 500);
        assert_eq!(config.stale_after_secs, 20);
        assert_eq!(config.parser, "jsonl");
        assert_eq!(config.theme, "nord");
        assert_eq!(config.graph_mode, "line");
        assert_eq!(config.keymap_profile, "vim");
        assert_eq!(config.profile_target, "project");
    }

    #[test]
    fn test_metric_id_lists_are_normalized_and_deduped() {
        let mut config = Config::default();
        config.apply_partial(ConfigPartial {
            pinned_metrics: Some(vec![
                " Tokens_Per_Second ".to_string(),
                "tokens_per_second".to_string(),
                "SAMPLES_PER_SECOND".to_string(),
            ]),
            hidden_metrics: Some(vec![
                " Steps_Per_Second ".to_string(),
                "steps_per_second".to_string(),
                "".to_string(),
            ]),
            ..ConfigPartial::default()
        });

        assert_eq!(
            config.pinned_metrics,
            vec![
                "tokens_per_second".to_string(),
                "samples_per_second".to_string()
            ]
        );
        assert_eq!(config.hidden_metrics, vec!["steps_per_second".to_string()]);
    }

    #[test]
    fn test_config_parse_partial_toml() {
        let toml_str = r#"tick_rate_ms = 100"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.tick_rate_ms, 100);
        assert_eq!(config.history_size, 300); // default
        assert_eq!(config.stale_after_secs, 10); // default
        assert_eq!(config.parser, "auto"); // default
        assert_eq!(config.theme, "classic"); // default
        assert_eq!(config.graph_mode, "sparkline");
        assert_eq!(config.keymap_profile, "default");
        assert_eq!(config.profile_target, "global");
    }

    #[test]
    fn test_config_stale_after_secs_default() {
        let config = Config::default();
        assert_eq!(config.stale_after_secs, 10);
    }

    #[test]
    fn test_custom_theme_parses_and_applies() {
        let root = temp_root("custom-theme-parse");
        let cfg_path = root.join("config.toml");
        fs::write(
            &cfg_path,
            r##"
theme = "custom"
[custom_theme]
header_bg = "#112233"
accent = "#abcdef"
"##,
        )
        .expect("custom config should be written");

        let config = Config::load_effective_with_paths(Some(&cfg_path), None)
            .expect("config should load with custom theme");

        assert_eq!(config.theme, "custom");
        let custom = config
            .custom_theme
            .as_ref()
            .expect("custom theme should be present");
        assert_eq!(custom.header_bg.as_deref(), Some("#112233"));
        assert_eq!(custom.accent.as_deref(), Some("#abcdef"));

        let palette = crate::ui::theme::resolve_palette_from_config(&config);
        assert_eq!(palette.header_bg, ratatui::style::Color::Rgb(17, 34, 51));
        assert_eq!(palette.accent, ratatui::style::Color::Rgb(171, 205, 239));

        fs::remove_dir_all(&root).expect("temp root should be removed");
    }

    #[test]
    fn test_invalid_custom_theme_uses_safe_fallback() {
        let root = temp_root("custom-theme-invalid");
        let cfg_path = root.join("config.toml");
        fs::write(
            &cfg_path,
            r#"
theme = "custom"
custom_theme = "not-a-table"
"#,
        )
        .expect("invalid custom config should be written");

        let config = Config::load_effective_with_paths(Some(&cfg_path), None)
            .expect("config should load even with invalid custom theme");

        assert_eq!(config.theme, "custom");
        assert!(config.custom_theme.is_none());

        let palette = crate::ui::theme::resolve_palette_from_config(&config);
        let classic = crate::ui::theme::palette_for_name("classic");
        assert_eq!(palette, classic);

        fs::remove_dir_all(&root).expect("temp root should be removed");
    }

    #[test]
    fn test_custom_theme_partial_table_keeps_valid_string_fields() {
        let root = temp_root("custom-theme-partial-table");
        let cfg_path = root.join("config.toml");
        fs::write(
            &cfg_path,
            r##"
theme = "custom"
[custom_theme]
header_bg = 123
accent = "#abcdef"
"##,
        )
        .expect("partial custom config should be written");

        let config = Config::load_effective_with_paths(Some(&cfg_path), None)
            .expect("config should load with partial custom theme");

        let custom = config
            .custom_theme
            .as_ref()
            .expect("custom theme should be present");
        assert_eq!(custom.header_bg, None);
        assert_eq!(custom.accent.as_deref(), Some("#abcdef"));

        let palette = crate::ui::theme::resolve_palette_from_config(&config);
        assert_eq!(palette.accent, ratatui::style::Color::Rgb(171, 205, 239));

        fs::remove_dir_all(&root).expect("temp root should be removed");
    }

    #[test]
    fn test_system_theme_resolution_uses_terminal_palette_defaults() {
        let palette = crate::ui::theme::resolve_palette_from_theme_and_custom_with_env(
            "system",
            None,
            |_| None,
        );
        assert_eq!(palette.header_bg, ratatui::style::Color::Reset);
        assert_eq!(palette.header_fg, ratatui::style::Color::Reset);
    }

    #[test]
    fn test_config_invalid_toml_errors() {
        let result: Result<Config, _> = toml::from_str("this is not valid toml {{{");
        assert!(result.is_err());
    }

    #[test]
    fn test_config_unknown_fields_accepted() {
        let toml_str = r#"
            tick_rate_ms = 100
            unknown_field = "oops"
        "#;
        let result: Result<Config, _> = toml::from_str(toml_str);
        assert!(result.is_ok());
    }

    #[test]
    fn test_config_merge_cli_overrides() {
        let mut config = Config::default();
        // Simulate CLI with parser set
        config.merge_cli_args(
            Some(PathBuf::from("/tmp/train.log")),
            false,
            Some("jsonl".to_string()),
        );
        assert_eq!(config.parser, "jsonl");
        assert_eq!(config.log_file, Some(PathBuf::from("/tmp/train.log")));
    }

    #[test]
    fn test_config_merge_cli_stdin() {
        let mut config = Config::default();
        config.merge_cli_args(None, true, None);
        assert!(config.stdin_mode);
        assert_eq!(config.parser, "auto"); // not overridden
    }

    #[test]
    fn test_config_merge_cli_no_override_when_none() {
        let mut config = Config {
            parser: "jsonl".to_string(),
            ..Config::default()
        };
        config.merge_cli_args(None, false, None);
        assert_eq!(config.parser, "jsonl"); // kept, not overridden to "auto"
    }

    #[test]
    fn test_layered_profile_precedence_defaults_global_project_cli() {
        let root = temp_root("layered-precedence");
        let global_path = root.join("global.toml");
        let project_path = root.join("project").join(".epoch").join("config.toml");

        fs::create_dir_all(project_path.parent().expect("project parent should exist"))
            .expect("project dir should be created");
        fs::write(
            &global_path,
            r#"
tick_rate_ms = 500
parser = "jsonl"
theme = "nord"
"#,
        )
        .expect("global config should be written");
        fs::write(
            &project_path,
            r#"
history_size = 900
theme = "github"
"#,
        )
        .expect("project config should be written");

        let mut config =
            Config::load_effective_with_paths(Some(&global_path), Some(&project_path)).unwrap();
        config.merge_cli_args(None, false, Some("regex".to_string()));

        assert_eq!(config.tick_rate_ms, 500);
        assert_eq!(config.history_size, 900);
        assert_eq!(config.theme, "github");
        assert_eq!(config.parser, "regex");

        fs::remove_dir_all(&root).expect("temp root should be removed");
    }

    #[test]
    fn test_cosmetic_autosave_does_not_touch_project_behavior() {
        let root = temp_root("cosmetic-autosave");
        let global_path = root.join("global.toml");
        let project_path = root.join("project").join(".epoch").join("config.toml");

        fs::create_dir_all(project_path.parent().expect("project parent should exist"))
            .expect("project dir should be created");
        fs::write(
            &project_path,
            r#"
parser = "regex"
history_size = 777
"#,
        )
        .expect("project config should be written");

        let cosmetic = Config {
            theme: "dracula".to_string(),
            ..Config::default()
        };
        Config::save_atomic(&global_path, &cosmetic).expect("global save should succeed");

        let project_after = fs::read_to_string(&project_path).expect("project config should exist");
        assert!(project_after.contains("parser = \"regex\""));
        assert!(project_after.contains("history_size = 777"));

        fs::remove_dir_all(&root).expect("temp root should be removed");
    }

    #[test]
    fn test_atomic_save_recovers_from_partial_write_failure() {
        let root = temp_root("atomic-failure");
        let blocked_parent = root.join("blocked");
        fs::write(&blocked_parent, "not-a-directory").expect("blocking file should be written");

        let target = blocked_parent.join("config.toml");
        let config = Config::default();
        let result = Config::save_atomic(&target, &config);
        assert!(result.is_err());

        let entries: Vec<_> = fs::read_dir(&root)
            .expect("root should be readable")
            .map(|e| e.expect("entry should exist").file_name())
            .collect();
        assert_eq!(entries.len(), 1);

        fs::remove_dir_all(&root).expect("temp root should be removed");
    }

    #[test]
    fn test_config_load_missing_file_returns_defaults() {
        // Config::load should not error if config file doesn't exist
        let config = Config::load().unwrap();
        assert_eq!(config.tick_rate_ms, 250);
    }

    #[test]
    fn test_readme_documented_release_keys_exist() {
        let readme = std::fs::read_to_string("README.md").expect("README should exist");
        assert!(readme.contains("1-4"));
        assert!(readme.contains("Focus graph"));
        assert!(readme.contains("[[alert_rules]]"));
        assert!(readme.contains("kind = \"throughput_drop\""));
        assert!(readme.contains("run_comparison_file"));
    }
}
