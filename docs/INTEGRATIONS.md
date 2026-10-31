# Plugging in harnesses and models

The lab only needs HTTP POSTs of JSON events. Pick whatever is easiest.

## Claude Code (hooks)

Quickest: `./bin/labctl setup-claude --dir <project> --id e1 --name Ada --role engineer --model claude-opus`
writes the block below into `<project>/.claude/settings.local.json` for you.

`labctl hook` reads the JSON that Claude Code passes to every hook and turns it
into lab events: it joins on session start, sets the activity per tool (editing,
reading, running commands), and copies prompts, commands and their output into
the agent's terminal. Put `labctl` on your PATH and add this to the project's
`.claude/settings.json` (change `LAB_AGENT`, `LAB_NAME` and `LAB_ROLE` per agent):

```json
{
  "env": { "LAB_AGENT": "e1", "LAB_NAME": "Ada", "LAB_ROLE": "engineer", "LAB_MODEL": "claude-opus", "LAB_URL": "http://127.0.0.1:7878" },
  "hooks": {
    "SessionStart":     [{ "hooks": [{ "type": "command", "command": "labctl hook" }] }],
    "UserPromptSubmit": [{ "hooks": [{ "type": "command", "command": "labctl hook" }] }],
    "PreToolUse":       [{ "matcher": "*", "hooks": [{ "type": "command", "command": "labctl hook" }] }],
    "PostToolUse":      [{ "matcher": "*", "hooks": [{ "type": "command", "command": "labctl hook" }] }],
    "Notification":     [{ "hooks": [{ "type": "command", "command": "labctl hook" }] }],
    "Stop":             [{ "hooks": [{ "type": "command", "command": "labctl hook" }] }]
  }
}
```

`labctl hook` always exits 0, so a stopped lab server never blocks Claude Code.
Five agents means five settings files (or five `LAB_AGENT` values); each one shows
up at its own desk with its name and profile, and clicking it shows its terminal.

## Mirror any terminal

Pipe any program through `labctl tee`. It still prints everything, and also
streams each line to that agent's terminal in the lab:

```bash
labctl join --id c1 --name "Codex 1" --role engineer --model gpt-5-codex --harness codex
codex exec "fix the failing test" 2>&1 | labctl tee --id c1
```

## Codex, Aider, other CLIs

Any harness that can run a shell command on an event can call `labctl status`
and `labctl output` the same way. Ones that cannot can be wrapped: run the harness, read its log or JSONL output,
and translate lines into events (`labctl pipe` reads one JSON event per line from
stdin and posts them as a batch).

## An agent framework in any language

POST JSON:

```python
import json, urllib.request
def lab(ev):
    urllib.request.urlopen(urllib.request.Request(
        "http://127.0.0.1:7878/api/events", json.dumps(ev).encode(),
        {"Content-Type": "application/json"}))
lab({"type": "status", "id": "r1", "activity": "thinking", "task": "new idea", "screen": ["hypothesis:", "..."]})
```

## A trained model competing in the Arena

The model process (Rust, per department rules) is the robot's brain. A referee
sends `arena_start`, then asks each model for its next action from what it can
see (blueprint plus its own grid), posts the action as `arena_action`, and sends
`arena_end` with the ranking. The referee, not the model, should own the world
state, so a model cannot cheat by writing to another robot's lane or reading
hidden state. The Integrity Auditor can watch the same stream.

## Security

The server binds to `127.0.0.1` by default. If you bind to a public address, set
`LAB_TOKEN`; `labctl` sends it automatically from the `LAB_TOKEN` env var.
