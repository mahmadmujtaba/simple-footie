# ⚽ simple-footie

**A football management simulation engine in Rust** — built for speed, built for the terminal.

[![CI](https://github.com/mahmadmujtaba/simple-footie/actions/workflows/ci.yml/badge.svg)](https://github.com/mahmadmujtaba/simple-footie/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/mahmadmujtaba/simple-footie)](https://github.com/mahmadmujtaba/simple-footie/releases)

---

## 🎮 Quick Start

### Download the latest release

Grab the latest tarball from the [Releases page](https://github.com/mahmadmujtaba/simple-footie/releases):

```bash
# Download and extract
tar xzf simple-footie-x86_64-unknown-linux-gnu.tar.gz
cd dist
```

### Option A: Run the HTTP control panel (recommended for trying it out)

```bash
./simple-footie-server
```

Then open **http://127.0.0.1:8080** in your browser.

You'll see a dark-themed control panel with:
- 📊 Live scoreboard (home vs away)
- 🎯 Tactical buttons — change mentality, press, tempo, width
- 📜 Real-time match events feed
- The page auto-refreshes every 2 seconds

### Option B: Run the terminal UI

In another terminal:

```bash
./simple-footie-tui
```

Navigate with `Tab` / arrow keys, press `s` to simulate a match, `q` to quit.

### Option C: Metrics

```bash
curl http://127.0.0.1:9090/metrics
```

Prometheus-formatted metrics: active matches, command rate, disk usage, etc.

---

## 🖥️ Screens

The TUI has **7 screens** you can navigate with `Tab` and arrow keys:

| Screen | What you see |
|--------|-------------|
| **Dashboard** | Upcoming fixtures + news feed |
| **Squad** | 18 players with Name, Age, Pos, OVR, POT, Nation, Contract, Value. OVR is color-coded (green ≥80, yellow ≥70, red <70) |
| **Tactics** | 4-4-2 formation diagram + starting XI with assigned roles |
| **League** | 16-team table with colored positions (green = top 2, yellow = top 4, red = relegation) |
| **Transfers** | Budget display (£12.5M) + transfer targets with interest % |
| **Scouting** | 5-region scouting network with progress bars |
| **Match** | Live score + full text commentary from the engine |

### Controls

| Key | Action |
|-----|--------|
| `Tab` / `→` | Next screen |
| `Shift+Tab` / `←` | Previous screen |
| `↓` / `j` | Scroll down |
| `↑` / `k` | Scroll up |
| `s` | Simulate a match |
| `q` / `Esc` | Quit |

---

## 🏗️ What's Under the Hood

```
simple-footie/
├── protocol/    # Binary protocol types (10-byte commands, 12-byte events)
├── engine/      # Algebraic simulation + SoA match store
├── server/      # UDP receiver, persistence, metrics, simulation core
├── tui/         # Terminal UI (ratatui + crossterm)
└── docs/        # Architecture & planning docs
```

### The Server

The game server listens on UDP `0.0.0.0:9001` and:

- **Authenticates** every command with 128-bit tokens
- **Rate limits** to 2 commands/sec per match
- **Simulates** matches at 2 match-minutes per real second
- **Persists** state via write-ahead journal + CRC32C-checked snapshots
- **Exposes** Prometheus metrics on localhost:9090

### The Engine

The algebraic simulation is **deterministic** — same inputs always produce the same match. No physics, just probability formulas for:
- Possession (based on midfield strength + tactic modifiers)
- Shots, shots on target, saves, goals
- Fouls and events

### Binary Protocol

| Packet | Size | Format |
|--------|------|--------|
| Command (steady) | 10 B | `match_id:u32, seq:u16, type:u8, args:[u8;3]` |
| Command (handshake) | 26 B | 10 B command + 16 B token |
| Event | 12 B | `match_id:u32, type:u8, team:u8, player:u16, value:f32` |

---

## 🔧 Build from Source

If you have Rust installed:

```bash
git clone git@github.com:mahmadmujtaba/simple-footie.git
cd simple-footie
make build        # Build everything
make server       # Run the game server
make tui          # Run the TUI client
make test         # Run all 25 tests
```

Or using Cargo directly:

```bash
cargo run --release -p server    # Game server
cargo run --release -p tui       # Terminal UI
cargo test                       # All tests
```

### Makefile targets

| Command | What it does |
|---------|-------------|
| `make build` | Build all crates in release mode |
| `make server` | Run the game server (UDP 9001) |
| `make tui` | Run the TUI client |
| `make test` | Run all 25 tests |
| `make check` | Fast compilation check |
| `make lint` | Run clippy |
| `make fmt` | Format code |
| `make clean` | Clean build artifacts |
| `make help` | Show all targets |

---

## 📊 Performance Targets

| Metric | Target |
|--------|--------|
| Concurrent matches (free tier) | 200,000 |
| Memory per match | ~80 bytes |
| Command latency (P99) | <2 ms |
| Recovery time | <10 sec |
| CPU at idle | 3–5% |

---

## 📦 Releases

We use [GitHub Releases](https://github.com/mahmadmujtaba/simple-footie/releases) with automatic binary builds triggered by version tags (`v*.*.*`).

Each release includes:
- Pre-built Linux x86_64 binaries (`simple-footie-server`, `simple-footie-tui`)
- Auto-generated changelog from commit history
- README included in the tarball

### Versioning

We follow **semantic versioning**:
- `v0.1.x` — Alpha releases (current)
- `v0.x.0` — Beta milestones
- `v1.0.0` — Stable release

---

## 🤝 Contributing

We use a **feature branch workflow** with **squash merges** to `main`.

```bash
# 1. Create a feature branch
git checkout -b feat/my-feature

# 2. Make changes and commit
git add .
git commit -m "feat: add my feature"

# 3. Push and open a Pull Request
git push -u origin feat/my-feature
# → Open PR at github.com/mahmadmujtaba/simple-footie

# 4. After review, squash-merge via GitHub UI
#    (branch auto-deletes, CI required, 1 review required)
```

### Rules

- ✅ **Squash merge only** — no merge commits or rebase merges
- ✅ **Branch protection** — `main` requires passing CI + 1 review
- ✅ **CI must pass** — `build-and-test` workflow runs on every PR
- ✅ **Delete branch on merge** — keeps the repo clean
- ✅ **Feature branches only** — never commit directly to `main`

### CI Pipeline

The `build-and-test` workflow runs on every push/PR:

1. **Format check** — `cargo fmt --check`
2. **Lint** — `cargo clippy -- -D warnings`
3. **Build** — `cargo build --release --workspace`
4. **Test** — `cargo test --release --workspace`
5. **Binary sizes** — reports `server` and `tui` sizes

---

## 📄 License

MIT