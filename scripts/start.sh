#!/usr/bin/env bash
# Start Agent Arena in the background and print its URL.
#   bash scripts/start.sh [--demo] [--port 7878] [--lan] [--public]
#   --demo    fill the lab with sample agents and scripted robots
#   --lan     listen on all interfaces so other machines on your network can open it
#   --public  also open a temporary public https URL (needs `cloudflared`)
# With --lan or --public a write token is generated (unless LAB_TOKEN is set),
# so only you and your agents can post events. Viewing stays open.
set -euo pipefail
cd "$(dirname "$0")/.."
say() { printf '\033[1;36m[agent-arena]\033[0m %s\n' "$*" >&2; }

PORT=7878; DEMO=""; LAN=0; PUBLIC=0
while [ $# -gt 0 ]; do
  case "$1" in
    --demo) DEMO="--demo" ;;
    --port) PORT="$2"; shift ;;
    --lan) LAN=1 ;;
    --public) PUBLIC=1 ;;
    *) say "unknown option $1"; exit 1 ;;
  esac
  shift
done

[ -x bin/lab-server ] || { say "Not installed yet. Running scripts/install.sh first..."; bash scripts/install.sh >&2; }
mkdir -p .lab

health() { curl -fsS "http://127.0.0.1:$PORT/api/health" >/dev/null 2>&1; }

if health; then
  say "Already running on port $PORT."
else
  HOST=127.0.0.1
  if [ $LAN = 1 ] || [ $PUBLIC = 1 ]; then
    [ $LAN = 1 ] && HOST=0.0.0.0
    if [ -z "${LAB_TOKEN:-}" ]; then
      LAB_TOKEN="$(head -c 16 /dev/urandom | od -An -tx1 | tr -d ' \n')"
    fi
    export LAB_TOKEN
    echo "$LAB_TOKEN" > .lab/token
  fi
  nohup ./bin/lab-server --config lab.toml --web web --bind "$HOST:$PORT" $DEMO > .lab/server.log 2>&1 &
  echo $! > .lab/server.pid
  for _ in $(seq 1 50); do health && break; sleep 0.2; done
  health || { say "Server did not start. Log:"; cat .lab/server.log >&2; exit 1; }
  say "Started (pid $(cat .lab/server.pid)). Log: .lab/server.log"
fi

echo "AGENT_ARENA_URL=http://127.0.0.1:$PORT"
if [ $LAN = 1 ]; then
  IP="$( (hostname -I 2>/dev/null || ipconfig getifaddr en0 2>/dev/null || true) | awk '{print $1}')"
  [ -n "$IP" ] && echo "AGENT_ARENA_LAN_URL=http://$IP:$PORT"
fi
[ -f .lab/token ] && echo "AGENT_ARENA_TOKEN=$(cat .lab/token)"

if [ $PUBLIC = 1 ]; then
  if ! command -v cloudflared >/dev/null 2>&1; then
    say "--public needs cloudflared: https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/"
    say "(or use any tunnel you like, e.g. 'ngrok http $PORT', 'tailscale funnel $PORT')"
    exit 0
  fi
  nohup cloudflared tunnel --no-autoupdate --url "http://127.0.0.1:$PORT" > .lab/tunnel.log 2>&1 &
  echo $! > .lab/tunnel.pid
  for _ in $(seq 1 75); do
    URL="$(grep -o 'https://[a-z0-9-]*\.trycloudflare\.com' .lab/tunnel.log | head -1 || true)"
    [ -n "$URL" ] && break; sleep 0.2
  done
  if [ -n "${URL:-}" ]; then echo "AGENT_ARENA_PUBLIC_URL=$URL"; else say "Tunnel did not report a URL yet; see .lab/tunnel.log"; fi
fi
