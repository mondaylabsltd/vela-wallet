/**
 * VelaGasSettlementSplitter CREATE2 derivation — cross-repo determinism.
 *
 * The golden vectors below MUST equal the bundler's
 * (vela-relay/tests/splitter_test.ts) and the Foundry test's
 * (vela-relay/evm_contracts/test/VelaGasSettlementSplitterCreate2.t.sol). If the wallet and
 * bundler disagree, the wallet deploys a splitter at an address the bundler never pays.
 */
import {
  VELA_SPLITTER_CREATION_CODE,
  VELA_SPLITTER_FACTORY,
  VELA_SPLITTER_SALT,
  computeSplitterAddress,
  encodeSplitterDeployCall,
} from '@/services/safe-address';
import { abiEncodeAddress, create2Address, keccak256 } from '@/services/eth-crypto';
import { concatBytes, fromHex, toHex } from '@/services/hex';

const TREASURY_A = '0x1111111111111111111111111111111111111111';
const SPLITTER_A = '0x3979be163bFb74Dce66F8E0839577807C2197226';
const TREASURY_B = '0x000000000000000000000000000000000000dEaD';
const SPLITTER_B = '0xdC95900610B854aB0c9B57A74B0f5bB67dDDB3B4';

const CREATION_CODE_HASH = 'eac7eb6ec1d5aa3a4d67982d8d969332cddd8bf0b91e02ad102742ff8e37ec4f';

describe('VelaGasSettlementSplitter derivation', () => {
  test('constants are byte-identical to the bundler', () => {
    expect(VELA_SPLITTER_FACTORY).toBe('0x4e59b44847b379578588920cA78FbF26c0B4956C');
    expect(VELA_SPLITTER_SALT).toBe('650cb20978a0e7efdcf6f077240c609a59f2f02401ed16fb4a222a2b51cb9720');
    expect(toHex(keccak256(fromHex(VELA_SPLITTER_CREATION_CODE)))).toBe(CREATION_CODE_HASH);
  });

  test('computeSplitterAddress matches the golden cross-repo vectors', () => {
    expect(computeSplitterAddress(TREASURY_A)).toBe(SPLITTER_A);
    expect(computeSplitterAddress(TREASURY_B)).toBe(SPLITTER_B);
  });

  test('address is treasury-dependent and case-insensitive on input', () => {
    expect(computeSplitterAddress(TREASURY_A)).not.toBe(computeSplitterAddress(TREASURY_B));
    expect(computeSplitterAddress(TREASURY_A.toLowerCase())).toBe(computeSplitterAddress(TREASURY_A));
  });

  test('encodeSplitterDeployCall = salt(32) ++ creationCode ++ abi.encode(treasury)', () => {
    const data = encodeSplitterDeployCall(TREASURY_A);
    const creationCodeBytes = fromHex(VELA_SPLITTER_CREATION_CODE);
    expect(data.length).toBe(32 + creationCodeBytes.length + 32);

    const expected = concatBytes(
      fromHex(VELA_SPLITTER_SALT),
      creationCodeBytes,
      abiEncodeAddress(TREASURY_A),
    );
    expect(toHex(data)).toBe(toHex(expected));
  });
});

describe('create2Address primitive', () => {
  test('matches the splitter golden vector via the raw formula', () => {
    const initCode = concatBytes(fromHex(VELA_SPLITTER_CREATION_CODE), abiEncodeAddress(TREASURY_A));
    const addr = create2Address(VELA_SPLITTER_FACTORY, fromHex(VELA_SPLITTER_SALT), keccak256(initCode));
    expect(addr).toBe(SPLITTER_A);
  });
});
