#!/usr/bin/env bash
#
# The Trusted Signer, end to end, against the real page in a real browser
# (specs 071 and 075). A local check — never CI: it needs Chrome.
#
# What runs: the desktop's own attempt (`executor::trusted_signer`) builds the
# request with the core, listens on 127.0.0.1:0 and hands over the launch URL;
# headless Chrome opens it on `app-web/trusted-signer` served from this checkout
# at http://localhost:<port>/; the page connects BACK over the loopback
# WebSocket (spec 075 — the desktop's old URL fragment and HTTP callback are
# gone) and stays connected, a CDP virtual authenticator plays the vault, the
# page's slide is confirmed through its automation hook, and the wallet
# verifies what comes back against the digest or the challenge form it
# expected.
#
# Five cases:
#   · the wallet's own send (SafeOp)
#   · a dApp's personal_sign (SafeMessage → EIP-1271)
#   · a tab closed unsigned (declined)
#   · an operation that is not the request (the page refuses, at once)
#   · a create AND the sign-in after it, on ONE page visit — the case the URL
#     fragment could not carry, and the reason the desktop moved
# See `src/executor/trusted_signer_e2e.rs`.
#
# Needs Chrome for Testing — stable Chrome works too; this is the build the
# page's own suites use (app-web/trusted-signer/HANDOVER.md). EVERY port is
# picked by the OS — the page's server, Chrome's DevTools and the wallet's own
# listener — so this can run beside other agents and other browsers.
#
# Usage: scripts/trusted-signer-e2e.sh            (CHROME_BIN overrides the path)
set -euo pipefail
cd "$(dirname "$0")/.."

: "${CHROME_BIN:=$HOME/Library/Caches/ms-playwright/chromium-1234/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing}"
if [ ! -x "$CHROME_BIN" ]; then
  echo "no Chrome at CHROME_BIN=$CHROME_BIN" >&2
  exit 1
fi
export CHROME_BIN

# Chrome's profiles go here, one per case, and go away with it.
TRUSTED_SIGNER_E2E_DIR=$(mktemp -d)
export TRUSTED_SIGNER_E2E_DIR
trap 'rm -rf "$TRUSTED_SIGNER_E2E_DIR"' EXIT

cargo test trusted_signer_e2e -- --ignored --test-threads=1 "$@"
