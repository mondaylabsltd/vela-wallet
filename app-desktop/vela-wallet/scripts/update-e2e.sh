#!/usr/bin/env bash
#
# The updater's local proof (README "Updates"), on this Mac, with nothing of
# the owner's: a throwaway P-256 release key made here, a feed on loopback,
# and two ad-hoc signed builds of the test flavor.
#
#   scripts/update-e2e.sh <work dir> [--no-build]
#
#   v(N)   = this crate's version, built with --features update-test,dev-fixtures
#            and the local key and feed compiled in;
#   v(N+1) = the same source reporting the next patch version
#            (VELA_UPDATE_TEST_VERSION), its bundle's Info.plist to match,
#            zipped as the release artifact.
#
# Scenarios, each a launch of v(N) from <work>/Applications against one
# throwaway state directory, in this order (issue times in brackets):
#
#   1. tampered   a signed manifest whose payload was edited     -> BadSignature
#   2. downgrade  a signed manifest offering an older version    -> NotNewer
#   3. bad hash   a signed manifest whose sha256 is not the zip's -> the prompt,
#                 Install, the download refused (ArtifactMismatch), v(N) stays
#   4. update     a signed manifest for v(N+1)   [t+3] -> prompt -> Install ->
#                 progress -> swap -> relaunch; the relaunched app logs
#                 "running v(N+1)" and offers a later v(N+2) as "N+1 → N+2"
#      mislabel   …whose signed artifact is v(N+1)'s own genuine bundle:
#                 downloaded, verified, then refused (VersionMismatch)
#   5. replay     v(N) put back, the [t+2] manifest served again  -> Replayed
#
# The private key stays in <work>/keys (mode 600) and is never printed. The
# app's windows open without taking the keyboard (VELA_NO_ACTIVATE); window
# pictures land in <work>/shots when `screencapture` may record the screen.
set -euo pipefail

APP_DIR="$(cd "$(dirname "$0")/.." && pwd)"
WORK="${1:?usage: $0 <work dir> [--no-build]}"
BUILD=1
[[ "${2:-}" == "--no-build" ]] && BUILD=0
mkdir -p "$WORK"
WORK="$(cd "$WORK" && pwd)"
PORT="${PORT:-8765}"
FEED="http://127.0.0.1:$PORT/stable.json"
TRIPLE=aarch64-apple-darwin
[[ "$(uname -m)" == arm64 ]] || TRIPLE=x86_64-apple-darwin
ARCH="$(uname -m)"
PLATFORM=macos-aarch64
[[ "$ARCH" == arm64 ]] || PLATFORM=macos-x86_64

VERSION="$(sed -n '/^\[package\]/,/^\[/ s/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$APP_DIR/Cargo.toml" | head -1)"
IFS=. read -r MAJ MIN PAT <<<"$VERSION"
NEXT="$MAJ.$MIN.$((PAT + 1))"
AFTER="$MAJ.$MIN.$((PAT + 2))"
OLDER="$MAJ.$MIN.$((PAT > 0 ? PAT - 1 : 0))"

KEYS="$WORK/keys" WWW="$WORK/www" APPS="$WORK/Applications" LOGS="$WORK/logs" SHOTS="$WORK/shots"
STATE="$WORK/state"
mkdir -p "$KEYS" "$WWW" "$APPS" "$LOGS" "$SHOTS"
chmod 700 "$KEYS"
note() { echo "==> $*"; }

# --- the key -----------------------------------------------------------------
if [[ ! -f "$KEYS/release.pem" ]]; then
  note "a throwaway P-256 release key (private half stays in $KEYS)"
  ( umask 077 && openssl ecparam -name prime256v1 -genkey -noout -out "$KEYS/release.pem" )
fi
PUB="$(openssl ec -in "$KEYS/release.pem" -pubout -conv_form uncompressed -outform DER 2>/dev/null |
  tail -c 65 | xxd -p -c 65)"
[[ ${#PUB} -eq 130 ]] || { echo "could not read the public key" >&2; exit 1; }

# --- the two builds ----------------------------------------------------------
BUNDLED="$APP_DIR/target/$TRIPLE/release/vela-wallet"
bundle() { # bundle <binary> <version> <out .app>
  mkdir -p "$(dirname "$BUNDLED")"
  cp "$1" "$BUNDLED"
  "$APP_DIR/scripts/build-macos-app.sh" --arch "$ARCH" --skip-build --no-dmg >/dev/null
  rm -f "$BUNDLED"
  local built="$APP_DIR/dist/macos/$ARCH/Vela Wallet.app"
  if [[ "$2" != "$VERSION" ]]; then
    /usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $2" -c "Set :CFBundleVersion $2" \
      "$built/Contents/Info.plist"
    codesign --force --deep --sign - "$built" 2>/dev/null
  fi
  rm -rf "$3"
  ditto "$built" "$3"
}
if (( BUILD )); then
  for flavor in "$VERSION" "$NEXT"; do
    note "build v$flavor (update-test, dev-fixtures)"
    extra=()
    [[ "$flavor" != "$VERSION" ]] && extra=(VELA_UPDATE_TEST_VERSION="$flavor")
    ( cd "$APP_DIR" && env VELA_UPDATE_TEST_KEY="$PUB" VELA_UPDATE_TEST_FEED="$FEED" ${extra[@]+"${extra[@]}"} \
      cargo build --release --features update-test,dev-fixtures 2>&1 | tail -1 )
    cp "$APP_DIR/target/release/vela-wallet" "$WORK/vela-wallet-$flavor"
  done
fi
bundle "$WORK/vela-wallet-$VERSION" "$VERSION" "$WORK/N.app"
bundle "$WORK/vela-wallet-$NEXT" "$NEXT" "$WORK/N1.app"
ZIP="$WWW/VelaWallet-$NEXT-macos-$ARCH.zip"
rm -f "$ZIP"
# The archive's top folder is the app's own name, as build-macos-app.sh --zip
# makes it.
rm -rf "$WORK/zip-stage" && mkdir -p "$WORK/zip-stage"
ditto "$WORK/N1.app" "$WORK/zip-stage/Vela Wallet.app"
ditto -c -k --keepParent "$WORK/zip-stage/Vela Wallet.app" "$ZIP"
rm -rf "$WORK/zip-stage"
SIZE="$(stat -f %z "$ZIP")"
SHA="$(shasum -a 256 "$ZIP" | cut -d' ' -f1)"
note "artifact v$NEXT: $SIZE bytes, sha256 ${SHA:0:12}…"

# --- the feed ----------------------------------------------------------------
# manifest <version> <issued_at> <sha256> [tamper]
manifest() {
  local payload sig
  payload="$(python3 -I -c 'import json,sys; v,t,url,sha,size,plat=sys.argv[1:]; print(json.dumps({"version":v,"issued_at":int(t),"notes_url":None,"platforms":{plat:{"url":url,"sha256":sha,"size":int(size)}}},separators=(",",":")))' \
    "$1" "$2" "http://127.0.0.1:$PORT/$(basename "$ZIP")" "$3" "$SIZE" "$PLATFORM")"
  sig="$(printf '%s' "$payload" | openssl dgst -sha256 -sign "$KEYS/release.pem" | xxd -p | tr -d '\n')"
  if [[ "${4:-}" == tamper ]]; then
    payload="${payload/127.0.0.1:$PORT/127.0.0.1:$((PORT + 1))}"
  fi
  python3 -I -c 'import json,sys; print(json.dumps({"payload":sys.argv[1],"signature":sys.argv[2]}))' \
    "$payload" "$sig" > "$WWW/stable.json"
}

cat > "$WORK/serve.py" <<'PY'
# A loopback file server that sends slowly enough for the progress to show.
import http.server, os, sys, time
port, root, seconds = int(sys.argv[1]), sys.argv[2], float(sys.argv[3])
class Slow(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *a, **k):
        super().__init__(*a, directory=root, **k)
    def log_message(self, *a):
        pass
    def copyfile(self, source, out):
        size = os.fstat(source.fileno()).st_size
        chunk = max(64 * 1024, int(size / (seconds * 20))) if size > 1_000_000 else size or 1
        while True:
            data = source.read(chunk)
            if not data:
                break
            out.write(data)
            if size > 1_000_000:
                time.sleep(1 / 20)
http.server.ThreadingHTTPServer(("127.0.0.1", port), Slow).serve_forever()
PY
python3 -I "$WORK/serve.py" "$PORT" "$WWW" "${DOWNLOAD_SECONDS:-6}" &
SERVER=$!
WINID="${WINID:-}"
cleanup() {
  kill "$SERVER" 2>/dev/null || true
  pkill -f "$APPS/Vela Wallet.app/Contents/MacOS/vela-wallet" 2>/dev/null || true
}
trap cleanup EXIT
sleep 0.5

shot() { # shot <pid> <name>
  [[ -n "$WINID" && -x "$WINID" ]] || return 0
  local wid
  wid="$("$WINID" "$1" 2>/dev/null || true)"
  if [[ -n "$wid" && "$wid" != 0 ]]; then
    screencapture -o -x -l "$wid" "$SHOTS/$2.png" 2>/dev/null || true
  fi
}

install_n() { rm -rf "$APPS/Vela Wallet.app"; ditto "$WORK/N.app" "$APPS/Vela Wallet.app"; }
EXE="$APPS/Vela Wallet.app/Contents/MacOS/vela-wallet"

# launch <log> <seconds> <until-pattern>: v(N) until the pattern or the time.
launch() {
  local log="$LOGS/$1" limit="$2" until="$3" waited=0
  env VELA_STATE_DIR="$STATE" VELA_PARALLEL_SPACE=1 VELA_PARALLEL_SEED=1 VELA_NO_ACTIVATE=1 VELA_SKIP_LAUNCH_ANIMATION=1 \
    VELA_UPDATE_CHECK_AFTER_MS=1500 VELA_UPDATE_AUTO_INSTALL=1 \
    VELA_UPDATE_RELAUNCH_LOG="$LOGS/relaunched.log" \
    "$EXE" >"$log" 2>&1 &
  LAUNCHED=$!
  while (( waited < limit * 10 )); do
    grep -q -- "$until" "$log" 2>/dev/null && break
    kill -0 "$LAUNCHED" 2>/dev/null || break
    sleep 0.1
    waited=$((waited + 1))
  done
}
stop() { kill "$1" 2>/dev/null || true; sleep 0.3; kill -9 "$1" 2>/dev/null || true; wait "$1" 2>/dev/null || true; }
installed_version() { /usr/libexec/PlistBuddy -c "Print :CFBundleShortVersionString" "$APPS/Vela Wallet.app/Contents/Info.plist"; }
verdict() { printf '  %-10s %s\n' "$1" "$2" | tee -a "$WORK/RESULTS.txt"; }

rm -rf "$STATE" "$LOGS"/*.log
: > "$WORK/RESULTS.txt"
T=$(date +%s)
install_n

note "1/5 tampered manifest"
manifest "$NEXT" "$T" "$SHA" tamper
launch 1-tampered.log 15 "manifest not taken"
stop "$LAUNCHED"
verdict tampered "$(grep -o 'manifest not taken: .*' "$LOGS/1-tampered.log" || echo MISSING) · installed v$(installed_version)"

note "2/5 downgrade"
manifest "$OLDER" "$((T + 1))" "$SHA"
launch 2-downgrade.log 15 "manifest not taken"
stop "$LAUNCHED"
verdict downgrade "$(grep -o 'manifest not taken: .*' "$LOGS/2-downgrade.log" || echo MISSING) · installed v$(installed_version)"

note "3/5 wrong hash"
manifest "$NEXT" "$((T + 2))" "$(printf '%064d' 0)"
launch 3-badhash.log 40 "download refused"
sleep 0.5
shot "$LAUNCHED" "e2e-3-refused"
stop "$LAUNCHED"
verdict "bad hash" "$(grep -o 'download refused: .*' "$LOGS/3-badhash.log" || echo MISSING) · installed v$(installed_version)"

note "4/5 the update"
manifest "$NEXT" "$((T + 3))" "$SHA"
rm -f "$LOGS/relaunched.log"
launch 4-update.log 15 "offered $NEXT"
sleep 1.2
shot "$LAUNCHED" "e2e-4a-offered"
PID_N=$LAUNCHED
# Install is pressed 3 s after the offer; the download takes ~6 s.
sleep 3.5
shot "$PID_N" "e2e-4b-downloading"
sleep 2
shot "$PID_N" "e2e-4c-downloading"
for _ in $(seq 1 600); do kill -0 "$PID_N" 2>/dev/null || break; sleep 0.1; done
wait "$PID_N" 2>/dev/null || true
# The relaunched v(N+1) asks the same feed: offer it v(N+2) so its prompt
# says which version is running. The artifact signed for it is v(N+1)'s own
# zip — a genuine bundle under a newer manifest — so its install is refused
# (VersionMismatch) and v(N+1) stays.
manifest "$AFTER" "$((T + 4))" "$SHA"
for _ in $(seq 1 300); do grep -q "offered $AFTER" "$LOGS/relaunched.log" 2>/dev/null && break; sleep 0.1; done
RELAUNCHED="$(pgrep -f "$EXE" | head -1 || true)"
sleep 1.2
[[ -n "$RELAUNCHED" ]] && shot "$RELAUNCHED" "e2e-4d-relaunched-offers-next"
for _ in $(seq 1 300); do grep -q "install refused\|download refused" "$LOGS/relaunched.log" 2>/dev/null && break; sleep 0.1; done
sleep 0.5
[[ -n "$RELAUNCHED" ]] && shot "$RELAUNCHED" "e2e-4e-relaunched-refuses-old-bundle"
verdict update "$(grep -o 'installed .*' "$LOGS/4-update.log" || echo MISSING) · installed v$(installed_version) · relaunched: $(grep -o 'update: running .*' "$LOGS/relaunched.log" 2>/dev/null || echo MISSING) · it offers: $(grep -o "offered $AFTER (running [^)]*)" "$LOGS/relaunched.log" 2>/dev/null || echo MISSING)"
verdict mislabel "$(grep -o 'install refused: .*' "$LOGS/relaunched.log" 2>/dev/null || echo MISSING) · installed v$(installed_version)"
[[ -n "$RELAUNCHED" ]] && stop "$RELAUNCHED"
pkill -f "$EXE" 2>/dev/null || true

note "5/5 replayed older manifest"
install_n
manifest "$NEXT" "$((T + 2))" "$SHA"
launch 5-replay.log 15 "manifest not taken"
stop "$LAUNCHED"
verdict replay "$(grep -o 'manifest not taken: .*' "$LOGS/5-replay.log" || echo MISSING) · installed v$(installed_version)"

echo
cat "$WORK/RESULTS.txt"
