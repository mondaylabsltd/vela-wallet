#!/usr/bin/env node
/**
 * Put Safe's passkey signer factory on the networks that lack it.
 *
 *   bun run deploy:passkey-factory                 # report only, sends nothing
 *   VELA_DEPLOYER_KEY=0x… bun run deploy:passkey-factory --broadcast
 *
 * ## Why this exists
 *
 * A Vela wallet's address is derived from ALL of its passkeys, so keys two
 * through seven are signer contracts that `SafeWebAuthnSignerFactory` creates.
 * Eleven of the twenty-four built-in networks do not have that factory, which
 * means a wallet made from more than one key **cannot be deployed there at
 * all** — and the readiness check that would have said so only runs in the
 * add-network wizard, never for a built-in chain.
 *
 * The fix is one transaction per chain. It is permissionless: the factory is
 * deployed through Safe's singleton factory with salt 0, so the address is a
 * hash of the code and lands identical everywhere — `0x1d31F259…`, the
 * constant the wallet already has compiled in. Measured 2026-09-23: all
 * eleven chains already have Safe's singleton factory, so nothing is waiting
 * on Safe, and the deployment costs about 1.05M gas — fractions of a cent on
 * most of them.
 *
 * **One deployment gives both contracts.** The factory's constructor creates
 * `SafeWebAuthnSignerSingleton` itself; `SINGLETON()` on a deployed factory
 * returns `0x4E27b51350e6c2083EE19011120F50DAfEc5CA50`, which is the other
 * address the wallet checks. Verified against Base, where both are live.
 *
 * ## What it refuses to do
 *
 * It sends nothing without `--broadcast`. It reads the key from the
 * environment, never an argument, so it cannot end up in shell history. It
 * checks that the address CREATE2 will produce is the address the wallet
 * expects, and stops if it is not — a mismatch means the deployment data
 * changed, and deploying a different contract at a different address would
 * leave the wallet checking for something nobody put there. It skips a chain
 * that already has the factory, and it verifies `SINGLETON()` afterwards
 * rather than assuming.
 */
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import {
	createPublicClient,
	createWalletClient,
	formatEther,
	getCreate2Address,
	http,
	keccak256,
	toHex
} from 'viem';
import { privateKeyToAccount } from 'viem/accounts';

/** Safe's singleton factory. Present on every chain below, checked at run time. */
const SAFE_SINGLETON_FACTORY = '0x914d7Fec6aaC8cd542e72Bca78B30650d45643d7';
/** What the wallet compiles in (`rust/crates/vela-core/src/app/network_admin.rs`). */
const EXPECTED_FACTORY = '0x1d31F259eE307358a26dFb23EB365939E8641195';
const EXPECTED_SINGLETON = '0x4E27b51350e6c2083EE19011120F50DAfEc5CA50';
/** `SINGLETON()` */
const SINGLETON_SELECTOR = keccak256(toHex('SINGLETON()')).slice(0, 10);

/**
 * The eleven, with an RPC that answered on 2026-09-23. Two of the built-in
 * RPCs would not: Ink's `rpc-gel` refused the connection and World Chain's
 * drpc endpoint rate-limited, so both are the alternates here.
 */
const CHAINS = [
	{ id: 57073, name: 'Ink', symbol: 'ETH', rpc: 'https://rpc-qnd.inkonchain.com' },
	{ id: 8217, name: 'Kaia', symbol: 'KAIA', rpc: 'https://public-en.node.kaia.io' },
	{ id: 5000, name: 'Mantle', symbol: 'MNT', rpc: 'https://rpc.mantle.xyz' },
	{ id: 4326, name: 'MegaETH', symbol: 'ETH', rpc: 'https://megaeth.drpc.org' },
	{ id: 143, name: 'Monad', symbol: 'MON', rpc: 'https://rpc.monad.xyz' },
	{ id: 98866, name: 'Plume', symbol: 'PLUME', rpc: 'https://rpc.plume.org' },
	{ id: 1868, name: 'Soneium', symbol: 'ETH', rpc: 'https://rpc.soneium.org' },
	{ id: 130, name: 'Unichain', symbol: 'ETH', rpc: 'https://mainnet.unichain.org' },
	{
		id: 480,
		name: 'World Chain',
		symbol: 'ETH',
		rpc: 'https://worldchain-mainnet.g.alchemy.com/public'
	},
	{ id: 196, name: 'X Layer', symbol: 'OKB', rpc: 'https://rpc.xlayer.tech' },
	{ id: 1440000, name: 'XRPL EVM', symbol: 'XRP', rpc: 'https://rpc.xrplevm.org' }
];

const here = (p) => fileURLToPath(new URL(`../${p}`, import.meta.url));
const data = JSON.parse(readFileSync(here('src/lib/chain-setup/deployment-data.json'), 'utf8'));
const entry = data.safePasskeySignerFactory;
if (entry?.factory !== 'safeSingletonFactory') {
	console.error('deployment-data.json no longer deploys the factory through Safe. Stopping.');
	process.exit(2);
}

// The address this data WILL produce, before anything is sent.
const computed = getCreate2Address({
	from: SAFE_SINGLETON_FACTORY,
	salt: entry.salt,
	bytecodeHash: keccak256(entry.initCode)
});
if (computed.toLowerCase() !== EXPECTED_FACTORY.toLowerCase()) {
	console.error(
		`The deployment data produces ${computed}, but the wallet looks for ${EXPECTED_FACTORY}.\n` +
			'Deploying it would put a contract somewhere nothing checks. Stopping.'
	);
	process.exit(2);
}

const broadcast = process.argv.includes('--broadcast');
const only = process.argv.includes('--chain')
	? Number(process.argv[process.argv.indexOf('--chain') + 1])
	: null;
const key = process.env.VELA_DEPLOYER_KEY;
if (broadcast && !key) {
	console.error('--broadcast needs VELA_DEPLOYER_KEY in the environment (never as an argument).');
	process.exit(2);
}
const account = key ? privateKeyToAccount(key) : null;

const calldata = `${entry.salt}${entry.initCode.slice(2)}`;
const GAS = 1_300_000n; // measured 1,049,765 on Unichain, with headroom

console.log(`factory ${EXPECTED_FACTORY} · singleton ${EXPECTED_SINGLETON}`);
console.log(account ? `deployer ${account.address}` : 'no key — reporting only');
console.log(broadcast ? 'MODE: broadcast\n' : 'MODE: dry run (pass --broadcast to send)\n');

let missing = 0;
let sent = 0;
for (const chain of CHAINS) {
	if (only !== null && chain.id !== only) continue;
	const def = {
		id: chain.id,
		name: chain.name,
		nativeCurrency: { name: chain.symbol, symbol: chain.symbol, decimals: 18 },
		rpcUrls: { default: { http: [chain.rpc] } }
	};
	const pub = createPublicClient({ chain: def, transport: http(chain.rpc, { timeout: 20_000 }) });
	const label = chain.name.padEnd(13);
	try {
		const [already, safeFactory, price] = await Promise.all([
			pub.getCode({ address: EXPECTED_FACTORY }),
			pub.getCode({ address: SAFE_SINGLETON_FACTORY }),
			pub.getGasPrice()
		]);
		if (already && already !== '0x') {
			console.log(`${label} already deployed — nothing to do`);
			continue;
		}
		if (!safeFactory || safeFactory === '0x') {
			// Safe signs these per chain on request; nobody else can.
			console.log(
				`${label} BLOCKED: Safe's singleton factory is not here. Ask for it at ` +
					'https://github.com/safe-global/safe-singleton-factory/issues and come back.'
			);
			continue;
		}
		missing++;
		const cost = price * GAS;
		const balance = account ? await pub.getBalance({ address: account.address }) : 0n;
		const funded = account ? balance >= cost : false;
		console.log(
			`${label} needs deploying · ≈ ${formatEther(cost)} ${chain.symbol}` +
				(account
					? ` · deployer holds ${formatEther(balance)} ${chain.symbol}${funded ? '' : ' — NOT ENOUGH'}`
					: '')
		);
		if (!broadcast) continue;
		if (!funded) {
			console.log(`${label} skipped: fund ${account.address} on ${chain.name} first`);
			continue;
		}
		const wallet = createWalletClient({
			account,
			chain: def,
			transport: http(chain.rpc, { timeout: 20_000 })
		});
		const hash = await wallet.sendTransaction({
			to: SAFE_SINGLETON_FACTORY,
			data: calldata,
			gas: GAS
		});
		const receipt = await pub.waitForTransactionReceipt({ hash, timeout: 180_000 });
		if (receipt.status !== 'success') {
			console.log(`${label} FAILED in ${hash}`);
			continue;
		}
		// Believe the chain, not the receipt: check both addresses answer.
		const [code, singleton] = await Promise.all([
			pub.getCode({ address: EXPECTED_FACTORY }),
			pub.call({ to: EXPECTED_FACTORY, data: SINGLETON_SELECTOR })
		]);
		const points = `0x${(singleton.data ?? '').slice(-40)}`.toLowerCase();
		const ok = code && code !== '0x' && points === EXPECTED_SINGLETON.toLowerCase();
		console.log(`${label} ${ok ? 'DEPLOYED' : 'deployed but UNVERIFIED'} in ${hash}`);
		if (ok) sent++;
	} catch (error) {
		console.log(
			`${label} could not ask: ${String(error?.shortMessage ?? error?.message).slice(0, 80)}`
		);
	}
}

console.log(
	`\n${missing} network${missing === 1 ? '' : 's'} still missing the factory` +
		(broadcast ? `, ${sent} deployed in this run` : ' — nothing was sent')
);
