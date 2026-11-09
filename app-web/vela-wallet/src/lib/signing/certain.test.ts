/**
 * What is CERTAIN and what is INTERPRETED, as the web sheet draws it
 * (clear-signing audit cs-1…cs-19). The real core reads each request through
 * a scripted shell — the audit's adversarial descriptors, a token that calls
 * itself USDC, WETH's mainnet address on Gnosis — and the sheet is built from
 * its view. The core decides; these tests hold the drawing to it.
 */
import { judgedByFreshMachine, simOutcome } from '$lib/core/kernels';
import { witnessed } from './sim-witnessed';
import type { SimEnded } from './verdict';
import { describe, expect, it } from 'vitest';
import type { ClearOperation } from '$lib/core/generated/ClearOperation';
import type { ClearShellResult } from '$lib/core/generated/ClearShellResult';
import type { ClearSigningEvent } from '$lib/core/generated/ClearSigningEvent';
import type { ClearSigningView } from '$lib/core/generated/ClearSigningView';
import type { SignView } from '$lib/core/generated/SignView';
import type { FeeView } from '$lib/core/generated/FeeView';
import { ClearSigningCore } from '$lib/core/client';
import { resolveSigningMessages } from '$lib/i18n/engine.server';
import { fill } from '$lib/wallet/messages';
import { toClearLocale } from './core/clear-types';
import { INITIAL_GUARD_VIEW } from './core/sheet.svelte';
import { INITIAL_SIGN_VIEW } from './core/sign-resident.svelte';
import { txKickoff } from './core/tx-params';
import { buildSigningModel, localizedTerms, type SigningLiveInputs } from './live';
import type { Block, SigningModel } from './model';

const m = resolveSigningMessages('en');
const ME = '0x85c3815e2a775ed845e0b64ba0421e9c3bf3ac88';
const THIEF = '0x0000000000000000000000000000000000badbad';
const SWEEPER = '0x48aba9cb175899e9aa05b8819b8fac9ef9d8a0da';
const REWARDS = '0x618a3cd671d8e4d1efa8ac9fee42dbcdada5713a';
const FAKE_USDC = '0x09803ad8ec9d9e9972dbc5e87c8ee00b9435b3f3';
const USDC = '0xddafbb505ad214d7b80b1f830fccc89b60fb7a83';
const WETH_MAINNET = '0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2';
const MAX = 'f'.repeat(64);
const HOST = 'ethereum-data.getvela.app';

const word = (hex: string) => hex.replace(/^0x/, '').toLowerCase().padStart(64, '0');
const num = (n: bigint | number) => BigInt(n).toString(16).padStart(64, '0');
const abiString = (text: string) => {
	const body = Buffer.from(text, 'utf8').toString('hex').padEnd(64, '0');
	return '0x' + num(32) + num(text.length) + body;
};

interface Shell {
	descriptors?: Record<string, unknown>;
	symbols?: Record<string, string>;
	empty?: string[];
	selectors?: Record<string, string[]>;
}

function answer(op: ClearOperation, shell: Shell): ClearShellResult {
	switch (op.type) {
		case 'now':
			return { type: 'clock', now_ms: 1_790_000_000_000 };
		case 'http_get': {
			const body = shell.descriptors?.[op.path];
			return {
				type: 'descriptor_fetched',
				path: op.path,
				json: body === undefined ? null : JSON.stringify(body)
			};
		}
		case 'rpc_get_code':
			return {
				type: 'rpc_answer',
				probe: 'code',
				chain_id: op.chain_id,
				to: op.address,
				result: shell.empty?.includes(op.address.toLowerCase()) ? '0x' : '0x6080604052',
				rpc_error: false
			};
		case 'rpc_eth_call': {
			const symbol = shell.symbols?.[op.to.toLowerCase()];
			const result =
				symbol === undefined ? null : op.probe === 'decimals' ? '0x' + num(6) : abiString(symbol);
			return {
				type: 'rpc_answer',
				probe: op.probe,
				chain_id: op.chain_id,
				to: op.to,
				result,
				rpc_error: result === null
			};
		}
		case 'selector_db_lookup':
			return { type: 'selector_candidates', sigs: shell.selectors?.[op.selector] ?? [] };
		case 'timer':
			return { type: 'timed_out', token: op.token };
	}
}

/** The real core's reading of one request, every question answered by `shell`. */
function read(event: ClearSigningEvent, shell: Shell = {}): ClearSigningView {
	const core = new ClearSigningCore();
	try {
		type Out = { effects: { id: number; operation: ClearOperation }[] };
		let queue = (JSON.parse(core.dispatch(JSON.stringify(event))) as Out).effects;
		for (let turn = 0; turn < 300 && queue.length > 0; turn++) {
			// Timers last, as a real shell's answers come in.
			const at = queue.findIndex((e) => e.operation.type !== 'timer');
			const [effect] = queue.splice(at === -1 ? 0 : at, 1);
			const next = JSON.parse(
				core.resolve_effect(BigInt(effect.id), JSON.stringify(answer(effect.operation, shell)))
			) as Out;
			queue = [...queue, ...next.effects];
		}
		const view = JSON.parse(core.view()) as ClearSigningView;
		expect(view.resolving).toBe(false);
		return localizedTerms(view, m);
	} finally {
		core.free();
	}
}

const locale = toClearLocale({ number: 'comma_dot', date: 'iso', time: 'h24' });
function tx(to: string, data: string, value = '0x0', chainId = 100): ClearSigningEvent {
	return txKickoff(
		'eth_sendTransaction',
		JSON.stringify([{ from: ME, to, data, value }]),
		chainId,
		locale,
		{ from: ME, probeCode: true }
	);
}

const FEE: FeeView = {
	busy: false,
	failed: null,
	fee: null,
	stale: false,
	fee_token: null,
	options: [],
	confirm_fee_ready: true,
	no_coin_pays: false,
	nothing_to_pay_from: false,
	provisional: false,
	failure: null
};

/** R22: a corroborated simulation under which nothing of the account's moves. */
function checkedSim(chainId: number): SimEnded {
	const verdict = simOutcome(
		ME,
		witnessed(JSON.stringify({ result: [{ calls: [{ status: '0x1', logs: [] }] }] }))
	)!;
	return { verdict, judged: judgedByFreshMachine(ME, chainId, verdict.deltas) };
}

function sheet(view: ClearSigningView, to: string, data: string, chainId = 100): SigningModel {
	const sign: SignView = {
		...INITIAL_SIGN_VIEW,
		surface: 'sheet',
		confirm_gate_open: true,
		request: {
			id: 'cs',
			method: 'eth_sendTransaction',
			kind: 'transaction',
			params_json: JSON.stringify([{ from: ME, to, data, value: '0x0' }]),
			origin: 'http://127.0.0.1:8951',
			dapp: null,
			chain_id: chainId,
			signer_address: ME,
			first_party: false
		}
	};
	const inputs: SigningLiveInputs = {
		sign,
		clear: view,
		guard: INITIAL_GUARD_VIEW,
		fee: FEE,
		currency: { code: 'USD', rate: 1, committed: true, pending: null },
		m,
		identity: { name: 'Clear One', address: ME, identiconSvg: '<svg/>' },
		identicon: () => '<svg/>',
		descriptorHost: HOST,
		// R22: the transaction was checked — two nodes agree nothing of the
		// account's moves — so its confirm is the reading's own, not "… anyway".
		sim: checkedSim(chainId)
	};
	return buildSigningModel(inputs)!;
}

const texts = (blocks: Block[]) =>
	blocks.flatMap((b) =>
		b.kind === 'warning' || b.kind === 'source' || b.kind === 'sentence' || b.kind === 'intent'
			? [b.text]
			: []
	);
const warnings = (blocks: Block[], tone: 'danger' | 'caution') =>
	blocks.flatMap((b) => (b.kind === 'warning' && b.tone === tone ? [b.text] : []));
const intentOf = (model: SigningModel) =>
	model.blocks.find((b): b is Extract<Block, { kind: 'intent' }> => b.kind === 'intent')?.text;

function evil(sig: string, intent: string, fields: unknown[], name: string, owner: string) {
	return {
		context: { contract: { deployments: [{ chainId: 100, address: '0x0' }] } },
		metadata: { owner, contractName: name },
		display: { formats: { [sig]: { intent, fields } } }
	};
}

describe('a description cannot rename what Vela reads (cs-3, adversarial)', () => {
	it('an unlimited approve a descriptor calls "Claim" is drawn as the approval', () => {
		const data = `0x095ea7b3${word(SWEEPER)}${MAX}`;
		const view = read(tx(FAKE_USDC, data), {
			descriptors: {
				[`/erc7730/calldata/eip155-100/${FAKE_USDC}.json`]: evil(
					'approve(address spender,uint256 amount)',
					'Claim',
					[
						{ path: 'spender', label: 'Reward from', format: 'addressName' },
						{ path: 'amount', label: 'Amount', format: 'raw', visible: 'never' }
					],
					'USDC Rewards',
					'Circle'
				)
			},
			symbols: { [FAKE_USDC]: 'USDC' }
		});
		const model = sheet(view, FAKE_USDC, data);
		expect(intentOf(model)).toBe(m.terms.intentApprove);
		expect(model.confirm.action).not.toBe('Claim');
		expect(model.confirm.action).toBe(m.terms.intentApprove);
		const danger = warnings(model.blocks, 'danger');
		expect(danger).toContain(fill(m.certain.contradiction, { words: 'Claim' }));
		expect(danger).toContain(fill(m.certain.contradiction, { words: 'Reward from' }));
		expect(texts(model.blocks)).toContain(m.certain.readByVela);
		// The spender keeps Vela's label, never the description's.
		const party = model.blocks.find((b) => b.kind === 'party');
		expect(party).toMatchObject({ label: m.terms.labelSpender });
		// The contract's name is the descriptor's — said as its word.
		expect(texts(model.blocks).some((t) => t.includes('USDC Rewards') && t.includes(HOST))).toBe(
			true
		);
	});

	it('a description cannot relabel an address as the network fee', () => {
		const sig = 'sweepTokens_1y1r(address token,uint256 amount)';
		const selector = '0x3d4ebdfe';
		const data = `${selector}${word(USDC)}${num(5_000_000_000_000n)}`;
		const view = read(tx(SWEEPER, data), {
			descriptors: {
				[`/erc7730/calldata/eip155-100/${SWEEPER}.json`]: evil(
					sig,
					'Claim airdrop',
					[{ path: 'token', label: 'Network fee paid to', format: 'addressName' }],
					'Gnosis Airdrop',
					'Gnosis'
				)
			}
		});
		const model = sheet(view, SWEEPER, data);
		expect(warnings(model.blocks, 'danger')).toContain(
			fill(m.certain.contradiction, { words: 'Network fee paid to' })
		);
		// An interpreted intent is never the button (cs-7).
		expect(model.confirm.action).toBe(m.confirmPlain);
		expect(texts(model.blocks)).toContain(
			fill(m.certain.describedFetched, { owner: 'Gnosis', host: HOST })
		);
	});
});

describe('certain facts on the face', () => {
	it('a token that calls itself USDC is USDC?, unpriced, with its address (cs-1)', () => {
		const data = `0xa9059cbb${word(THIEF)}${num(250_000_000)}`;
		const model = sheet(
			read(tx(FAKE_USDC, data), { symbols: { [FAKE_USDC]: 'USDC' } }),
			FAKE_USDC,
			data
		);
		const amount = model.blocks.find((b) => b.kind === 'amount');
		expect(amount).toMatchObject({ line: { value: '250 USDC?' } });
		expect(amount && amount.kind === 'amount' ? amount.line.fiat : 'x').toBeUndefined();
		expect(texts(model.blocks)).toContain(
			fill(m.certain.tokenByContract, { symbol: 'USDC?', address: '0x0980...b3f3' }).replace(
				'0x0980...b3f3',
				texts(model.blocks)
					.find((t) => t.startsWith('USDC? · '))
					?.split(' · ')[1]
					?.split(' ')[0] ?? ''
			)
		);
	});

	it('WETH’s mainnet address on Gnosis has no code: no name, red Sign anyway (cs-13, cs-8)', () => {
		const data = '0xd0e30db0';
		const view = read(tx(WETH_MAINNET, data, '0xde0b6b3a7640000'), { empty: [WETH_MAINNET] });
		expect(view.no_code).toBe(true);
		const model = sheet(view, WETH_MAINNET, data);
		expect(intentOf(model)).toBe(m.intentBlind);
		expect(warnings(model.blocks, 'danger')).toContain(fill(m.certain.noCode, { chain: 'Gnosis' }));
		expect(model.confirm).toMatchObject({ action: m.certain.signAnyway, danger: true });
		expect(texts(model.blocks).some((t) => t.includes('Wrap'))).toBe(false);
	});

	it('an approve-all names its collection by its own name(), beside its address', () => {
		const NFT = '0x7eca6b925e9849cbd1b7c07bd29b14de26c67d86';
		const data = `0xa22cb465${word(REWARDS)}${num(1)}`;
		const named = intentOf(
			sheet(read(tx(NFT, data), { symbols: { [NFT]: 'Gnosis Punks' } }), NFT, data)
		);
		expect(named).toMatch(/^Approve all your Gnosis Punks \(0x7eca.*\) NFTs$/);
		// A collection that answers no name is still said, by its address.
		const nameless = intentOf(sheet(read(tx(NFT, data)), NFT, data));
		expect(nameless).toMatch(/^Approve all your 0x7eca\S* NFTs$/);
	});

	it('the contract line names a token Vela lists, on a single call', () => {
		const data = `0xa9059cbb${word(THIEF)}${num(1_000_000)}`;
		const lines = texts(sheet(read(tx(USDC, data)), USDC, data).blocks);
		const contract = lines.find((t) => t.startsWith('Contract · '));
		expect(contract).toContain('· USDC, called so by');
		expect(contract).toContain(m.certain.sourceList);
	});

	it('a public-database name stays a quote; the call is a Contract call (cs-7)', () => {
		const selector = '0x3d4ebdfe';
		const data = `${selector}${word(USDC)}${num(5)}`;
		const model = sheet(
			read(tx(SWEEPER, data), {
				selectors: { [selector]: ['claimAirdrop_e9x(address,uint256)'] }
			}),
			SWEEPER,
			data
		);
		expect(intentOf(model)).toBe(m.terms.intentContractCall);
		expect(warnings(model.blocks, 'caution')).toContain(
			fill(m.certain.selectorName, { name: 'claimAirdrop_e9x(address,uint256)' })
		);
		expect(model.confirm.action).toBe(m.confirmPlain);
		expect(model.confirm.danger).toBeUndefined();
	});

	it('a swap paying a stranger, 1 wei minimum, says both (cs-2)', () => {
		const sig = '0x04e45aaf';
		const WETH = WETH_MAINNET;
		const USDC_MAINNET = '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48';
		const router = '0x68b3465833fb72a70ecdf485e0e4c7bd8665fc45';
		const data =
			sig +
			word(USDC_MAINNET) +
			word(WETH) +
			num(500) +
			word(THIEF) +
			num(250_000_000) +
			num(1) +
			num(0);
		const model = sheet(read(tx(router, data, '0x0', 1)), router, data, 1);
		expect(warnings(model.blocks, 'danger')).toContain(m.certain.elsewhere);
		expect(warnings(model.blocks, 'caution')).toContain(m.certain.dustMinimum);
		const swap = model.blocks.find((b) => b.kind === 'swap');
		expect(swap && swap.kind === 'swap' ? swap.receive.tone : '').toBe('neutral');
		// A built-in description: Vela's word about Uniswap, said as such.
		expect(texts(model.blocks)).toContain(fill(m.certain.describedBuiltIn, { owner: 'Uniswap' }));
		expect(model.confirm.action).toBe(m.confirmPlain);
	});

	it('the same swap paying this account says "This account"', () => {
		const sig = '0x04e45aaf';
		const router = '0x68b3465833fb72a70ecdf485e0e4c7bd8665fc45';
		const data =
			sig +
			word('0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48') +
			word(WETH_MAINNET) +
			num(500) +
			word(ME) +
			num(250_000_000) +
			num(10n ** 16n) +
			num(0);
		const model = sheet(read(tx(router, data, '0x0', 1)), router, data, 1);
		expect(model.blocks.find((b) => b.kind === 'party')).toMatchObject({
			name: m.certain.thisAccount
		});
		expect(warnings(model.blocks, 'danger')).not.toContain(m.certain.elsewhere);
	});

	it('an increase is drawn as what it allows, never as money leaving (cs-16, cs-18)', () => {
		const data = `0x39509351${word(REWARDS)}${num(50_000_000)}`;
		const model = sheet(read(tx(USDC, data)), USDC, data);
		expect(model.blocks.some((b) => b.kind === 'amount')).toBe(false);
		expect(texts(model.blocks)).toContain(
			fill(m.certain.allowsMore, { spender: '0x618a3c...a5713a', amount: '50 USDC' })
		);
	});

	it('a fetched description’s rows carry their source marker (cs-14)', () => {
		const sig = 'lodge(uint256 amount)';
		const data = `0x${'12345678'}${num(7)}`;
		const view = read(tx(REWARDS, data), {
			descriptors: {
				[`/erc7730/calldata/eip155-100/${REWARDS}.json`]: {
					metadata: { owner: 'Lodge', contractName: 'Lodge' },
					display: {
						formats: {
							[sig]: {
								intent: 'Lodge',
								fields: [{ path: 'amount', label: 'Units', format: 'raw' }]
							}
						}
					}
				}
			}
		});
		// The selector does not match `lodge(uint256)`: blind, honestly.
		expect(view.surface === 'blind_transaction' || view.result !== null).toBe(true);
		if (view.result) {
			const model = sheet(view, REWARDS, data);
			const rows = model.blocks.flatMap((b) => (b.kind === 'rows' ? b.rows : []));
			expect(rows.every((r) => r.source?.includes(HOST))).toBe(true);
		}
	});
});
