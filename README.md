<div align="center">

<img src="docs/logo.svg" width="100%" alt="epoch logo"/>

<br>

[![build](https://img.shields.io/badge/build-passing-21262d?style=flat-square)]()
[![kernel](https://img.shields.io/badge/kernel-rust-21262d?style=flat-square)]()
[![telemetry](https://img.shields.io/badge/telemetry-active-21262d?style=flat-square)]()

<br>

</div>

**epoch** is a terminal-native telemetry console for AI/ML training workloads.

It provides **real-time observability for model training** directly in the terminal — combining **training metrics** and **hardware telemetry** into a single responsive interface.

<!-- TODO: record demo → uncomment below -->
<!-- ## Visual Proof -->
<!-- ![demo](docs/demo.gif) -->

## ❯ Capabilities

```
╔═════════════════════════════════════════════════════════════════════╗
║                     EPOCH TELEMETRY MATRIX                          ║
╠════════════════════════════╦════════════════════════════════════════╣
║ TRAINING METRICS           ║ HARDWARE TELEMETRY                     ║
╠════════════════════════════╬════════════════════════════════════════╣
║ Loss monitoring            ║ GPU utilization                        ║
║ Learning rate tracking     ║ VRAM usage                             ║
║ Training steps / epochs    ║ CPU load                               ║
║ Tokens / samples per sec   ║ System memory                          ║
║ Throughput visualization   ║ Optional NVML GPU support              ║
╚════════════════════════════╩════════════════════════════════════════╝
```

Additional system capabilities:

- **Multi-view TUI**
  Dashboard / Metrics / System tabs

- **Multiple log formats**
  JSONL + custom regex patterns

- **Pipe-based streaming input**

## Protocol

### Installation

```bash
$ git clone https://github.com/grannejanne/epoch.git
$ cd epoch
$ cargo build --release
$ ./target/release/epoch
```

Optional CPU-only build:

```bash
$ cargo build --release --no-default-features
```

### Monitor a training log

```bash
$ epoch train.log
```

### Stream directly from a training process

```bash
$ python train.py 2>&1 | epoch --stdin
```

This allows **zero-integration monitoring** without modifying training scripts.

### Override parser

```bash
$ epoch --parser regex train.log
```

Supported parsers:

```
auto
jsonl
csv
regex
```

## Interaction

### Keyboard Controls

```
┌──────────────┬─────────────────────────┐
│ Key          │ Action                  │
├──────────────┼─────────────────────────┤
│ Tab / →      │ Next tab                │
│ Shift+Tab / ←│ Previous tab            │
│ 1 2 3        │ Jump to tab             │
│ q / Ctrl+C   │ Quit                    │
└──────────────┴─────────────────────────┘
```

## Stream Formats

### Current (v0.1.0)

```
JSONL
{"loss": 0.53, "step": 120, "lr": 1e-4}
```

```
Regex
custom framework training logs
```

### Planned

```
CSV training logs
TensorBoard event files
```

## Configuration

Configuration file:

```
~/.config/epoch/config.toml
```

Example:

```toml
refresh_rate_ms = 100
parser = "auto"
```

## Future

```
SYSTEM_DIAGNOSTICS
```

```
[ ] TensorBoard stream ingestion
[ ] Multi-run comparison
[ ] Training loss graph smoothing
[ ] Distributed training telemetry
[ ] HuggingFace Trainer integration
[ ] WebSocket metric streaming
```

## ❯ License

MIT — see `LICENSE` for details.
