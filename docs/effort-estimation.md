# Effort Estimation – Football Management TUI Game (All Rust)

> Based on `features.md`, `backend.md`, and `architecture.md`.  
> Fully reconciled, conservative estimates for a solo Rust developer.

---

## 1. Reconciling the Three Documents

| Aspect | `backend.md` | `architecture.md` | **Synthesized (use this)** |
|--------|-------------|-------------------|----------------------------|
| Token size | 64-bit | 128-bit | **128-bit** – strong security, negligible memory cost |
| Command size | 24 bytes (token in every packet) | 10 bytes steady‑state (token cached after handshake) | **10 bytes** – smarter protocol |
| Event size | 20 bytes | 12 bytes | **12 bytes** – more compact |
| Snapshot frequency | Every 5 minutes | Every 30 seconds | **5 minutes** – avoids SSD wear on free tier |
| Journal batch size | 64 KB | 4 KB | **64 KB** – reduces write amplification |
| Horizontal scaling | Client‑side consistent hashing | Router + dispatcher | **Router + dispatcher** – simpler to implement and debug |
| Idle CPU | 3–5% (keepalive) | <1% (no keepalive) | **3–5%** – Oracle Free Tier will reclaim <1% idle |
| Recovery time | <10 sec | <5 sec | **<10 sec** – realistic for 300 MB snapshot |
| Max concurrent (free) | 200k | 200k–500k | **200k** – safe, testable upper bound |
| Memory per 200k matches | ~350 MB | ~280 MB + overhead | **~350 MB** – includes DashMap and queue overhead |

---

## 2. Backend Engine — Effort Breakdown

*A production‑ready engine handling 200k+ matches on Oracle Free Tier.*

| Component | Effort | Priority |
|-----------|--------|----------|
| **Algebraic simulation** – possession, shots, goals, deterministic RNG | 2–3 weeks | P0 |
| **SoA match state + DashMap lookup** – contiguous `Vec<MatchState>`, free‑list recycling | 1 week | P0 |
| **Binary protocol** – 10‑byte commands, 12‑byte events, handshake, batching | 1–2 weeks | P0 |
| **Token system** – `getrandom()` 128‑bit, server‑side caching after first packet | 3–5 days | P0 |
| **UDP receive loop** – `recv_mmsg`, validation, lock‑free queue (crossbeam) | 1 week | P0 |
| **Rate limiting** – 2 commands/sec per match, bounds‑check all args | 2–3 days | P0 |
| **Journal (io_uring + O_DIRECT)** – async append, 64 KB batching, CRC32C | 2–3 weeks | P1 |
| **Snapshots** – every 5 minutes, atomic rename, CRC32C | 1–2 weeks | P1 |
| **Crash recovery** – load latest snapshot, replay journal entries | 1 week | P1 |
| **CPU isolation** – `taskset`, core pinning, crossbeam channels | 3–5 days | P2 |
| **Keepalive simulation** – prevent Oracle reclamation (~3–5% CPU) | 1 week | P2 |
| **Prometheus metrics** – active matches, command rate, queue depth | 1 week | P2 |
| **Systemd service + health checks** | 2–3 days | P2 |
| **Router + dispatcher** – horizontal scaling (optional, for >200k matches) | 2–3 weeks | P3 |
| **Load testing** – synthetic 200k matches | 1–2 weeks | P3 |

**Backend engine total: 13–22 weeks (~3–5 months) for one senior Rust engineer.**

---

## 3. Full Game Features — Effort Breakdown

### Tier 1: Core Management Loop (MVP)

| Feature | Effort | Notes |
|---------|--------|-------|
| **TUI scaffold** – ratatui + crossterm, navigation, keybindings | 3–4 weeks | Learning curve for Rust TUI |
| **Backend integration** – Unix socket client in the same Rust binary | 1–2 weeks | Single binary: simpler deployment |
| **Manager profile & career system** – save/load manager state | 2–3 weeks | |
| **Basic league/competition engine** – fixtures, tables, cup draws | 4–6 weeks | |
| **Player database** – 10k generated players, attributes, positions | 3–4 weeks | |
| **Simple transfer system** – offers, budgets, basic AI | 2–3 weeks | |
| **Match simulation** – text commentary generated from engine events | 3–4 weeks | |
| **TUI screens** – squad, tactics, league table, match view | 4–6 weeks | ratatui widgets, async event loop |
| **Save/load career** – via engine snapshot mechanism | 1–2 weeks | |

**MVP subtotal: 23–32 weeks (~5–8 months).**

### Tier 2: Tactical Depth

| Feature | Effort |
|---------|--------|
| Two‑phase formation (possession / out-of-possession) | 3–4 weeks |
| Player roles (20+ roles with instructions) | 4–6 weeks |
| Match engine upgrades (smarter AI, event variety) | 6–8 weeks |
| Tactics visualiser (ASCII formation using ratatui Canvas) | 3–4 weeks |
| In‑match shouts & substitutions | 2–3 weeks |

**Tactical subtotal: 18–25 weeks (~4–6 months).**

### Tier 3: Scouting & Transfers

| Feature | Effort |
|---------|--------|
| Scouting system (assign scouts, generate reports) | 4–6 weeks |
| Transfer market (AI club behaviour, negotiations) | 3–4 weeks |
| Contract system (wages, clauses, agents) | 3–4 weeks |
| Loan system (playing time promises) | 2–3 weeks |

**Scouting subtotal: 12–17 weeks (~3–4 months).**

### Tier 4: International Management

| Feature | Effort |
|---------|--------|
| National team management | 3–4 weeks |
| World Cup mode (licensed 2026) | 4–6 weeks |
| Dual nationality system | 2–3 weeks |

**International subtotal: 10–15 weeks (~2–3 months).**

### Tier 5: Data, UI Polish & Quality of Life

| Feature | Effort |
|---------|--------|
| Data Hub / analytics (ASCII charts, trends) | 4–6 weeks |
| FMPedia (in‑game help) | 2–3 weeks |
| Delegation system (AI assistant) | 3–4 weeks |
| Mod support (custom databases) | 3–4 weeks |
| International expectations UI | 1–2 weeks |

**QoL subtotal: 13–19 weeks (~3–4 months).**

---

## 4. Consolidated Timelines

### Solo Developer (full-time)

| Scenario | Time |
|----------|------|
| **Backend engine only** (production‑ready) | 3–5 months |
| **MVP** (backend + career + basic tactics + TUI) | 9–12 months |
| **Full game** (all tiers, polished, mod support) | 3–4 years |

### Small Team (2–3 Rust developers)

| Scenario | Time |
|----------|------|
| Backend engine only | 2–3 months |
| MVP | 6–9 months |
| Full game | 14–20 months |

### Larger Team (5+ developers)

| Scenario | Time |
|----------|------|
| Full game | 10–14 months |

---

## 5. Recommended Phased Roadmap

| Phase | Focus | Duration | Deliverable |
|-------|-------|----------|-------------|
| **1** | Backend engine | Months 1–4 | Algebraic sim, UDP, persistence, metrics, load‑tested at 200k |
| **2** | MVP game | Months 4–9 | TUI scaffold, career, one league, basic tactics, match commentary |
| **3** | Tactical depth | Months 9–14 | Two‑phase formations, roles, improved AI, shouts |
| **4** | Scouting & transfers | Months 14–18 | Scouts, market, contracts, loans |
| **5** | International + World Cup | Months 18–21 | National teams, World Cup mode |
| **6** | Polish & QoL | Months 21–24 | Data Hub, delegation, mod support, performance |

---

## 6. Key Rust‑Specific Considerations

| Factor | Impact |
|--------|--------|
| **Async runtime** | Use Tokio for UDP, io_uring, and TUI event loop. TUI runs on main thread, engine on separate threads. |
| **TUI framework** | Ratatui + crossterm – most capable, active community, good async support. |
| **Dependencies** | `tokio`, `ratatui`, `crossterm`, `dashmap`, `serde`/`bincode`, `getrandom`, `crc32c`, `prometheus` (or custom), `clap`. |
| **Cross‑compilation** | Use `cross` or `cargo-zigbuild` to target Oracle ARM Ampere from x86 dev machine. |
| **Single binary** | All Rust → one binary (~10–15 MB release). Easier deployment, no cross‑language overhead. |
| **Testing** | Algebraic sim is easy to unit‑test. Network code needs integration tests with mock UDP. |

---

## 7. Conclusion

- **Backend engine** is well‑scoped and realistic at **3–5 months** for a solo Rust developer.
- **MVP game** that is actually playable and fun: **9–12 months**.
- **Full game** with all FM26‑inspired features: **3–4 years** solo, **14–20 months** with a small team.

The most pragmatic path is to **build the backend first** (months 1–4), then **layer the MVP game on top** (months 4–9). At that point you have something you can put in front of people, gather feedback, and decide how deep to go.