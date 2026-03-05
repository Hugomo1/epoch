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
      --parser <TYPE>    Override log parser (auto, jsonl, csv, regex)
  -h, --help             Print help
  -V, --version          Print version
```

Examples:

```bash
epoch train.log
epoch --parser jsonl train.log
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

Epoch works with common training log styles.

**Currently supported**

```
JSONL
{"loss": 0.53, "step": 120, "lr": 1e-4}
```

```
CSV
step,loss,lr
```

```
Regex
custom training logs
```

```
HuggingFace trainer_state.json
```

More integrations are planned.

# Keybindings

| Key               | Action                      |
| ----------------- | --------------------------- |
| `Tab` / `→`       | Next panel                  |
| `Shift+Tab` / `←` | Previous panel              |
| `1 2 3 4`         | Jump to panel               |
| `Space`           | Pause / resume live updates |
| `Left / Right`    | Scroll history              |
| `- / =`           | Zoom timeline               |
| `g`               | Return to live view         |
| `q`               | Quit                        |

# Configuration

`epoch` can be configured with a small TOML file.

```
~/.config/epoch/config.toml
```

Example:

```toml
tick_rate_ms = 100
parser = "auto"
```

# Coming Next

```
[ ] TensorBoard log support
[ ] Run comparison view
[ ] smoother loss graphs
[ ] HuggingFace Trainer integration
[ ] distributed training monitoring
```

# License

MIT

<p align="center">
  <sub>Built with Rust and Ratatui.</sub>
</p>
