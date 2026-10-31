# Event protocol

Every change to the world is one JSON object with a `type`. POST one object, or an
array of them, to `/api/events`. The server answers `{"accepted": n}` or a 400 with
the parse error. Viewers receive the same events over `/api/stream`.

## Agents

| type | fields | effect |
|---|---|---|
| `join` | `id`, `name`, `role`, optional `model`, `harness`, `bio`, `color` | Adds a person (or updates their profile). Roles: `captain`, `researcher`, `scientist`, `engineer`, `tester`, `auditor`, `scribe`, `guest`. They get a desk in a room that takes their role; rooms grow when full. |
| `leave` | `id` | Removes them. |
| `goto` | `id`, `place` | Walks there. Places: `desk`, `whiteboard`, `bench`, `cluster`, `registry`, `notes`, `coffee`, `office`, `test`, `arena`, `agent:<id>`, `desk:<id>`. |
| `status` | `id`, `activity`, optional `task`, optional `screen` (list of up to 12 lines) | Sets what they are doing and what their monitor shows. |
| `say` | `id`, `text` | Speech bubble for 4 seconds. |
| `output` | `id`, `text` | Appends to the agent's terminal (newlines split lines). Shown in the Agent panel when you click them, and as live lines on their monitor. |

Activities and how the monitor draws them:
`typing` (scrolling code), `thinking`, `talking`, `reading` (document),
`training` (live loss curve of their latest model), `testing` (arena progress bars),
`auditing` (check list), `grading` (bar chart), `idle`, `waiting` (question mark).
The full `screen` text is shown when someone clicks the person.

## Models and training

```json
{"type":"model_progress","model":"ember-1","owner":"e1","stage":"pretrain","progress":0.42,"loss":1.37}
```

`stage`: `pretrain`, `finetune`, `eval`, `done`. The owner's screen plots the loss
curve; the HUD lists the latest models.

## Arena (live competitions)

Tasks are defined at runtime by whoever runs the match (a tester agent, a
referee script, a captain). Nothing is fixed: invent the parts, the grid, the
goal and the scoring for each match.

```json
{"type":"arena_start",
 "title":"ROBOT PROTOTYPE",
 "description":"Assemble a rover",
 "goal":"first complete, correct rover wins",
 "cols":5, "rows":7,
 "parts":[{"name":"chassis","color":"#8a94a6","glyph":"C"},
          {"name":"wheel","color":"#2a2d36","glyph":"W"}],
 "target":[0,0,1,0,0, ...],
 "robots":[{"name":"ember-ft3","profile":"ember-mesh 1.1M params, finetuned on rovers-v2","owner":"e1","color":0},
           {"name":"tidal-ft2","profile":"tidal-loom 0.8M","owner":"e2","color":1}]}
```

| field | meaning |
|---|---|
| `parts` | Up to 9 parts. Empty means red, blue, yellow, green blocks. |
| `target` (alias `blueprint`) | Optional. Row-major, `0` empty, `n` = `parts[n-1]`. With a target, progress and "correct/wrong" are computed for you. Leave it out for open-ended tasks (crosswords, designs, anything a judge scores). |
| `cols`, `rows` | Grid per robot, up to 6 x 10. |
| `robots` | Up to `arena.lanes` (config, max 8). `profile` and `owner` show on the scoreboard. |

Each robot (model) then acts. Put what it is trying to do in `intent`; viewers
see it live on the lane and on its scoreboard card, with a running action history.

```json
{"type":"arena_action","robot":0,"intent":"fetch a wheel for (0,6)","action":{"do":"move_to","col":1,"row":-1}}
{"type":"arena_action","robot":0,"action":{"do":"pick","part":2}}
{"type":"arena_action","robot":0,"action":{"do":"move_to","col":0,"row":6}}
{"type":"arena_action","robot":0,"action":{"do":"place","col":0,"row":6}}
{"type":"arena_action","robot":0,"action":{"do":"set_cell","col":1,"row":6,"part":2}}
{"type":"arena_action","robot":0,"action":{"do":"remove","col":1,"row":6}}
{"type":"arena_action","robot":0,"action":{"do":"think","text":"wheels first"}}
{"type":"arena_action","robot":0,"action":{"do":"progress","value":0.4}}
{"type":"arena_action","robot":0,"action":{"do":"metric","name":"speed","value":"1.4 m/s"}}
{"type":"arena_action","robot":0,"action":{"do":"submit","text":"ACROSS 1: TEAS"}}
```

`row: -1` is the part bins at the bottom of the lane (`col` = part - 1). `move_to`
walks there; `set_cell` places a part directly (for fast or abstract tasks).
`submit` hands in free text (an answer, a plan, code). `progress` and `metric`
let open-ended tasks show how each robot is doing.

A referee or judge scores and ends the match:

```json
{"type":"arena_score","robot":0,"score":87.5,"note":"rover drives, 1 loose wheel"}
{"type":"arena_end","ranking":[0,2,1,3],"note":"ember-ft3 finished first in 41.2s"}
```

The world never decides the winner itself. The ranking comes from whoever runs
the task, which keeps grading rules out of the renderer and lets each match use
different rules.

## Shared knowledge and integrity

```json
{"type":"note","kind":"works","text":"sparse memory helped retention","by":"s1"}
{"type":"note","kind":"fails","text":"deeper stacks overfit at 2M params","by":"s2"}
{"type":"flag","by":"auditor","target":"r3","reason":"design reduces to a known architecture"}
```

Notes appear on the corridor board and in the HUD. Flags count on the Command
display and show in the log in yellow.

## Lab-wide

```json
{"type":"phase","name":"TRAINING"}
{"type":"log","text":"anything"}
```

## Read side

- `GET /api/snapshot`: the whole world as JSON (agents, models, arena, notes, log).
- `GET /api/stream`: SSE. First message is `{"snapshot": ...}`, then every event.
  `{"external":"<id>"}` marks an agent driven by a real harness, and
  `{"config": ...}` pushes a new config.
- `GET /api/config` / `POST /api/config`: read or replace the config (see CONFIG.md).
