# Drafft.ink


<img src="./logo.png" alt="Drafft.ink Logo" width="120" align="left">

**An infinite canvas whiteboard built with Rust and WebGPU.**

Try it now: [drafft.ink](https://drafft.ink/) — draw first, sign up never.

Cross-platform (Linux, Windows, macOS, browser, mobile). Real-time collaboration via CRDTs. No account required. Self-hostable with a single binary.

<br clear="left"/>

<img width="874" alt="Screenshot" src="./screenshot.png" />

---

## Features

- **Shapes and Drawing** - Rectangles, ellipses, lines, arrows, freehand paths with pressure sensitivity
- **Smart Guides** - Smart alignment snapping, equal spacing detection, angle snapping
- **Text** - Multiple font families (GelPen, GelPen Serif, Vanilla Extract), per-character styling, inline LaTeX math
- **Images** - Drag-and-drop, paste from clipboard, embedded in document
- **Collaboration** - Real-time sync via Loro CRDT. Watch your colleagues draw boxes around things that don't need boxes.
- **Open Formats** - Export to PNG or JSON. Import them back.
- **No Telemetry** - We don't know what you're drawing, and frankly, we don't want to.
- **Touch Support** - iPad and tablet friendly, gesture navigation
- **Sketch Style** - Sketchy on purpose. Precise when it matters. Hand-drawn aesthetic via roughr and fonts

---

## Installation

### Toolchain (recommended)

Uses [mise](https://mise.jdx.dev) so Rust, `wasm-pack`, and the WASM target stay out of your global install:

```bash
mise trust
mise install
```

Pinned in [`mise.toml`](mise.toml) and [`rust-toolchain.toml`](rust-toolchain.toml) (matches CI: Rust 1.91.1, `rustfmt`, `clippy`, `wasm32-unknown-unknown`).

### Desktop

```bash
git clone https://github.com/PatWie/drafft-ink.git
cd drafft-ink
mise install   # if using mise
cargo run --release
```

Or use the build script:

```bash
./build.sh --native
```

### Web (Local)

```bash
mise install   # wasm-pack + wasm target
./build.sh --wasm
```

### Collaboration Server

```bash
cargo build --release -p drafftink-server
./target/release/drafftink-server
```

Listens on `ws://localhost:3030/ws` by default.

**Persistence (Kubernetes / self-host)**

| `STORE` | Env | Use |
|---------|-----|-----|
| `memory` | (default) | Dev only; rooms lost on restart |
| `file` | `PERSISTENCE_DIR=/data/rooms` | Single relay replica + PVC |
| `redis` | `REDIS_URL=redis://host:6379` | Multi-replica; requires `cargo build -p drafftink-server --features redis` |

Other env: `PORT`, `HOST`, `ROOM_TTL_SECS`, `MAX_ROOM_BYTES`.

Example file-backed server:

```bash
STORE=file PERSISTENCE_DIR=./data/rooms ./target/release/drafftink-server
```

**WASM deploy config** (build-time):

```bash
DRAFFTINK_DEFAULT_WS=/ws DRAFFTINK_HIDE_SERVER_URL=true ./build.sh --wasm
```

Or runtime override in `web/index.html` via `window.__DRAFFTINK_COLLAB__`.

**Kubernetes:** see [deploy/k8s/](deploy/k8s/) (single-replica PVC + optional Redis for scale).

Share links use `?room=<id>` (and `&server=` when the relay is on another host). Use **Start shared room** in the collab UI to generate a room id and update the browser URL.

---

## Architecture

```
crates/
  drafftink-core/     # Canvas state, shapes, CRDT sync, snapping logic
  drafftink-render/   # Vello-based GPU rendering, text layout (Parley)
  drafftink-app/      # Application logic, UI (egui), event handling
  drafftink-server/   # WebSocket collaboration server
  drafftink-widgets/  # Custom UI components
```

---

## Philosophy

Your tools should work for you. No accounts, no paywalls, no telemetry, no "upgrade to Pro."

---

## Contributing

PRs welcome. Issues welcome. The code is right here.

---

## License

**AGPLv3** - Use it, modify it, host it. Keep it open.
