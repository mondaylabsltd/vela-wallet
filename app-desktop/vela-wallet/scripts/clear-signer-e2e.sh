#!/usr/bin/env bash
#
# The Clear Signer, end to end, against the real page in a real browser
# (spec 071, T031). A local check — never CI: it needs Chrome.
#
# What runs: the desktop's own ceremony (`executor::clear_signer`) builds the
# request with the core, listens on 127.0.0.1:0 and hands over the page URL;
# headless Chrome opens that URL on `app-web/clearsigning` served from this
# checkout at http://localhost:<port>/, a CDP virtual authenticator holds the
# wallet's key under rpId `localhost`, the page's slide is confirmed through
# its automation hook, and the wallet verifies what comes back on the
# loopback against the digest it computed. Four cases: the wallet's own send
# (SafeOp), a dApp's personal_sign (SafeMessage → EIP-1271), a tab closed
# unsigned (declined), and an operation that is not the request (the page
# refuses). See `src/executor/clear_signer_e2e.rs`.
#
# Needs Chrome for Testing — stable Chrome works too; this is the build the
# page's own suites use (app-web/clearsigning/HANDOVER.md). Every port is
# picked by the OS.
#
# Usage: scripts/clear-signer-e2e.sh            (CHROME_BIN overrides the path)
set -euo pipefail
cd "$(dirname "$0")/.."

: "${CHROME_BIN:=$HOME/Library/Caches/ms-playwright/chromium-1234/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing}"
if [ ! -x "$CHROME_BIN" ]; then
  echo "no Chrome at CHROME_BIN=$CHROME_BIN" >&2
  exit 1
fi
export CHROME_BIN

# Chrome's profiles go here, one per case, and go away with it.
CLEAR_SIGNER_E2E_DIR=$(mktemp -d)
export CLEAR_SIGNER_E2E_DIR
trap 'rm -rf "$CLEAR_SIGNER_E2E_DIR"' EXIT

cargo test clear_signer_e2e -- --ignored --test-threads=1 "$@"
