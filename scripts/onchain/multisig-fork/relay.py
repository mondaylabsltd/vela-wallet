#!/usr/bin/env python3
"""A minimal local relay for the multisig fork harness.

It speaks what the web wallet asks the Vela relay (ERC-4337 v0.7 JSON-RPC at
`/{chainId}`, plus the three REST endpoints) and bundles every operation it is
sent straight into the fork with `EntryPoint.handleOps`, from an anvil account.
Nothing is simulated: the wallet's real user operations — passkey signatures,
in-band fee legs, deployments, a multisig's `execTransaction` — execute on the
forked chain's real contracts.

    RPC_URL      the fork (default http://127.0.0.1:8545)
    RPCS         one fork per chain, as JSON {"100": url, …} (default: chain
                 100 at RPC_URL); a request at `/{chainId}` goes to its own
    RELAY_PORT   where to listen (default 8790)
    BUNDLER_KEY  the key that pays for handleOps (default: a fixed fresh key)
    FEE_RECIPIENT where in-band fees go (default: a fixed fresh address)
    MOVING_GAS   §39 (R22 follow-up): a gas price that moves on EVERY quote —
                 a comma list of gwei figures, cycled one per
                 `pimlico_getUserOperationGasPrice` (e.g. "2,1.8,2.1,1.9,2.6"):
                 the fee the wallet prices moves up and down between its look
                 and its press, as a real network's does. A test sets or
                 reads it at run time: `POST /v1/test/gas {"gwei": [...]}`
                 (the cycle from its start), `GET /v1/test/gas` (the cycle
                 and every price quoted, in order).

Requires `cast` (Foundry) on PATH. Standard library only otherwise.
"""
import json
import os
import subprocess
import sys
import threading
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

RPC_URL = os.environ.get("RPC_URL", "http://127.0.0.1:8545")
RPCS: dict[str, str] = json.loads(os.environ.get("RPCS", "{}")) or {"100": RPC_URL}
PORT = int(os.environ.get("RELAY_PORT", "8790"))
# Fresh keys, not anvil's defaults: on Gnosis those carry EIP-7702 delegations
# (sweepers), and a payment to one reverts the whole bundle (AA91).
# keccak256("vela multisig fork bundler") / ("… fee recipient").
BUNDLER_KEY = os.environ.get("BUNDLER_KEY", "0x9a53f6d77cc2523fa22311321b475feaad0928fb773b746ecc1522db9d8e65fd")
FEE_RECIPIENT = os.environ.get("FEE_RECIPIENT", "0x5c3D966577b66a12B815119f1af452Ed7adf260c")
ENTRY_POINT = "0x0000000071727De22E5E9d8BAf0edAc6f37da032"
USER_OP_EVENT = "0x49628fd1471006c1482da88028e9ce4dbb080b815c9b0344d39e5a8e6ec1419f"
PACKED = "(address,uint256,bytes,bytes,bytes32,uint256,bytes32,bytes,bytes)"

lock = threading.Lock()
ops: dict[str, str] = {}  # userOpHash -> transaction hash
here = threading.local()  # .url: the fork this request's chain runs on


def url() -> str:
    return getattr(here, "url", RPC_URL)


def rpc(method, params):
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
    req = urllib.request.Request(url(), body, {"content-type": "application/json"})
    with urllib.request.urlopen(req, timeout=60) as r:
        answer = json.loads(r.read())
    if "error" in answer:
        raise RuntimeError(answer["error"].get("message", str(answer["error"])))
    return answer["result"]


def cast(*args):
    out = subprocess.run(["cast", *args], capture_output=True, text=True, timeout=120)
    if out.returncode != 0:
        raise RuntimeError(out.stderr.strip() or out.stdout.strip())
    return out.stdout.strip()


def word(n: str) -> str:
    return format(int(n, 16) if isinstance(n, str) else n, "032x")


def packed(op: dict) -> str:
    init = "0x"
    if op.get("factory"):
        init = op["factory"] + op.get("factoryData", "0x")[2:]
    gas_limits = "0x" + word(op["verificationGasLimit"]) + word(op["callGasLimit"])
    fees = "0x" + word(op["maxPriorityFeePerGas"]) + word(op["maxFeePerGas"])
    pm = "0x"
    if op.get("paymaster"):
        pm = (
            op["paymaster"]
            + word(op.get("paymasterVerificationGasLimit", "0x0"))
            + word(op.get("paymasterPostOpGasLimit", "0x0"))
            + op.get("paymasterData", "0x")[2:]
        )
    return (
        f"({op['sender']},{int(op['nonce'], 16)},{init},{op['callData']},{gas_limits},"
        f"{int(op['preVerificationGas'], 16)},{fees},{pm},{op['signature']})"
    )


def user_op_hash(op: dict) -> str:
    return cast(
        "call", ENTRY_POINT, f"getUserOpHash({PACKED})(bytes32)", packed(op), "--rpc-url", url()
    )


def send(op: dict) -> str:
    h = user_op_hash(op)
    with lock:
        if h in ops:
            return h
        data = cast("calldata", f"handleOps({PACKED}[],address)", f"[{packed(op)}]", FEE_RECIPIENT)
        # As a bundler does: answer with the hash at once, mine in the background.
        # (Waiting for the receipt here made a slow fork's answer late enough
        # for the wallet to give up and send the operation again.)
        tx = cast(
            "send", ENTRY_POINT, data, "--private-key", BUNDLER_KEY, "--rpc-url", url(),
            "--gas-limit", "15000000", "--async",
        )
        ops[h] = tx
        print(f"[relay] {h[:12]}… → {tx}", flush=True)
    return h


def op_receipt(h: str):
    tx = ops.get(h)
    if tx is None:
        return None
    r = rpc("eth_getTransactionReceipt", [tx])
    if r is None:
        return None
    success = False
    for log in r["logs"]:
        topics = log["topics"]
        if len(topics) > 1 and topics[0] == USER_OP_EVENT and topics[1].lower() == h.lower():
            success = int(log["data"][2 + 64 : 2 + 128], 16) == 1
    return {
        "userOpHash": h,
        "success": success,
        "actualGasCost": "0x0",
        "actualGasUsed": r["gasUsed"],
        "receipt": {
            "transactionHash": tx,
            "status": r["status"],
            "blockNumber": r["blockNumber"],
            "blockHash": r["blockHash"],
            "gasUsed": r["gasUsed"],
            "logs": r["logs"],
        },
    }


GAS = {"slow": "0x3b9aca00", "standard": "0x3b9aca00", "fast": "0x3b9aca00"}

# §39: the moving gas price (MOVING_GAS), and every price quoted.
moving: list[float] = [float(g) for g in os.environ.get("MOVING_GAS", "").split(",") if g.strip()]
moving_at = 0
quoted: list[str] = []


def gas_now() -> dict:
    """This quote's price: the next of the moving cycle, or the fixed one."""
    global moving_at
    with lock:
        if not moving:
            price = GAS
        else:
            wei = hex(int(moving[moving_at % len(moving)] * 1_000_000_000))
            moving_at += 1
            price = {k: wei for k in GAS}
        quoted.append(price["standard"])
    return price


def answer(method: str, params: list):
    if method == "eth_estimateUserOperationGas":
        op = params[0]
        deployed = not op.get("factory")
        return {
            "preVerificationGas": "0x186a0",
            "verificationGasLimit": "0x7a120" if deployed else "0xf4240",
            "callGasLimit": "0x4c4b40",
        }
    if method == "pimlico_getUserOperationGasPrice":
        return {k: {"maxFeePerGas": v, "maxPriorityFeePerGas": v} for k, v in gas_now().items()}
    if method == "vela_getInBandGasQuote":
        return [
            {
                "recipient": FEE_RECIPIENT,
                "asset": "native",
                "feeToken": None,
                "balance": "0x56bc75e2d63100000",
                "decimals": 18,
                "symbol": "xDAI",
                "usdBalance": "100",
                "usdPrice": "1",
            }
        ]
    if method == "eth_sendUserOperation":
        return send(params[0])
    if method == "eth_getUserOperationReceipt":
        return op_receipt(params[0])
    if method == "pimlico_getUserOperationStatus":
        h = params[0]
        return {"status": "included" if op_receipt(h) else ("submitted" if h in ops else "not_found")}
    if method == "eth_supportedEntryPoints":
        return [ENTRY_POINT]
    if method == "eth_chainId":
        return rpc("eth_chainId", [])
    # Anything else is the chain's own.
    return rpc(method, params)


class Handler(BaseHTTPRequestHandler):
    def _send(self, status, payload):
        body = json.dumps(payload).encode()
        self.send_response(status)
        self.send_header("content-type", "application/json")
        self.send_header("access-control-allow-origin", "*")
        self.send_header("access-control-allow-headers", "*")
        self.send_header("access-control-allow-methods", "GET, POST, OPTIONS")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_):
        pass

    def do_OPTIONS(self):
        self._send(204, {})

    def do_GET(self):
        path = self.path.split("?")[0]
        if path.startswith("/v1/account/"):
            return self._send(200, {
                "activeDepositAddress": FEE_RECIPIENT, "onchainBalance": "0x56bc75e2d63100000",
                "spendableBalance": "0x56bc75e2d63100000", "status": "ACTIVE",
            })
        if path.startswith("/v1/treasury/"):
            return self._send(200, {
                "address": FEE_RECIPIENT, "asset": "native", "balance": "0x56bc75e2d63100000",
                "floor": "0x2386f26fc10000", "bootstrapNeeded": False,
            })
        if path.startswith("/v1/sponsor"):
            return self._send(200, {"sponsored": False})
        if path in ("/", "/health"):
            return self._send(200, {"relay": "multisig-fork", "rpcs": RPCS})
        if path == "/v1/test/gas":
            with lock:
                return self._send(200, {"gwei": moving, "at": moving_at, "quoted": quoted})
        return self._send(404, {"error": "not found"})

    def do_POST(self):
        length = int(self.headers.get("content-length", "0"))
        raw = self.rfile.read(length)
        try:
            request = json.loads(raw)
        except json.JSONDecodeError:
            return self._send(400, {"error": "not json"})
        if self.path.startswith("/v1/sponsor"):
            return self._send(200, {"sponsored": False})
        if self.path.split("?")[0] == "/v1/test/gas":
            global moving, moving_at
            with lock:
                moving = [float(g) for g in request.get("gwei", [])]
                moving_at = 0
            return self._send(200, {"gwei": moving})
        chain = self.path.split("?")[0].strip("/").split("/")[0]
        if chain not in RPCS:
            return self._send(404, {"error": f"no fork for chain {chain}"})
        here.url = RPCS[chain]

        def one(req):
            try:
                return {"jsonrpc": "2.0", "id": req.get("id", 1), "result": answer(req["method"], req.get("params", []))}
            except Exception as e:  # noqa: BLE001 — every failure is the caller's answer
                print(f"[relay] {req.get('method')} failed: {e}", file=sys.stderr, flush=True)
                return {"jsonrpc": "2.0", "id": req.get("id", 1), "error": {"code": -32500, "message": str(e)}}

        if isinstance(request, list):
            return self._send(200, [one(r) for r in request])
        return self._send(200, one(request))


if __name__ == "__main__":
    print(f"[relay] listening on 127.0.0.1:{PORT}, forks {RPCS}", flush=True)
    ThreadingHTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
