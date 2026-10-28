/**
 * Clear-signing audit, surface 1: the web wallet's signing sheet for dApp
 * requests (packaged extension, fork, virtual passkey). Every request is
 * REVIEWED and then refused with the sheet's ✕ — nothing is signed here.
 * Each case: the face (what a person sees), the details, a screenshot.
 */
import { test, type Page, type Route } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { ask, closeServer, D, launch, noRequestWindow, requestWindow, serveDApp, shot, watch, within, type Logs } from './ext';
import { makeAccount, textOf } from './account';

const PORT = Number(process.env.CS_PORT ?? 8951);
const ONLY = process.env.CS_ONLY ? new RegExp(process.env.CS_ONLY) : null;
const C = JSON.parse(readFileSync(`${D}/contracts.json`, 'utf8'));
const cast = (...a: string[]) => execFileSync('cast', a, { encoding: 'utf8' }).trim();
const cd = (sig: string, ...a: string[]) => cast('calldata', sig, ...a);

const USDC = '0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83';
const WXDAI = '0xe91D153E0b41518A2Ce8Dd3D7944Fa863463a97d';
const RECIP = '0xaF5e8917831Ef08A64e18b2Cde9f8f5d32c7b3e1';
const PERMIT2 = '0x000000000022D473030F116dDEE9F6B43aC78BA3';
const V3R2 = '0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45';
const UR = '0x66a9893cC07D91D95644AEDD05D03f95e1dBA8Af';
const WETH_MAINNET = '0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2';
const ONEINCH_V6 = '0x111111125421cA6dc452d289314280a0f8842A65';
const MAX = '115792089237316195423570985008687907853269984665640564039457584007913129639935';
const hex = (n: bigint) => '0x' + n.toString(16);
const E18 = 10n ** 18n;

type Case = { name: string; method: string; params: (a: string) => unknown[]; chain?: string; route?: (page: Page) => Promise<void> };

const eip712Domain = [
	{ name: 'name', type: 'string' },
	{ name: 'version', type: 'string' },
	{ name: 'chainId', type: 'uint256' },
	{ name: 'verifyingContract', type: 'address' }
];
const permitTypes = {
	EIP712Domain: eip712Domain,
	Permit: [
		{ name: 'owner', type: 'address' },
		{ name: 'spender', type: 'address' },
		{ name: 'value', type: 'uint256' },
		{ name: 'nonce', type: 'uint256' },
		{ name: 'deadline', type: 'uint256' }
	]
};
const permit = (a: string, token: string, value: string, chainId = 100) => ({
	types: permitTypes,
	primaryType: 'Permit',
	domain: { name: 'USD Coin', version: '2', chainId, verifyingContract: token },
	message: { owner: a, spender: C.rewards, value, nonce: 0, deadline: 1893456000 }
});
const permit2Single = (a: string) => ({
	types: {
		EIP712Domain: [
			{ name: 'name', type: 'string' },
			{ name: 'chainId', type: 'uint256' },
			{ name: 'verifyingContract', type: 'address' }
		],
		PermitSingle: [
			{ name: 'details', type: 'PermitDetails' },
			{ name: 'spender', type: 'address' },
			{ name: 'sigDeadline', type: 'uint256' }
		],
		PermitDetails: [
			{ name: 'token', type: 'address' },
			{ name: 'amount', type: 'uint160' },
			{ name: 'expiration', type: 'uint48' },
			{ name: 'nonce', type: 'uint48' }
		]
	},
	primaryType: 'PermitSingle',
	domain: { name: 'Permit2', chainId: 100, verifyingContract: PERMIT2 },
	message: {
		details: { token: USDC, amount: '1461501637330902918203684832716283019655932542975', expiration: '1893456000', nonce: '0' },
		spender: C.sweeper,
		sigDeadline: '1893456000'
	},
	_a: a
});
const mail = {
	types: { EIP712Domain: eip712Domain, Mail: [{ name: 'from', type: 'string' }, { name: 'contents', type: 'string' }] },
	primaryType: 'Mail',
	domain: { name: 'Explore', version: '1', chainId: 100, verifyingContract: '0xCcCCccccCCCCcCCCCCCcCcCccCcCCCcCcccccccC' },
	message: { from: 'Explorer', contents: 'Hello typed data' }
};
const siwe = (a: string, domain: string, uri: string) =>
	`${domain} wants you to sign in with your Ethereum account:\n${a}\n\nSign in to the explore dApp.\n\nURI: ${uri}\nVersion: 1\nChain ID: 100\nNonce: 32891756\nIssued At: 2026-10-28T08:00:00.000Z`;
const utf8hex = (s: string) => '0x' + Buffer.from(s, 'utf8').toString('hex');

/** A third party's ERC-7730 descriptor for a contract: the shape the wallet fetches. */
function evilDescriptor(sig: string, intent: string, fields: unknown[], name = 'Vela Rewards', owner = 'Vela') {
	return JSON.stringify({
		context: { contract: { deployments: [{ chainId: 100, address: '0x0' }] } },
		metadata: { owner, contractName: name },
		display: { formats: { [sig]: { intent, fields } } }
	});
}
async function routeDescriptor(page: Page, address: string, body: string) {
	await page.context().route(`**/erc7730/calldata/eip155-100/${address.toLowerCase()}.json`, (r: Route) =>
		r.fulfill({ status: 200, contentType: 'application/json', body })
	);
}

// Universal Router: execute(bytes commands, bytes[] inputs, uint256 deadline), V3_SWAP_EXACT_IN (0x00).
const v3path = '0x' + USDC.slice(2).toLowerCase() + '0001f4' + WXDAI.slice(2).toLowerCase();
const urInput = cast('abi-encode', 'f(address,uint256,uint256,bytes,bool)', C.thief, '250000000', '1', v3path, 'true');

const CASES: Case[] = [
	{ name: 'w01-erc20-transfer-usdc', method: 'eth_sendTransaction', params: (a) => [{ from: a, to: USDC, data: cd('transfer(address,uint256)', RECIP, '250000000') }] },
	{ name: 'w02-erc20-transfer-fake-usdc', method: 'eth_sendTransaction', params: (a) => [{ from: a, to: C.fakeUsdc6, data: cd('transfer(address,uint256)', RECIP, '250000000') }] },
	{ name: 'w03-approve-finite', method: 'eth_sendTransaction', params: (a) => [{ from: a, to: USDC, data: cd('approve(address,uint256)', C.rewards, '100000000') }] },
	{ name: 'w04-approve-unlimited', method: 'eth_sendTransaction', params: (a) => [{ from: a, to: USDC, data: cd('approve(address,uint256)', C.rewards, MAX) }] },
	{ name: 'w05-approve-unlimited-fake-usdc', method: 'eth_sendTransaction', params: (a) => [{ from: a, to: C.fakeUsdc6, data: cd('approve(address,uint256)', C.sweeper, MAX) }] },
	{ name: 'w06-increase-allowance', method: 'eth_sendTransaction', params: (a) => [{ from: a, to: USDC, data: cd('increaseAllowance(address,uint256)', C.rewards, '50000000') }] },
	{ name: 'w07-permit-usdc', method: 'eth_signTypedData_v4', params: (a) => [a, JSON.stringify(permit(a, USDC, '1000000'))] },
	{ name: 'w08-permit-fake-usdc', method: 'eth_signTypedData_v4', params: (a) => [a, JSON.stringify(permit(a, C.fakeUsdc6, '1000000'))] },
	{ name: 'w09-permit2-single-max', method: 'eth_signTypedData_v4', params: (a) => { const p = permit2Single(a); delete (p as { _a?: string })._a; return [a, JSON.stringify(p)]; } },
	{ name: 'w10-set-approval-for-all', method: 'eth_sendTransaction', params: (a) => [{ from: a, to: C.nft, data: cd('setApprovalForAll(address,bool)', C.rewards, 'true') }] },
	{
		name: 'w11-v3router2-swap-recipient-thief',
		method: 'eth_sendTransaction',
		params: (a) => [{ from: a, to: V3R2, data: cd('exactInputSingle((address,address,uint24,address,uint256,uint256,uint160))', `(${USDC},${WXDAI},500,${C.thief},250000000,1,0)`) }]
	},
	{ name: 'w12-universal-router-swap', method: 'eth_sendTransaction', params: (a) => [{ from: a, to: UR, data: cd('execute(bytes,bytes[],uint256)', '0x00', `[${urInput}]`, '1893456000') }] },
	{
		name: 'w13-multicall-unknown',
		method: 'eth_sendTransaction',
		params: (a) => [{ from: a, to: C.rewards, data: cd('multicall(bytes[])', `[${cd('claimRewards(address,uint256)', USDC, '1')},${cd('transfer(address,uint256)', C.thief, '5')}]`) }]
	},
	{ name: 'w14-wxdai-deposit-value', method: 'eth_sendTransaction', params: (a) => [{ from: a, to: WXDAI, data: cd('deposit()'), value: hex(1n * E18) }] },
	{ name: 'w15-weth-mainnet-addr-on-gnosis', method: 'eth_sendTransaction', params: (a) => [{ from: a, to: WETH_MAINNET, data: cd('deposit()'), value: hex(1n * E18) }] },
	{ name: 'w16-erc4626-shape-not-vault-value', method: 'eth_sendTransaction', params: (a) => [{ from: a, to: C.rewards, data: cd('deposit(uint256,address)', '1000000', a), value: hex(2n * E18) }] },
	{ name: 'w17-unknown-selector-value', method: 'eth_sendTransaction', params: (a) => [{ from: a, to: C.rewards, data: cd('zzUnknown_9f3(uint256)', '7'), value: hex(E18 / 2n) }] },
	{ name: 'w18-claimrewards-4byte-value', method: 'eth_sendTransaction', params: (a) => [{ from: a, to: C.rewards, data: cd('claimRewards(address,uint256)', USDC, '1000000000'), value: hex(3n * E18) }] },
	{
		name: 'w19-collision-claim-airdrop',
		method: 'eth_sendTransaction',
		params: (a) => [{ from: a, to: C.sweeper, data: cd('sweepTokens_1y1r(address,uint256)', USDC, '5000000000') }],
		// An attacker registers the colliding name; the DB answers it (simulated).
		route: async (page) => {
			const ctx = page.context();
			await ctx.route('**/signature-database/v1/lookup**', (r) =>
				r.request().url().includes('0x3d4ebdfe')
					? r.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ ok: true, result: { function: { '0x3d4ebdfe': [{ name: 'claimAirdrop_e9x(address,uint256)', filtered: false }] }, event: {} } }) })
					: r.continue()
			);
		}
	},
	{ name: 'w20-typed-unknown-mail', method: 'eth_signTypedData_v4', params: (a) => [a, JSON.stringify(mail)] },
	{ name: 'w21-siwe-ok', method: 'personal_sign', params: (a) => [utf8hex(siwe(a, `127.0.0.1:${PORT}`, `http://127.0.0.1:${PORT}`)), a] },
	{ name: 'w22-siwe-mismatch', method: 'personal_sign', params: (a) => [utf8hex(siwe(a, 'app.uniswap.org', 'https://app.uniswap.org')), a] },
	{ name: 'w23-personal-bidi', method: 'personal_sign', params: (a) => [utf8hex('Approve transfer to Bob ‮gpj.exe‬​​ ok\nAmount: 1‍0 USDC⁦ hidden⁩'), a] },
	{ name: 'w24-personal-hidden-below', method: 'personal_sign', params: (a) => [utf8hex('Welcome to Explore! Click to sign in.' + '\n'.repeat(60) + 'I authorise transfer of all my assets to 0xBAdBad.'), a] },
	{
		name: 'w25-sendcalls-mixed',
		method: 'wallet_sendCalls',
		params: (a) => [{
			version: '2.0.0', chainId: '0x64', from: a, atomicRequired: true,
			calls: [
				{ to: USDC, data: cd('transfer(address,uint256)', RECIP, '1000000'), value: '0x0' },
				{ to: C.rewards, data: cd('zzUnknown_9f3(uint256)', '7'), value: hex(E18) },
				{ to: C.fakeUsdc6, data: cd('approve(address,uint256)', C.sweeper, MAX), value: '0x0' },
				{ to: C.rewards, data: cd('deposit(uint256,address)', '1', a), value: hex(E18 / 10n) }
			]
		}]
	},
	{
		// A third-party descriptor for the attacker's own token (as a registry
		// PR or a custom chain-data URL would supply): approve renamed "Claim",
		// spender labelled "Reward from", the amount hidden.
		name: 'w26-evil-descriptor-approve-as-claim',
		method: 'eth_sendTransaction',
		params: (a) => [{ from: a, to: C.fakeUsdc6, data: cd('approve(address,uint256)', C.sweeper, MAX) }],
		route: (page) =>
			routeDescriptor(page, C.fakeUsdc6, evilDescriptor('approve(address spender,uint256 amount)', 'Claim', [
				{ path: 'spender', label: 'Reward from', format: 'addressName' },
				{ path: 'amount', label: 'Amount', format: 'raw', visible: 'never' }
			], 'USDC Rewards', 'Circle'))
	},
	{
		// The sweeper's descriptor: recipient called "Network fee", the amount scaled by the wrong token.
		name: 'w27-evil-descriptor-fee-label',
		method: 'eth_sendTransaction',
		params: (a) => [{ from: a, to: C.sweeper, data: cd('sweepTokens_1y1r(address,uint256)', USDC, '5000000000000') }],
		route: (page) =>
			routeDescriptor(page, C.sweeper, evilDescriptor('sweepTokens_1y1r(address token,uint256 amount)', 'Claim airdrop', [
				{ path: 'amount', label: 'You receive', format: 'tokenAmount', params: { token: '0xe91d153e0b41518a2ce8dd3d7944fa863463a97d' } },
				{ path: 'token', label: 'Network fee paid to', format: 'addressName' }
			], 'Gnosis Airdrop', 'Gnosis'))
	},
	{
		// The same descriptor service answering for a WELL-KNOWN name on an empty address.
		name: 'w28-evil-descriptor-unknown-sel-as-swap',
		method: 'eth_sendTransaction',
		params: (a) => [{ from: a, to: C.rewards, data: cd('zzUnknown_9f3(uint256)', '7'), value: hex(2n * E18) }],
		route: (page) =>
			routeDescriptor(page, C.rewards, evilDescriptor('zzUnknown_9f3(uint256 x)', 'Swap', [
				{ path: 'x', label: 'You receive (min)', format: 'raw' }
			], 'Uniswap V3 Router', 'Uniswap'))
	},
	{ name: 'w31-swap-receive-fake-usdc', method: 'eth_sendTransaction', params: (a) => [{ from: a, to: V3R2, data: cd('exactInputSingle((address,address,uint24,address,uint256,uint256,uint160))', `(${WXDAI},${C.fakeUsdc6},500,${a},5000000000000000000,1000000000,0)`) }] },
	{ name: 'w32-approve-permit2-unlimited', method: 'eth_sendTransaction', params: (a) => [{ from: a, to: USDC, data: cd('approve(address,uint256)', PERMIT2, MAX) }] },
	// Ethereum mainnet (real descriptor service, read-only): built-in vs fetched.
	{ name: 'w29-mainnet-v3router2-builtin', chain: '0x1', method: 'eth_sendTransaction', params: (a) => [{ from: a, to: V3R2, data: cd('exactInputSingle((address,address,uint24,address,uint256,uint256,uint160))', `(0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48,${WETH_MAINNET},500,${a},250000000,1,0)`) }] },
	{
		name: 'w30-mainnet-1inch-v6-fetched',
		chain: '0x1',
		method: 'eth_sendTransaction',
		params: (a) => [{ from: a, to: ONEINCH_V6, data: cd('swap(address,(address,address,address,address,uint256,uint256,uint256),bytes)', C.thief, `(0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48,${WETH_MAINNET},${C.thief},${C.thief},250000000,1,0)`, '0x') }]
	}
];

test('clear signing on the web sheet', async () => {
	test.setTimeout(60 * 60_000);
	const logs: Logs = { errors: [], console: [] };
	const out: Record<string, unknown>[] = [];
	const save = () => writeFileSync(`${D}/w1-report.json`, JSON.stringify({ out, logs }, null, 1));
	const server = await serveDApp(PORT);
	const context = await launch('web', { logs });
	const { address } = await makeAccount(context, 'Clear One');
	const dapp = await context.newPage();
	watch(dapp, logs, 'dapp');
	await dapp.goto(`http://127.0.0.1:${PORT}/`);
	await dapp.waitForTimeout(800);
	{
		const asked = ask(dapp, 'eth_requestAccounts');
		const win = await requestWindow(context);
		await win.waitForTimeout(1500);
		await shot(win, 'w00-connect');
		out.push({ name: 'w00-connect', face: (await textOf(win)).replace(/\s+/g, ' ').slice(0, 800) });
		await win.getByRole('button', { name: 'Connect', exact: true }).click();
		await within(asked, 20000);
		await noRequestWindow(context);
	}
	let chain = '0x64';
	await ask(dapp, 'wallet_switchEthereumChain', [{ chainId: chain }]);
	for (const c of CASES) {
		if (ONLY && !ONLY.test(c.name)) continue;
		const t0 = Date.now();
		try {
			const want = c.chain ?? '0x64';
			if (want !== chain) {
				await ask(dapp, 'wallet_switchEthereumChain', [{ chainId: want }]);
				chain = want;
			}
			await context.unrouteAll({ behavior: 'ignoreErrors' });
			if (c.route) await c.route(dapp);
			const asked = ask(dapp, c.method, c.params(address), c.name);
			const win = await requestWindow(context, 20000).catch(() => null);
			if (!win) {
				out.push({ name: c.name, noWindow: await within(asked, 5000) });
				save();
				continue;
			}
			const net: string[] = [];
			win.on('request', (r) => {
				const u = r.url();
				if (/erc7730|signature-database|4byte/.test(u)) net.push(u.replace(/^https?:\/\//, '').slice(0, 140));
			});
			// Let the reading and the simulation land.
			await win.getByTestId('signing-confirm').waitFor({ timeout: 30000 }).catch(() => {});
			for (let i = 0; i < 40; i++) {
				const t = await textOf(win);
				if (!/Loading|Checking/.test(t)) break;
				await win.waitForTimeout(500);
			}
			await win.waitForTimeout(2500);
			const face = (await textOf(win)).replace(/\s+/g, ' ');
			await win.screenshot({ path: `${D}/shots/${c.name}.png`, fullPage: true });
			await win.getByRole('button', { name: 'Technical details' }).click().catch(() => {});
			await win.waitForTimeout(800);
			await win.evaluate(() => document.querySelectorAll('*').forEach((n) => { if ((n as HTMLElement).scrollHeight > (n as HTMLElement).clientHeight + 4) (n as HTMLElement).scrollTop = 99999; })).catch(() => {});
			await win.screenshot({ path: `${D}/shots/${c.name}-details.png`, fullPage: true });
			const details = (await textOf(win)).replace(/\s+/g, ' ');
			const confirm = await win.getByTestId('signing-confirm').evaluate((b) => ({ text: (b as HTMLElement).innerText, disabled: (b as HTMLButtonElement).disabled })).catch(() => null);
			await win.getByRole('button', { name: 'Close', exact: true }).first().click().catch(() => win.close());
			const answer = await within(asked, 15000);
			await noRequestWindow(context);
			out.push({ name: c.name, ms: Date.now() - t0, face, details, confirm, net, answer });
		} catch (e) {
			out.push({ name: c.name, error: String(e).slice(0, 300) });
		}
		save();
	}
	save();
	await context.close();
	await closeServer(server);
});
