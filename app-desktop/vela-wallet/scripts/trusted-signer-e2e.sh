#!/usr/bin/env bash
#
# The Trusted Signer, end to end, against the real page in a real browser
# (specs 071, 075, 076 and 102). A local check — never CI: it needs Chrome.
#
# What runs: the desktop's own attempt (`executor::trusted_signer`) builds the
# request with the core and hands over the launch URL — the request in the
# fragment, the answer address `velawallet://sign-result`; this checkout's
# `app-web/trusted-signer/dist/` is served at http://localhost:<port>/ and
# checked first, as the app checks a page (spec 102 R6), and headless Chrome
# opens the version that check admitted (`/b/<sha256>/sign.html`, with the
# app's `lang=`); a CDP virtual authenticator plays the vault,
# the page's slide is confirmed through its automation hook, and the page's
# navigation to `velawallet://sign-result?…` is caught through DevTools and
# delivered as the OS would (`deliver_callback`). The wallet then verifies
# what came back against the digest or the challenge form it expected. The
# page answers that address and no other (spec 102 R7), so nothing here
# needs a test exception in the page.
#
# Five cases:
#   · the wallet's own send (SafeOp)
#   · a dApp's personal_sign (SafeMessage → EIP-1271)
#   · a tab closed unsigned (nothing signed, nothing reaches the wallet)
#   · an operation that is not the request (the page refuses before any
#     passkey prompt)
#   · a create, then the sign-in that finds the key — two visits, one flow
# See `src/executor/trusted_signer_e2e.rs`.
#
# Needs Chrome for Testing — stable Chrome works too; this is the build the
# page's own suites use (app-web/trusted-signer/HANDOVER.md). EVERY port is
# picked by the OS — the page's server and Chrome's DevTools — so this can run
# beside other agents and other browsers.
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
