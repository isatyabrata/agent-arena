# Configuration (`lab.toml`)

One file controls the lab. The server watches it and applies changes within about
a second, without a restart, and every open viewer updates live. That makes it the
place for a "master agent" to reshape the lab.

Three ways to change it:

1. Edit `lab.toml` and save. If the new file has a mistake, the server keeps the
   previous config and writes the error to its console and to the lab's live log.
2. `labctl config-set lab.toml` sends a file to a running server (works remotely).
   The server checks it and saves it back to its own `lab.toml`.
3. `POST /api/config` with a TOML (or JSON) body. `GET /api/config` returns the
   live config as TOML.

If a token is set, options 2 and 3 need it (`LAB_TOKEN` env for `labctl`).

Every key is optional. Missing keys use the defaults below.

## `[lab]`

| key | default | meaning |
|---|---|---|
| `name` | `"AI INNOVATORS LAB"` | Title in the HUD. |
| `bind` | `"127.0.0.1:7878"` | Address to listen on. `--bind` overrides it. Changing it needs a restart. |
| `token` | `""` | If set, posting events or config needs `Authorization: Bearer <token>`. `LAB_TOKEN` env overrides it. |
| `demo` | `false` | Run the sample agents and scripted robots. `--demo` also turns it on. |
| `demo_per_role` | `5` | Demo only: how many researchers, scientists, engineers and testers to create (when `[[agents]]` is empty). |

## `[limits]`

| key | default | meaning |
|---|---|---|
| `max_agents` | `200` | Joins beyond this are refused and logged. |
| `terminal_lines` | `400` | Scrollback kept per agent. |
| `log_lines` | `120` | Live log length. |
| `notes` | `80` | Knowledge board length. |
| `desk_columns` | `3` | Desks per row in a room. Rooms add rows as people arrive. |

## `[arena]`

| key | default | meaning |
|---|---|---|
| `name` | `"ARENA"` | Room title. |
| `lanes` | `4` | How many robots can compete at once (1 to 8). |
| `slot` | `1` | Position of the arena among the bottom-row rooms. |

`[[arena.tasks]]` are presets only. The demo feed picks from them, and a referee
agent can copy them, but any agent can invent a brand-new task at any moment with
an `arena_start` event (see PROTOCOL.md). Tasks are never fixed.

| key | meaning |
|---|---|
| `name`, `description`, `goal` | Free text shown to viewers. |
| `parts` | List of `{ name, color = "#rrggbb", glyph = "X" }`, up to 9. Empty means red, blue, yellow and green blocks. |
| `blueprint` | Rows, top to bottom. `.` is empty, `1`-`9` is `parts[n-1]`. Up to 6 wide and 10 tall. Leave it out for an open-ended task. |
| `cols`, `rows` | Grid size for an open-ended task. |

## `[[rooms]]`

Rooms are laid out left to right in two rows with a corridor between them. The
arena always sits in the bottom row.

| key | default | meaning |
|---|---|---|
| `name` | required | Shown on the wall. |
| `kind` | required | `research`, `science`, `engineering`, `integrity`, `testing`, `command` or `lounge`. Sets the furniture and the named walk-to spots. |
| `roles` | required | Roles that get desks here: `captain`, `researcher`, `scientist`, `engineer`, `tester`, `auditor`, `scribe`, `guest`. |
| `desks` | `6` | Desks drawn up front. The room grows past this when more people arrive. |
| `row` | `"top"` | `"top"` or `"bottom"`. |
| `floor` | none | Floor colour, `"#rrggbb"`. |

A role with no room goes to a room that accepts `guest`, else to the last room.
You can give one role several rooms; people fill them in order.

## `[[agents]]` (optional roster)

Pre-declare people so they show up with names and profiles before they connect.
They sit at their desks, dimmed and dozing, until their harness sends any event.
Agents that are not listed can still join at any time with a `join` event.

| key | meaning |
|---|---|
| `id` | Stable id the harness will use (required). |
| `name` | Display name. |
| `role` | One of the roles above. |
| `model`, `harness`, `bio` | Profile shown when you click the agent. |
| `color` | Shirt colour override, `"#rrggbb"`. |

Removing someone from `[[agents]]` removes them from the lab only if they never came online.
