// SPDX-License-Identifier: LGPL-3.0-only
pragma solidity =0.8.26;

/// @title SafeStateProbe — what a Safe transaction would change about who controls the Safe.
/// @notice Never deployed. The wallet puts this runtime code at a fresh address with an
///         `eth_call` state override and calls the Safe's own `simulateAndRevert(probe,
///         probe(...))` (StorageAccessible, Safe v1.3.0 – v1.5.0): the Safe DELEGATECALLs it,
///         so it runs in the Safe's storage. It reads everything that decides who controls the
///         Safe, runs the transaction's inner call exactly as `execTransaction` does with
///         `safeTxGas = 0` (a CALL or a DELEGATECALL, as the Safe), reads it all again, and
///         returns both readings. `simulateAndRevert` then reverts with them — nothing is ever
///         written, on any chain.
/// @dev    vela-core reads the answer (`multisig_review::probe_answer`); the layout of a reading
///         is fixed (see `_read`). Rebuild: `rust/crates/vela-core/contracts/build.sh`.
contract SafeStateProbe {
    address private constant SENTINEL = address(0x1);
    /// keccak256("guard_manager.guard.address")
    bytes32 private constant GUARD_SLOT = 0x4a204f620c8c5ccdca3fd54d003badd85ba500436a431f0cbda4f558c93c34c8;
    /// keccak256("module_manager.module_guard.address") — Safe v1.5.0
    bytes32 private constant MODULE_GUARD_SLOT = 0xb104e0b93118902c651344349b610029d694cfdec91c589c91ebafbcd0289947;
    /// keccak256("fallback_manager.handler.address")
    bytes32 private constant HANDLER_SLOT = 0x6c9a6c4a39284e37ed1cf53d337577d14212a4870fb976a4366c693b939918d5;
    /// Safe's storage: singleton 0, modules 1, owners 2, ownerCount 3, threshold 4, nonce 5.
    uint256 private constant MODULES_SLOT = 1;
    uint256 private constant OWNERS_SLOT = 2;
    /// The longest owner or module list walked; a longer one (or a loop) is marked incomplete.
    uint256 private constant MAX_LIST = 64;

    /// @param to,value,data,operation The SafeTx's inner call.
    /// @param watch Addresses whose `owners[a]` and `modules[a]` entries are read directly — a
    ///        write there grants signing or module rights without touching the linked lists.
    /// @param tokens ERC-20s whose `balanceOf(safe)` is read before and after.
    function probe(
        address to,
        uint256 value,
        bytes calldata data,
        uint8 operation,
        address[] calldata watch,
        address[] calldata tokens
    ) external returns (uint256[] memory before, uint256[] memory afterwards, bool success, bytes memory ret) {
        before = _read(watch, tokens);
        if (operation == 0) {
            (success, ret) = to.call{value: value}(data);
        } else {
            (success, ret) = to.delegatecall(data);
        }
        afterwards = _read(watch, tokens);
    }

    /// One reading, as words:
    /// [singleton, ownerCount, threshold, nonce, guard, moduleGuard, handler, balance,
    ///  ownersComplete, nOwners, owners…, modulesComplete, nModules, modules…,
    ///  for each watched address: owners[a], modules[a],
    ///  for each token: ok (1/0), balance]
    function _read(address[] calldata watch, address[] calldata tokens) private view returns (uint256[] memory w) {
        w = new uint256[](8 + 2 * (2 + MAX_LIST) + 2 * watch.length + 2 * tokens.length);
        uint256 n;
        w[n++] = _load(0);
        w[n++] = _load(3);
        w[n++] = _load(4);
        w[n++] = _load(5);
        w[n++] = _load(uint256(GUARD_SLOT));
        w[n++] = _load(uint256(MODULE_GUARD_SLOT));
        w[n++] = _load(uint256(HANDLER_SLOT));
        w[n++] = address(this).balance;
        n = _list(w, n, OWNERS_SLOT);
        n = _list(w, n, MODULES_SLOT);
        for (uint256 i = 0; i < watch.length; i++) {
            w[n++] = _load(_entry(watch[i], OWNERS_SLOT));
            w[n++] = _load(_entry(watch[i], MODULES_SLOT));
        }
        for (uint256 i = 0; i < tokens.length; i++) {
            (bool ok, bytes memory b) =
                tokens[i].staticcall(abi.encodeWithSelector(0x70a08231, address(this)));
            bool read = ok && b.length >= 32;
            w[n++] = read ? 1 : 0;
            w[n++] = read ? abi.decode(b, (uint256)) : 0;
        }
        assembly {
            mstore(w, n)
        }
    }

    /// Walk a Safe linked list from the sentinel: [complete, count, entries…].
    function _list(uint256[] memory w, uint256 n, uint256 slot) private view returns (uint256) {
        uint256 at = n;
        n += 2;
        uint256 count;
        address next = address(uint160(_load(_entry(SENTINEL, slot))));
        while (next != SENTINEL && next != address(0) && count < MAX_LIST) {
            w[n++] = uint160(next);
            count++;
            next = address(uint160(_load(_entry(next, slot))));
        }
        // Complete: the walk came back to the sentinel (or the list was never set up).
        w[at] = (next == SENTINEL || (next == address(0) && count == 0)) ? 1 : 0;
        w[at + 1] = count;
        return n;
    }

    function _entry(address key, uint256 slot) private pure returns (uint256) {
        return uint256(keccak256(abi.encode(key, slot)));
    }

    function _load(uint256 slot) private view returns (uint256 v) {
        assembly {
            v := sload(slot)
        }
    }
}
