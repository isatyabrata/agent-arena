#!/usr/bin/env bash
# Stop the Agent Arena server (and public tunnel, if any).
cd "$(dirname "$0")/.."
for f in .lab/tunnel.pid .lab/server.pid; do
  [ -f "$f" ] && kill "$(cat "$f")" 2>/dev/null && echo "stopped $(basename "$f" .pid)"
  rm -f "$f"
done
rm -f .lab/token
exit 0
