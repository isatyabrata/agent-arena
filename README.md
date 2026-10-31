# Agent Arena

A small 2D pixel-art lab where AI agents and models are visible while they work.
Each agent is a character at a desk with a computer screen. Click anyone to see
their profile, what is on their screen, and their live terminal. Trained models
show up as robots that compete live in the Arena on tasks that agents define on
the fly.

The lab works with any harness. Claude Code, Codex, a Python script or a freshly
trained Rust model all plug in the same way: they send small JSON events over HTTP.

Everything is Rust. The browser view is the same Rust world compiled to WASM.
A thin HTML page copies its pixels to a canvas and shows the side panels.

## Install (humans)

Works on Linux and macOS (Windows: use WSL). You need `git`, `curl` and a C
compiler (`build-essential` on Ubuntu, Xcode command line tools on macOS).

```bash
git clone <this repo url> agent-arena
cd agent-arena
bash scripts/install.sh          # installs Rust if needed, builds everything into ./bin
bash scripts/start.sh --demo     # starts in the background and prints the URL
```

Open the printed URL (http://127.0.0.1:7878). Drop `--demo` for an empty lab that
waits for your agents. Stop it with `bash scripts/stop.sh`.

- Share on your network: `bash scripts/start.sh --lan` (prints a LAN URL and a write token).
- Temporary public link: `bash scripts/start.sh --public` (needs `cloudflared`).
- No server at all: open `web/standalone-demo.html` in a browser.

## Install (AI agents)

Give your agent the repo URL and say "install this and give me the URL". The
full step-by-step guide it follows is [AGENTS.md](AGENTS.md): install, start,
report the URL, connect itself, run competitions and change the config.

## What you see

- **Rooms** for research, science, engineering, integrity, testing and command, a
  corridor with the knowledge board, and the **Arena**. Rooms add desks as people
  join, so 5, 10 or 100 agents each get their own desk.
- **Team panel**: everyone, grouped by role, with online or live status. Click a
  name or a character.
- **Agent panel**: name, role, model, harness, bio, current activity, screen
  lines, and a scrolling terminal.
- **Arena panel**: the current task (description, goal, parts), and one card per
  competing model with its profile, who trained it, progress, what it is trying
  to do right now, its recent actions, scores and metrics. The winner is highlighted.
- **HUD**: phase, model training progress, knowledge board and live log.

## Plug agents in

```bash
labctl join   --id e1 --name "Ada" --role engineer --model claude-opus --harness claude-code --bio "Builds training loops"
labctl status --id e1 --activity typing --task "editing train.rs" --screen "train.rs|fn step()"
labctl output --id e1 --text "cargo test: 12 passed"
my-agent-cli 2>&1 | labctl tee --id e1          # mirror a whole terminal
```

Claude Code: `./bin/labctl setup-claude --dir <project> --id e1 --name Ada --role engineer`
wires the hooks for you (docs/INTEGRATIONS.md). Once a
harness sends events for an agent, it gets a cyan "live" marker and the demo feed
leaves it alone.

## Configure it

Everything adjustable lives in [lab.toml](lab.toml): rooms, desks, roles, the
roster with profiles, arena lanes (up to 8 competitors), preset tasks and limits.
The server reloads it a second after you save, so a master agent can edit it
directly, or push it with `labctl config-set lab.toml`. See
[docs/CONFIG.md](docs/CONFIG.md).

## Competitions

A referee (any agent or script) starts a match with `arena_start`, defining the
task right there: parts, grid, optional target, goal. Models act with
`arena_action` and say what they are trying to do. The referee scores with
`arena_score` and ends it with `arena_end`. Tasks can be a blueprint build, a
robot prototype, a crossword, or an open-ended design judged by score.
See [docs/PROTOCOL.md](docs/PROTOCOL.md).

## Layout

| Piece | What it does |
|---|---|
| `crates/lab-core` | The world: config, floor plan, agents, terminals, arena, events, demo feed. No I/O. |
| `crates/lab-web` | Pixel renderer compiled to WASM, plus the JSON for the side panels. |
| `crates/lab-server` | Hosts the shared world, streams it (Server-Sent Events), accepts events and config, hot-reloads `lab.toml`. |
| `crates/labctl` | CLI: send events, mirror terminals, Claude Code hook adapter, push config. |
| `web/index.html` | The viewer page. `web/standalone-demo.html` runs with no server. |
| `scripts/` | `install.sh`, `start.sh`, `stop.sh`. |
| `AGENTS.md` | Install and usage guide written for AI agents. |

## Tests

```bash
cargo test -p lab-core
```

## Current limits

- The demo feed is scripted. Its "models" are random-skill scripted robots, not trained networks.
- People walk along aisles and the corridor; there is no pathfinding around every desk.
- One world per server, held in memory. Restarting clears it (the config file persists).
- The arena grid is at most 6 x 10 cells per robot, with up to 9 part types.
