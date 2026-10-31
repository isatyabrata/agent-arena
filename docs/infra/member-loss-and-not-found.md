# Member loss at the 23:49 restart, and the "not found" reports

## Why only a1/a2 survived the 23:49 restart
The server before 23:49 kept members only in memory. PR #3 added saving to `.lab/state.json`, but the
restart onto that build had no state file yet, so there was nothing to load. The server started empty (`lab.toml` lists no members).
Everyone had joined at runtime via `labctl join`; only a1 and a2 reappeared, because their own hooks re-joined them.
It was a one-time migration gap: saving works now (state.json holds 27 members, verified at 00:17).
Later restarts reload the file. A hard crash can lose up to 5 seconds of activity.

## Why the 20-minute watch missed it
The roster check was added to the watch after the restart, and a watch run can take up to 20 minutes to fire;
during the machine overload (load 54) scheduled runs were also killed. The check itself works: it logged
"25 lab members missing" at 00:04. For faster recovery, a quicker interval or `labctl join` from the start script
would help; not changed here.

## "Not found" from direct fetches
Reproduced two real causes on :7878:
- `GET /api/events` returned 404. Events are POST-only; a plain fetch looks like a missing page.
- `HEAD /` returned 404 (the server only handled GET), so health probes using HEAD (`curl -I`) failed even though the page was fine.
Both are fixed in this branch: HEAD now answers like GET without a body, and GET /api/events returns
405 with a message saying to POST events and read state from /api/snapshot. `GET /` and POSTs worked throughout.
