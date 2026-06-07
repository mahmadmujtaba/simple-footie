# ⚽ simple-footie — Football Management TUI Game Engine

[![CI](https://github.com/mahmadmujtaba/simple-footie/actions/workflows/ci.yml/badge.svg)](https://github.com/mahmadmujtaba/simple-footie/actions/workflows/ci.yml)

A **high-performance, event-driven football simulation engine** written in Rust,
designed to support **200k+ concurrent matches** on Oracle Cloud Free Tier.

---

## Architecture

```
fm-rust/
├── protocol/    # Binary protocol types (10-byte commands, 12-byte events)
├── engine/      # Algebraic simulation + SoA match store
├── server/      # UDP receiver, persistence, metrics, simulation core
├── tui/         # Terminal UI (ratatui + crossterm)
└── docs/        # Architecture & planning docs
```

### Crates

| Crate | Role | Status |
|-------|------|--------|
| `protocol` | Shared binary types (CommandPacket, EventPacket, MatchState) | ✅ Complete |
| `engine` | Algebraic simulation, match store, command application | ✅ Complete |
| `server` | UDP network, token auth, persistence, metrics, sim core | ✅ Phase 2–3 |
| `tui` | Ratatui terminal client (7 screens, live match sim) | ✅ Prototype |

---

## Quick Start

```bash
# Run the server (UDP on port 9001)
make server

# Run the TUI client
make tui

# Run all tests
make test

# Build everything in release mode
make build
```

### Controls (TUI)

| Key | Action |
|-----|--------|
| `Tab` / `→` | Next screen |
| `Shift+Tab` / `←` | Previous screen |
| `s` | Simulate a match |
| `q` / `Esc` | Quit |

### Screens

- **Dashboard** — Upcoming fixtures + news feed
- **Squad** — 18-player table with color-coded OVR ratings
- **Tactics** — 4-4-2 formation + starting XI with roles
- **League** — 16-team table with colored positions
- **Transfers** — Budget + transfer targets with interest %
- **Scouting** — 5-region scouting network
- **Match** — Live score + commentary from engine simulation

---

## Server

The server listens on UDP `0.0.0.0:9001`. It:

- **Receives** 10-byte commands (steady-state) or 26-byte handshakes (with 128-bit token)
- **Authenticates** every command against per-match tokens
- **Rate limits** to 2 commands/sec per match
- **Simulates** matches using the algebraic engine (2 match-min per real second)
- **Persists** state via write-ahead journal + snapshots (every 5 min)
- **Exposes** Prometheus metrics on `localhost:9090/metrics`

### Binary Protocol

| Packet | Size | Format |
|--------|------|--------|
| Command (steady) | 10 B | `match_id:u32, seq:u16, type:u8, args:[u8;3]` |
| Command (handshake) | 26 B | 10 B command + 16 B token |
| Event | 12 B | `match_id:u32, type:u8, team:u8, player:u16, value:f32` |

---

## Phase 3 — Persistence

- **Journal**: Append-only write-ahead log with 64 KB batching + CRC32C checksums
- **Snapshots**: Full state dump every 5 minutes, atomic rename
- **Recovery**: Load latest snapshot + replay journal ≥ last sequence
- **Metrics**: Prometheus endpoint tracking active matches, command rate, disk usage

---

## Performance Targets

| Metric | Target |
|--------|--------|
| Concurrent matches (free tier) | 200,000 |
| Memory per match | ~80 bytes |
| Command latency (P99) | <2 ms |
| Recovery time | <10 sec |
| CPU at idle | 3–5% (keepalive) |

---

## Contributing

We use a **feature branch workflow** with **squash merges** to `main`.

### Workflow

```bash
# 1. Create a feature branch from main
git checkout -b feat/my-feature

# 2. Make changes and commit
git add .
git commit -m "feat: add my feature"

# 3. Push and open a Pull Request
git push -u origin feat/my-feature
# → Open PR on github.com/mahmadmujtaba/simple-footie

# 4. After review, squash-merge via GitHub UI
#    (automatic: delete branch on merge, squash commit)
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

## License

MIT