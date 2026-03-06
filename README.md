<p align="center">
  <img src="docs/logo_v2.svg" alt="epoch" width="100%"/>
</p>

<p align="center">
  <strong>A real-time view into your AI training runs, right in the terminal.</strong>
</p>

<p align="center">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-292929?style=for-the-badge&logo=rust&logoColor=46b980"/>
  <img alt="Ratatui" src="https://img.shields.io/badge/Ratatui-TUI-46b980?style=for-the-badge&logoColor=ffffff"/>
  <img alt="Terminal" src="https://img.shields.io/badge/Interface-Terminal-46b980?style=for-the-badge"/>
  <img alt="License" src="https://img.shields.io/badge/License-MIT-46b980?style=for-the-badge"/>
</p>

<p align="center">
  <code>epoch</code> watches your training logs and turns them into a smooth, readable dashboard —
  loss curves, training speed, and hardware usage all in one place.
</p>

---

<p align="center">
  <img src="docs/demo.gif" alt="epoch preview" width="900"/>
</p>

---

# Quick Start

```bash
# Clone the repository
git clone https://github.com/grannejanne/epoch.git
cd epoch

# Build
cargo build --release

# Run
./target/release/epoch train.log
```

Or pipe output directly from your training script:

```bash
python train.py 2>&1 | epoch --stdin
```

Epoch will immediately start visualizing your training metrics.

# Usage

```
epoch [OPTIONS] [LOG_FILE]

Arguments:
  [LOG_FILE]    Training log file to monitor

Options:
      --stdin            Read metrics from standard input
      --parser <TYPE>    Override log parser (auto, jsonl, csv, regex, tensorboard)
  -h, --help             Print help
  -V, --version          Print version
```

Examples:

```bash
epoch train.log
epoch --parser jsonl train.log
epoch --parser tensorboard train.log
python train.py | epoch --stdin
epoch
```

Running `epoch` with no arguments will search the current directory for recent training logs and attach automatically.

# What `epoch` Shows

Epoch focuses on the things you care about during training.

### Training Metrics

- Live **loss tracking**
- **Learning rate**
- **training steps**
- **throughput** (tokens/s, samples/s, or steps/s)
- rolling metric history

### System Metrics

- **GPU utilization**
- **VRAM usage**
- **CPU load**
- **system memory**

### Interface

- smooth terminal dashboard
- multiple views (dashboard / metrics / system)
- live graphs and history navigation
- works great over SSH

# Supported Log Formats

Epoch supports parser detection and explicit parser override.

- `auto` (default): detects known formats from stream/file content
- `jsonl`: JSON object per line (including nested aliases)
- `csv`: header-based CSV metrics
- `regex`: custom named capture parser via `regex_pattern`
- `tensorboard`: parser entrypoint is wired and safe (non-panicking fallback)

Examples:

```text
JSONL
{"loss": 0.53, "step": 120, "lr": 1e-4}
```

```text
CSV
step,loss,lr
```

```text
Regex
step=120 loss=0.53 lr=1e-4
```

```text
HuggingFace trainer_state.json (auto mode)
```

# Keybindings

Global:

| Key | Action |
| --- | --- |
| `q` / `Ctrl+C` | Quit |
| `Tab` / `Shift+Tab` | Switch tabs |
| `1/2/3/4` | Jump to Dashboard / Metrics / System / Advanced |
| `Space` | Toggle live/pause |
| `Left/Right` | Pan history |
| `- / =` | Zoom out / in |
| `g` | Reset viewport to live |
| `s` | Open settings |
| `?` | Toggle help overlay |

Vim profile extras (`keymap_profile = "vim"`):

| Key | Action |
| --- | --- |
| `j` / `k` | Next / previous tab (monitoring mode) |
| `h` / `l` | Pan history left / right (monitoring mode) |

File picker in vim profile is modal:

| Mode | Key | Action |
| --- | --- | --- |
| `NORMAL` | `i` | Enter insert mode |
| `NORMAL` | `j/k` or `Up/Down` | Move selection |
| `NORMAL` | `Enter` | Open selected file |
| `NORMAL` | `Esc` / `q` | Quit |
| `INSERT` | Type | Edit query |
| `INSERT` | `Esc` | Return to normal mode |

# Configuration

`epoch` uses layered TOML configuration.

```
~/.config/epoch/config.toml
```

Optional project-local override:

```
<project>/.epoch/config.toml
```

Effective precedence:

1. Built-in defaults
2. Global config (`~/.config/epoch/config.toml`)
3. Project config (`.epoch/config.toml`)
4. CLI flags

Example:

```toml
tick_rate_ms = 100
parser = "auto"
theme = "system"           # classic | catppuccin | github | nord | gruvbox | solarized | dracula | system | custom
graph_mode = "line"        # sparkline | line
adaptive_layout = true
pinned_metrics = ["tokens_per_second"]
hidden_metrics = []
keymap_profile = "vim"     # default | vim
profile_target = "project" # global | project

[custom_theme]
header_bg = "#1e1e2e"
accent = "#89b4fa"
```

Notes:

- `theme = "system"` follows terminal/TTY color semantics (terminal default colors + ANSI palette), not desktop GTK/OS theme.
- `EPOCH_SYSTEM_THEME` can explicitly force a built-in palette when needed (for example `nord`, `dark`, `light`).
- Hidden metrics are UI visibility controls only; raw histories are still collected and available when re-enabled.

Settings mode (`s`) edits these values at runtime:

- `a`: apply without closing
- `w` or `Enter`: save and close
- `Esc`: cancel and restore original draft

# Behavior Guarantees

- Parser normalization strips ANSI/progress artifacts and handles invalid control bytes safely.
- Parser diagnostics track success/skipped/error counters and surface parser mode in status.
- Adaptive layout and metric relevance never discard raw metric history.
- Terminal restoration is preserved on exit and panic paths.

# License

MIT

<p align="center">
  <sub>Built with Rust and Ratatui.</sub>
</p>
