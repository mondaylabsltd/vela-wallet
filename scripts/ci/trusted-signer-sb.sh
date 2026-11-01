#!/usr/bin/env bash
#
# The three files the trusted-signer page suites read from $SB
# (app-web/trusted-signer/HANDOVER.md, "$SB 里需要三个文件"), made at job time:
#
#   cert.pem / key.pem   a throwaway self-signed certificate for getvela.app
#                        (the suites map getvela.app:443 to a local TLS server
#                        with --host-resolver-rules, and start Chrome with
#                        --ignore-certificate-errors);
#   tls-serve.py         the static TLS server test-kit.mjs spawns.
#
#   scripts/ci/trusted-signer-sb.sh <dir>     (prints the directory)
#
# Nothing here is a secret: the key is generated per run, lives for the run,
# and signs nothing but a loopback page. It is never committed.
set -euo pipefail

SB="${1:?usage: trusted-signer-sb.sh <dir>}"
mkdir -p "$SB"

if [[ ! -s "$SB/cert.pem" || ! -s "$SB/key.pem" ]]; then
  openssl req -x509 -newkey rsa:2048 -keyout "$SB/key.pem" -out "$SB/cert.pem" \
    -days 30 -nodes -subj "/CN=getvela.app" -addext "subjectAltName=DNS:getvela.app" \
    >/dev/null 2>&1
fi

cat >"$SB/tls-serve.py" <<'EOF'
import http.server, functools, ssl, sys
directory, port, cert, key = sys.argv[1], int(sys.argv[2]), sys.argv[3], sys.argv[4]
handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=directory)
httpd = http.server.ThreadingHTTPServer(('127.0.0.1', port), handler)
ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
ctx.load_cert_chain(cert, key)
httpd.socket = ctx.wrap_socket(httpd.socket, server_side=True)
httpd.serve_forever()
EOF

echo "$SB"
