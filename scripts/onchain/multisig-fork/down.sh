#!/bin/sh
# Stop what up.sh started.
here=$(cd "$(dirname "$0")" && pwd)
STATE=${STATE:-"$here/.state"}
for p in relay genesis anvil anvil-unread peerconnect key-index; do
  [ -f "$STATE/$p.pid" ] && kill "$(cat "$STATE/$p.pid")" 2>/dev/null; rm -f "$STATE/$p.pid"
done
