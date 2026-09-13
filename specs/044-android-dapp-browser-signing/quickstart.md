# Quickstart — verifying 044 on the device

Prerequisites: the Xiaomi (`9d5f42fb`) with the debug build installed, the
parallel space entered (`--es vela.parallelSpace true`), and:

```sh
cd app-android/vela-wallet/dev/testdapp && python3 -m http.server 8137 &
adb -s 9d5f42fb reverse tcp:8137 tcp:8137
```

1. **Provider announces** (SC-001): 探索 → address bar → `http://127.0.0.1:8137/`
   → the page prints `announced: Vela` and `window.ethereum: present`.
2. **Connect** (SC-002): tap Connect → the consent sheet names the origin
   `127.0.0.1:8137`, the account `Parallel space 0x88cC…6894`, the
   network → 连接 → the page prints the Safe's address; a second Connect
   prints it with no sheet. Open `https://app.uniswap.org` → Connect →
   the same address in its header.
3. **Read** (SC-003): tap Block number → the page's value equals
   `RpcPool` traces (`rpc.post eth_blockNumber`) at that moment.
4. **Sign a transfer** (SC-004): tap Send dust → the signing sheet: origin,
   `0.001 XDAI → 0x7687…d141`, the quoted fee, the signer, the slide →
   slide → the page prints a tx hash; the home shows the pending row, then
   confirmed; the relay receipt matches.
5. **Guard** (SC-005): tap Approve unlimited → the sheet blocks the slide
   with the guard's words and offers an amount → enter 1 → slide → the
   page prints the hash; the calldata carried `1000000` (6 decimals).
6. **Sign-in** (SC-006): tap Sign → the message block → slide → the page
   verifies the signature through `isValidSignature` (EIP-1271) on the
   Safe and prints `valid`.
7. **Revoke** (SC-007): account pill → the connection sheet → 断开连接 →
   the page prints `disconnect`; Connect opens consent again.
8. **Memory** (SC-008): add a favourite and a group, open a second tab,
   `am force-stop`, reopen → favourites, groups, tabs, recents intact;
   switch to the wallet tab and back → the page is where it was; while on
   the wallet tab, a screenshot shows no page pixels.
