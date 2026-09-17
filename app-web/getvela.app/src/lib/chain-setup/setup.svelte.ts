/**
 * The page's state, in two phases that never blur into each other.
 *
 * 1. **Check.** Take a chain id or an RPC URL, find endpoints that answer for
 *    that chain, run the wallet's admission bar, and give a verdict.
 * 2. **Set up** — only offered when the verdict says something is missing and
 *    can be fixed. Each step is one action with one status. Nothing here is
 *    optimistic: a step is "done" when the chain says the code is there.
 */
import {
	ARACHNID_PRESIGNED_TX,
	MULTICALL3_PRESIGNED_TX,
	create2Calldata,
	estimatedCreate2Gas
} from './deployers';
import {
	deployerExportText,
	forgetDeployer,
	getOrCreateDeployer,
	loadDeployer,
	sendFromDeployer,
	sweepDeployer,
	type DeployerKey
} from './deployer-wallet';
import {
	checkChain,
	checkContract,
	getBalance,
	getGasPrice,
	probeRpcs,
	sendRaw,
	waitForReceipt,
	type CheckResult,
	type RpcProbe
} from './rpc';
import { ARACHNID_PROXY, SAFE_SINGLETON_FACTORY, requiredContract } from './required-contracts';
import { loadIndex, search, type ChainHit } from './search';
import { estimatedFundingWei, plan as makePlan, type Plan, type Step } from './verdict';

const CHAIN_DATA = 'https://ethereum-data.getvela.app/chains/eip155-';

export type Phase = 'idle' | 'resolving' | 'checking' | 'done' | 'error';

export interface ChainIdentity {
	chainId: number;
	name: string | null;
	nativeSymbol: string;
	nativeDecimals: number;
	explorer: string | null;
	rpcs: string[];
}

export type StepStatus = 'idle' | 'waiting-funds' | 'sending' | 'confirming' | 'done' | 'failed';
export interface StepRun {
	status: StepStatus;
	hash?: `0x${string}`;
	error?: string;
}

interface ChainDataFile {
	name?: string;
	rpc?: string[];
	nativeCurrency?: { symbol?: string; decimals?: number };
	explorers?: { url?: string }[];
}

async function fetchChainData(chainId: number): Promise<ChainDataFile | null> {
	try {
		const res = await fetch(`${CHAIN_DATA}${chainId}.json`, {
			headers: { accept: 'application/json' }
		});
		if (!res.ok || !(res.headers.get('content-type') ?? '').includes('json')) return null;
		return (await res.json()) as ChainDataFile;
	} catch {
		return null;
	}
}

function looksLikeUrl(s: string): boolean {
	return /^https?:\/\//i.test(s);
}

/** A chain nobody but this machine can reach: the directory's name for its id is a coincidence. */
function isLocalUrl(s: string): boolean {
	try {
		const h = new URL(s).hostname;
		return (
			h === 'localhost' || h === '127.0.0.1' || h === '::1' || h === '[::1]' || h.endsWith('.local')
		);
	} catch {
		return false;
	}
}

export class ChainSetup {
	input = $state('');
	suggestions = $state<ChainHit[]>([]);
	phase = $state<Phase>('idle');
	error = $state<string | null>(null);

	chain = $state<ChainIdentity | null>(null);
	rpcs = $state<RpcProbe[]>([]);
	rpcUrl = $state('');
	result = $state<CheckResult | null>(null);
	plan = $derived<Plan | null>(
		this.result ? makePlan(this.result.contracts, this.result.p256) : null
	);

	gasPrice = $state<bigint | null>(null);
	/** Balances of the two keyless one-time senders, refreshed while a step waits on them. */
	senderBalances = $state<Record<string, bigint>>({});
	deployer = $state<DeployerKey | null>(null);
	deployerBalance = $state<bigint | null>(null);
	steps = $state<Record<string, StepRun>>({});
	/** Steps this page finished, in the order they finished. A done step leaves the
	 *  plan (the chain now has the contract) but must not leave the screen. */
	completed = $state<Step[]>([]);
	deployingAll = $state(false);
	sweepHash = $state<`0x${string}` | null>(null);

	private generation = 0;
	private poll: ReturnType<typeof setInterval> | null = null;
	private suggestTimer: ReturnType<typeof setTimeout> | null = null;

	/** Suggest chains from the directory as the person types — by id, name or gas-coin symbol. */
	suggest(query = this.input): void {
		if (this.suggestTimer) clearTimeout(this.suggestTimer);
		const q = query.trim();
		if (q.length < 2 || looksLikeUrl(q)) {
			this.suggestions = [];
			return;
		}
		this.suggestTimer = setTimeout(async () => {
			const index = await loadIndex();
			if (this.input.trim() !== q) return; // superseded by a later keystroke
			this.suggestions = search(index, q);
		}, 180);
	}

	pick(hit: ChainHit): void {
		this.suggestions = [];
		void this.start(String(hit.chainId));
	}

	// ── Phase 1: check ──────────────────────────────────────────────────────

	async start(query = this.input): Promise<void> {
		const q = query.trim();
		if (!q) return;
		const gen = ++this.generation;
		this.reset(false);
		this.input = q;
		this.suggestions = [];
		this.phase = 'resolving';

		try {
			let chainId: number;
			let rpcs: string[] = [];
			if (looksLikeUrl(q)) {
				const [probe] = await probeRpcs([q]);
				if (probe.chainId === null) throw new Error('rpc-unreachable');
				chainId = probe.chainId;
				rpcs = [q];
			} else if (/^\d+$/.test(q)) {
				chainId = Number(q);
			} else {
				throw new Error('not-a-chain');
			}
			const data = await fetchChainData(chainId);
			const fromData = (data?.rpc ?? []).filter(
				(u) => u.startsWith('https://') && !u.includes('${')
			);
			for (const u of fromData) if (!rpcs.includes(u)) rpcs.push(u);
			if (rpcs.length === 0) throw new Error('no-rpc');

			// A local RPC's chain id usually collides with some registered chain
			// (31337 is "GoChain Testnet" in the directory). Its name, explorer and
			// coin would all be confident wrong guesses; a dev chain is ETH-denominated
			// in every tool that ships one.
			const local = looksLikeUrl(q) && isLocalUrl(q);
			this.chain = {
				chainId,
				name: local ? null : (data?.name ?? null),
				nativeSymbol: local ? 'ETH' : (data?.nativeCurrency?.symbol ?? 'coin'),
				nativeDecimals: local ? 18 : (data?.nativeCurrency?.decimals ?? 18),
				explorer: local ? null : (data?.explorers?.[0]?.url?.replace(/\/$/, '') ?? null),
				rpcs
			};
			if (gen !== this.generation) return;

			this.rpcs = await probeRpcs(rpcs, chainId);
			const best = this.rpcs.find((p) => p.latencyMs !== null);
			if (!best) throw new Error('rpc-unreachable');
			this.rpcUrl = best.url;
			if (gen !== this.generation) return;

			await this.check(gen);
		} catch (e) {
			if (gen !== this.generation) return;
			this.phase = 'error';
			this.error = e instanceof Error ? e.message : 'unknown';
		}
	}

	private async check(gen = this.generation): Promise<void> {
		this.phase = 'checking';
		const [result, gasPrice] = await Promise.all([
			checkChain(this.rpcUrl),
			getGasPrice(this.rpcUrl).catch(() => null)
		]);
		if (gen !== this.generation) return;
		this.result = result;
		this.gasPrice = gasPrice;
		this.phase = 'done';
		// A returning visitor's key is theirs to see, funded or not.
		if (this.chain) this.deployer = loadDeployer(this.chain.chainId);
		await this.refreshBalances();
	}

	async recheck(): Promise<void> {
		if (!this.rpcUrl) return;
		await this.check(++this.generation);
	}

	async useRpc(url: string): Promise<void> {
		this.rpcUrl = url;
		await this.recheck();
	}

	// ── Phase 2: set up ─────────────────────────────────────────────────────

	stepRun(key: string): StepRun {
		return this.steps[key] ?? { status: 'idle' };
	}
	private setStep(key: string, run: StepRun): void {
		this.steps = { ...this.steps, [key]: run };
	}

	/** Re-read the sender and deployer balances; called on a timer while anything waits on money. */
	async refreshBalances(): Promise<void> {
		if (!this.rpcUrl || !this.plan) return;
		const senders = this.plan.steps.flatMap((s) =>
			s.kind === 'fund-and-broadcast' ? [s.deployer] : []
		);
		const reads = senders.map(
			async (a) => [a, await getBalance(this.rpcUrl, a).catch(() => 0n)] as const
		);
		const entries = await Promise.all(reads);
		this.senderBalances = Object.fromEntries(entries);
		if (this.deployer) {
			this.deployerBalance = await getBalance(this.rpcUrl, this.deployer.address).catch(() => null);
		}
	}

	watchBalances(on: boolean): void {
		if (this.poll) clearInterval(this.poll);
		this.poll = on ? setInterval(() => void this.refreshBalances(), 5_000) : null;
	}

	senderReady(step: Extract<Step, { kind: 'fund-and-broadcast' }>): boolean {
		return (this.senderBalances[step.deployer] ?? 0n) >= step.fundingWei;
	}

	/** Broadcast a keyless deployment once its sender holds the gas it names. */
	async broadcast(step: Extract<Step, { kind: 'fund-and-broadcast' }>): Promise<void> {
		const key = step.contract.key;
		if (!this.senderReady(step)) {
			this.setStep(key, { status: 'waiting-funds' });
			return;
		}
		this.setStep(key, { status: 'sending' });
		try {
			const raw = step.rawTx === 'arachnid' ? ARACHNID_PRESIGNED_TX : MULTICALL3_PRESIGNED_TX;
			const hash = await sendRaw(this.rpcUrl, raw);
			this.setStep(key, { status: 'confirming', hash });
			await this.settle(key, hash);
		} catch (e) {
			this.setStep(key, { status: 'failed', error: explain(e) });
		}
	}

	ensureDeployer(): DeployerKey {
		if (!this.chain) throw new Error('no chain');
		this.deployer = getOrCreateDeployer(this.chain.chainId);
		void this.refreshBalances();
		return this.deployer;
	}

	exportText(): string | null {
		return this.chain && this.deployer
			? deployerExportText(this.chain.chainId, this.deployer)
			: null;
	}

	/** What the whole runnable plan is likely to cost, in wei of the native coin. */
	fundingEstimate(): bigint | null {
		return this.plan && this.gasPrice !== null
			? estimatedFundingWei(this.plan, this.gasPrice)
			: null;
	}

	async deploy(step: Extract<Step, { kind: 'create2' }>): Promise<boolean> {
		const key = step.contract.key;
		if (!this.chain) return false;
		if (step.blockedBy) {
			this.setStep(key, { status: 'failed', error: 'blocked-by-factory' });
			return false;
		}
		const deployer = this.deployer ?? this.ensureDeployer();
		this.setStep(key, { status: 'sending' });
		try {
			const to = step.factory === 'arachnid' ? ARACHNID_PROXY : SAFE_SINGLETON_FACTORY;
			const hash = await sendFromDeployer(this.rpcUrl, this.chain.chainId, deployer, {
				to,
				data: create2Calldata(key),
				// The node's estimate wins when higher; this is what stands in when it
				// cannot answer, and it is the measured size-based figure, not a guess.
				gasFloor: estimatedCreate2Gas(key)
			});
			this.setStep(key, { status: 'confirming', hash });
			return await this.settle(key, hash);
		} catch (e) {
			this.setStep(key, { status: 'failed', error: explain(e) });
			return false;
		}
	}

	/** Every runnable CREATE2 step whose factory is present, in order; stops at the first failure. */
	async deployAll(): Promise<void> {
		if (!this.plan || this.deployingAll) return;
		this.deployingAll = true;
		try {
			for (const step of this.plan.runnable) {
				if (step.kind !== 'create2' || step.blockedBy) continue;
				if (this.stepRun(step.contract.key).status === 'done') continue;
				const ok = await this.deploy(step);
				if (!ok) break;
			}
		} finally {
			this.deployingAll = false;
			await this.refreshBalances();
		}
	}

	/** Wait for the receipt, then believe the CHAIN, not the receipt: re-read the code. */
	private async settle(key: string, hash: `0x${string}`): Promise<boolean> {
		const mined = await waitForReceipt(this.rpcUrl, hash);
		const status = await checkContract(this.rpcUrl, requiredContract(key));
		if (status.deployed) {
			this.setStep(key, { status: 'done', hash });
			const step = this.plan?.steps.find((s) => s.contract.key === key);
			if (step && !this.completed.some((s) => s.contract.key === key)) {
				this.completed = [...this.completed, step];
			}
			this.replaceStatus(status);
			return true;
		}
		this.setStep(key, {
			status: 'failed',
			hash,
			error: mined === false ? 'reverted' : mined === null ? 'not-mined' : 'no-code'
		});
		return false;
	}

	private replaceStatus(status: Awaited<ReturnType<typeof checkContract>>): void {
		if (!this.result) return;
		this.result = {
			...this.result,
			contracts: this.result.contracts.map((c) =>
				c.contract.key === status.contract.key ? status : c
			)
		};
	}

	async sweep(to: `0x${string}`): Promise<void> {
		if (!this.chain || !this.deployer) return;
		this.sweepHash = await sweepDeployer(this.rpcUrl, this.chain.chainId, this.deployer, to);
		await this.refreshBalances();
	}

	forgetKey(): void {
		if (!this.chain) return;
		forgetDeployer(this.chain.chainId);
		this.deployer = null;
		this.deployerBalance = null;
	}

	reset(clearInput = true): void {
		this.watchBalances(false);
		if (clearInput) this.input = '';
		this.phase = 'idle';
		this.error = null;
		this.chain = null;
		this.rpcs = [];
		this.rpcUrl = '';
		this.result = null;
		this.gasPrice = null;
		this.senderBalances = {};
		this.deployer = null;
		this.deployerBalance = null;
		this.steps = {};
		this.completed = [];
		this.deployingAll = false;
		this.sweepHash = null;
	}
}

/** Node errors, in the words a person can act on. Keys resolve in the page's copy. */
function explain(e: unknown): string {
	const msg = e instanceof Error ? e.message : String(e);
	const m = msg.toLowerCase();
	if (m.includes('insufficient funds')) return 'insufficient-funds';
	if (m.includes('already known') || m.includes('already imported')) return 'already-sent';
	if (m.includes('nonce')) return 'nonce';
	if (m.includes('replacement')) return 'already-sent';
	return msg.slice(0, 200);
}

export function formatCoin(wei: bigint, decimals = 18, digits = 4): string {
	const whole = wei / 10n ** BigInt(decimals);
	const frac = wei % 10n ** BigInt(decimals);
	const fracStr = frac.toString().padStart(decimals, '0').slice(0, digits).replace(/0+$/, '');
	return fracStr ? `${whole}.${fracStr}` : whole.toString();
}
