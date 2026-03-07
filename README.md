<p align="center">
  <img src="docs/logo.svg" alt="epoch" width="100%" />
</p>

<p align="center">
  <strong>The terminal-native home for training runs.</strong>
</p>

<p align="center">
  Monitor live runs, inspect system health, compare experiments, and stay close to training —
  all from the terminal or over SSH.
</p>

<p align="center">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-292929?style=for-the-badge&logo=rust&logoColor=white"/>
  <img alt="Ratatui" src="https://img.shields.io/badge/Ratatui-292929?style=for-the-badge"/>
  <img alt="Interface" src="https://img.shields.io/badge/Interface-Terminal-C0A359?style=for-the-badge"/>
  <img alt="Linux" src="https://img.shields.io/badge/Linux-supported-46B980?style=for-the-badge&logo=linux&logoColor=white"/>
  <img alt="macOS" src="https://img.shields.io/badge/macOS-supported-46B980?style=for-the-badge&logo=apple&logoColor=white"/>
  <img alt="License" src="https://img.shields.io/badge/License-MIT-597BC0?style=for-the-badge"/>
</p>

---

<p align="center">
  <img src="docs/demo.gif" alt="epoch demo" width="900" />
</p>

---

## Quick start

```bash
git clone https://github.com/GranneJanne/epoch.git
cd epoch
cargo build --release

# watch a log file
./target/release/epoch train.log

# or pipe directly from a training script
python train.py 2>&1 | ./target/release/epoch --stdin

# or let epoch discover a run in the current directory
./target/release/epoch
```

When launched without arguments, `epoch` searches the current directory for recent training logs and tries to attach automatically.

## Why epoch

Training still feels fragmented when you live in logs, tmux, and SSH.

You tail raw output in one pane, watch GPU stats in another, guess whether loss is healthy, and lose context between runs. Browser-first tools can help, but they often feel far away from the actual training loop.

`epoch` is built to be the place you stay during training: standalone, local-first, fast, SSH-friendly, and useful from anywhere on the machine.

It starts with live observability, but the goal is bigger: a terminal environment for understanding runs, not just watching them.

<p align="center">
  <img src="docs/flow.svg" alt="epoch flow: logs and training outputs into a live terminal environment" width="860" />
</p>

## What it does

`epoch` is built for the during-training experience.

It helps you:

- monitor live loss, learning rate, throughput, steps, and timeline history
- inspect GPU, VRAM, CPU, and memory usage alongside training
- keep track of remote jobs over SSH
- pipe output from ad hoc scripts without changing your workflow
- launch inside a project, point at a log, or let it discover likely runs

In short, `epoch` sits between raw training output and actual understanding.

## Supported inputs

`epoch` currently works with common training log styles, including JSONL, CSV, regex-parsed logs, and Hugging Face `trainer_state.json`.

Example JSONL:

```json
{ "loss": 0.53, "step": 120, "lr": 1e-4 }
```

Example CSV:

```csv
step,loss,lr
120,0.53,0.0001
```

## Keybindings

| Key                 | Action                              |
| ------------------- | ----------------------------------- |
| `q` / `Ctrl+C`      | Quit                                |
| `Tab` / `Shift+Tab` | Switch views                        |
| `1-4`               | Focus graph (Loss, Eval, LR, Grad)  |
| `Space`             | Toggle live/pause (all viewports)   |
| `Left/Right`        | Pan active graph history            |
| `- / =`             | Zoom active graph out/in            |
| `g`                 | Reset all viewports to live         |
| `s`                 | Open settings                       |
| `?`                 | Toggle help overlay                 |

## Configuration

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
run_comparison_file = "baseline.jsonl"

[[alert_rules]]
kind = "throughput_drop"
warning = 200.0
critical = 120.0
enabled = true
cooldown_secs = 30
evaluation = "rolling"
window = 10

[custom_theme]
header_bg = "#1e1e2e"
accent = "#89b4fa"
```

## Roadmap

The long-term goal is not to build a better log viewer. It is to make `epoch` the place you stay during training — an operating environment, not a one-shot dashboard.

Development follows five pillars:

1. **Live observability** — excellent while training is happening
2. **Habitable environment** — inspect, compare, annotate, and navigate, not just watch
3. **Standalone and local-first** — no accounts, no cloud, no required web connection
4. **Anywhere on the machine** — discover projects, runs, and processes from any directory
5. **Model understanding** — make architecture and training state legible, not just metrics

### Phase 1: Foundations

- local run store and project resolution
- active process discovery
- home view with orientation and entry points
- live run monitoring
- run explorer with filtering and search
- launch-from-anywhere support
- notes, bookmarks, and event timeline

### Phase 2: Habitable environment

- run comparison (config diff, curve overlay, metric summary)
- artifact browser (checkpoints, configs, logs, eval outputs)
- session memory (remembered runs, comparisons, context)
- global finder / command palette
- attach and resume workflows
- alerting and anomaly surfacing

### Phase 3: Model understanding

- model structure visualization (module hierarchy, block flow)
- parameter and trainability summaries
- frozen / trainable / adapter overlays
- model diff between runs or checkpoints

### Phase 4: Advanced operational insight

- runtime overlays (gradient norms, memory hotspots, latency per block)
- distributed training awareness (rank status, desync detection)
- deeper framework adapters (PyTorch Lightning, DeepSpeed, Accelerate)
- richer eval and sample inspection

### Phase 5: Optional expansions

- exportable summaries and reports
- optional sync and sharing layer
- plugin ecosystem
- TensorBoard event file import

## Contributing

Contributions are welcome. Start here: [Contributing guide](./CONTRIBUTING.md)

High-leverage areas right now:

- parsers and framework integrations
- run discovery and process attach
- comparison workflows
- terminal UX and interaction design
- model visualization

## License

MIT
