<script lang="ts">
	import { browser } from '$app/environment';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import Seo from '$lib/components/Seo.svelte';
	import SiteHeader from '$lib/components/SiteHeader.svelte';
	import SiteFooter from '$lib/components/SiteFooter.svelte';
	import {
		CONTRACT_ADDRESS,
		LEGACY_CONTRACT_ADDRESS,
		makeChainClient,
		makeGnosisClient
	} from '$lib/chain';
	import {
		fetchWalletPage,
		type RegistrySource,
		type WalletRecord,
		type MemberPasskey
	} from '$lib/registry';
	import {
		DEFAULT_MULTISIG_CHAIN,
		GENESIS_REGISTRY_ADDRESS,
		MULTISIG_CHAINS,
		multisigChain,
		readMultisigChain,
		type MultisigChainKey
	} from '$lib/registry-chains';
	import {
		blockTimes,
		fetchMultisigPage,
		hasCode,
		isRecordedOn,
		knownContract,
		registryDeployment,
		type Deployment,
		type MultisigRecord
	} from '$lib/multisig';
	import { cleanUntrusted } from '$lib/untrusted-text';
	import {
		parseAttestation,
		resolveAuthenticator,
		fallbackLabel,
		securityChips,
		type AttestationSignals,
		type AuthenticatorLabel
	} from '$lib/authenticators';
	import type { PublicClient } from 'viem';

	const SIZE_OPTIONS = [20, 50, 100] as const;
	const DEFAULT_SIZE = 100;

	/** Two categories: Vela (passkey) accounts — the default — and multisig
	 *  accounts from the Safe Genesis Registry. */
	type Category = 'vela' | 'multisig';
	const category = $derived<Category>(
		page.url.searchParams.get('category') === 'multisig' ? 'multisig' : 'vela'
	);
	/** Both categories switch by chain (`?chain=`). Vela accounts: the current
	 *  possession-proven registry on Gnosis, where every wallet is recorded, or
	 *  its copy on Ethereum (same contract, same address), where a wallet's
	 *  "Back up the account record on Ethereum" writes. Multisig accounts: the
	 *  Safe Genesis Registry on Gnosis, Ethereum, or the labelled testnet. */
	const requestedChain = $derived(readMultisigChain(page.url.searchParams.get('chain')));
	/** Vela accounts have no testnet source. */
	const VELA_CHAINS = MULTISIG_CHAINS.filter((c) => !c.testnet);
	const chainKey = $derived<MultisigChainKey>(
		category === 'vela' && multisigChain(requestedChain).testnet
			? DEFAULT_MULTISIG_CHAIN
			: requestedChain
	);
	const chain = $derived(multisigChain(chainKey));
	/** The legacy server-signed index (Gnosis only), for pre-migration wallets.
	 *  Out of the main switch; reached from a link under the list, and old
	 *  `?source=legacy` links still open it. */
	const source = $derived<RegistrySource>(
		category === 'vela' && page.url.searchParams.get('source') === 'legacy' ? 'legacy' : 'v2'
	);
	const contractUrl = $derived(
		source === 'legacy'
			? `https://gnosisscan.io/address/${LEGACY_CONTRACT_ADDRESS}`
			: `${chain.explorer}/address/${CONTRACT_ADDRESS}`
	);
	const genesisUrl = $derived(`${chain.explorer}/address/${GENESIS_REGISTRY_ADDRESS}`);

	// --- Live data ---
	let client: PublicClient | null = null;
	let records = $state<WalletRecord[]>([]);
	/** The (page, desc, size) the currently shown records belong to — so ordinals
	 *  always match the visible rows, even while a newer page is still loading. */
	let loaded = $state<{ page: number; desc: boolean; size: number } | null>(null);
	let total = $state<number | null>(null);
	let loading = $state(true);
	let errorMsg = $state<string | null>(null);
	let expandedId = $state<string | null>(null);
	let copiedKey = $state<string | null>(null);
	let reqId = 0;

	/** AAGUID (lowercase) → resolved authenticator label, filled in as the
	 *  AAGUID Explorer answers. Reads degrade to a generic label. */
	let authLabels = $state<Record<string, AuthenticatorLabel>>({});

	/** Kick off (cached) AAGUID lookups for every passkey on the loaded page. */
	function resolveLabels(rows: WalletRecord[]) {
		const seen = new Set<string>();
		for (const r of rows) {
			for (const m of r.members ?? []) {
				const att = parseAttestation(m.attestation);
				if (!att?.aaguid) continue;
				const key = att.aaguid.toLowerCase();
				if (seen.has(key) || authLabels[key]) continue;
				seen.add(key);
				resolveAuthenticator(att.aaguid, m.authenticatorAttachment, m.transports).then((label) => {
					authLabels = { ...authLabels, [key]: label };
				});
			}
		}
	}

	/** The attestation signals + best-known authenticator label for one passkey. */
	function memberAuth(m: MemberPasskey): {
		att: AttestationSignals | null;
		label: AuthenticatorLabel;
	} {
		const att = parseAttestation(m.attestation);
		if (att?.aaguid) {
			const known = authLabels[att.aaguid.toLowerCase()];
			if (known) return { att, label: known };
			// Awaiting the lookup: show the AAGUID itself, not a wrong guess.
			return {
				att,
				label: { name: att.aaguid, iconUrl: null, iconUrlDark: null, glyph: '🔐', known: false }
			};
		}
		return { att, label: fallbackLabel(m.authenticatorAttachment, m.transports) };
	}

	// --- URL-driven state (shareable, back-button friendly) ---
	const currentPage = $derived(readPage(page.url.searchParams.get('page')));
	const desc = $derived(page.url.searchParams.get('order') !== 'oldest');
	const pageSize = $derived(readSize(page.url.searchParams.get('size')));
	const pageCount = $derived(total === null ? null : Math.max(1, Math.ceil(total / pageSize)));
	const skeletonRows = $derived(Array.from({ length: pageSize }, (_, i) => i));

	function readPage(v: string | null): number {
		const n = Number.parseInt(v ?? '1', 10);
		return Number.isFinite(n) && n >= 1 ? n : 1;
	}
	function readSize(v: string | null): number {
		const n = Number.parseInt(v ?? '', 10);
		return (SIZE_OPTIONS as readonly number[]).includes(n) ? n : DEFAULT_SIZE;
	}

	function ensureClient(): PublicClient {
		if (!client) client = makeGnosisClient();
		return client;
	}

	// --- Chain reads shared by both categories ---
	/** Which registry a deployment check is about: the passkey registry (Vela
	 *  accounts) or the Safe Genesis Registry (multisig accounts). */
	type Reg = 'pk' | 'ms';
	const REG_ADDRESS: Record<Reg, `0x${string}`> = {
		pk: CONTRACT_ADDRESS,
		ms: GENESIS_REGISTRY_ADDRESS
	};
	/** The selected registry has no code on the selected chain: nothing to list,
	 *  which is NOT the same as "no accounts". */
	let notDeployed = $state(false);
	/** `${reg}:${chain}` → is that registry there? Filled as the checks answer;
	 *  'error' means the chain could not be asked. */
	let deployments = $state<Record<string, Deployment | 'error'>>({});

	// Caches, not state: what renders is `deployments` above.
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	const chainClients = new Map<MultisigChainKey, PublicClient>();
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	const deploymentChecks = new Map<string, Promise<Deployment>>();

	function clientFor(key: MultisigChainKey): PublicClient {
		let c = chainClients.get(key);
		if (!c) {
			c = makeChainClient(key);
			chainClients.set(key, c);
		}
		return c;
	}

	/** Cached once answered; a failed check is forgotten so a retry asks again. */
	function deploymentOf(reg: Reg, key: MultisigChainKey): Promise<Deployment> {
		const id = `${reg}:${key}`;
		let check = deploymentChecks.get(id);
		if (!check) {
			check = registryDeployment(clientFor(key), REG_ADDRESS[reg]);
			deploymentChecks.set(id, check);
			check.then(
				(d) => (deployments = { ...deployments, [id]: d }),
				() => {
					deploymentChecks.delete(id);
					deployments = { ...deployments, [id]: 'error' };
				}
			);
		}
		return check;
	}

	// --- Multisig accounts (Safe Genesis Registry) ---
	let msRecords = $state<MultisigRecord[]>([]);
	/** The chain the shown rows were read from (links follow it, not the URL,
	 *  while a newer chain is still loading). */
	let msChain = $state<MultisigChainKey>(DEFAULT_MULTISIG_CHAIN);
	/** `${chain}:${block}` → unix ms, filled after the rows render. */
	let msTimes = $state<Record<string, number>>({});
	type Elsewhere = 'checking' | 'recorded' | 'not-recorded' | 'no-registry' | 'error';
	/** `${chain}:${safe}` → what each other chain's registry says about it. */
	let elsewhere = $state<Record<string, Partial<Record<MultisigChainKey, Elsewhere>>>>({});
	/** `${chain}:${safe}` → is a contract deployed at the Safe's address there? */
	let deployedHere = $state<Record<string, 'checking' | 'yes' | 'no' | 'error'>>({});

	async function fillTimes(key: MultisigChainKey, rows: MultisigRecord[]) {
		const missing = rows.map((r) => r.recordedAt).filter((b) => !(`${key}:${b}` in msTimes));
		if (missing.length === 0) return;
		const times = await blockTimes(clientFor(key), missing);
		const next = { ...msTimes };
		for (const [b, ms] of times) next[`${key}:${b}`] = ms;
		msTimes = next;
	}

	/** On first expand: which other chains hold this Safe's record, and is the
	 *  Safe deployed on the chain it is listed from. */
	async function checkSafe(key: MultisigChainKey, safe: `0x${string}`) {
		const id = `${key}:${safe}`;
		if (elsewhere[id]) return;
		const others = MULTISIG_CHAINS.filter((c) => c.key !== key);
		elsewhere[id] = Object.fromEntries(others.map((c) => [c.key, 'checking']));
		deployedHere[id] = 'checking';
		hasCode(clientFor(key), safe).then(
			(yes) => (deployedHere[id] = yes ? 'yes' : 'no'),
			() => (deployedHere[id] = 'error')
		);
		await Promise.all(
			others.map(async (c) => {
				let state: Elsewhere;
				try {
					state =
						(await deploymentOf('ms', c.key)) === 'not-deployed'
							? 'no-registry'
							: (await isRecordedOn(clientFor(c.key), safe))
								? 'recorded'
								: 'not-recorded';
				} catch {
					state = 'error';
				}
				elsewhere[id] = { ...elsewhere[id], [c.key]: state };
			})
		);
	}

	async function load(p: number, d: boolean, size: number) {
		const id = ++reqId;
		loading = true;
		errorMsg = null;
		expandedId = null;
		const reg: Reg = category === 'multisig' ? 'ms' : 'pk';
		const key = chainKey;
		const label = source === 'legacy' ? 'Gnosis' : chain.label;
		// Ask every chain of this category up front: the switcher says which
		// have no registry yet.
		for (const c of category === 'multisig' ? MULTISIG_CHAINS : VELA_CHAINS) {
			deploymentOf(reg, c.key).catch(() => {});
		}
		try {
			// The legacy index is Gnosis-only and long deployed; everything else
			// is asked "is the contract there?" before any list is read.
			if (source !== 'legacy' && (await deploymentOf(reg, key)) === 'not-deployed') {
				if (id !== reqId) return;
				notDeployed = true;
				total = null;
				records = [];
				msRecords = [];
				return;
			}
			if (id !== reqId) return;
			if (reg === 'ms') {
				const res = await fetchMultisigPage(clientFor(key), p, size, d);
				if (id !== reqId) return;
				total = res.total;
				msRecords = res.records;
				msChain = key;
				fillTimes(key, res.records).catch(() => {});
			} else {
				const c = source === 'v2' && key !== 'gnosis' ? clientFor(key) : ensureClient();
				const res = await fetchWalletPage(c, p, size, d, source);
				if (id !== reqId) return; // a newer request superseded this one
				total = res.total;
				records = res.records;
				resolveLabels(res.records);
			}
			notDeployed = false;
			loaded = { page: p, desc: d, size };
		} catch (e) {
			if (id !== reqId) return;
			console.error('registry read failed', e);
			errorMsg = `Couldn’t reach ${label}. Please try again.`;
			records = [];
			msRecords = [];
		} finally {
			if (id === reqId) loading = false;
		}
	}

	/** What the shown list belongs to; a change drops the old rows instead of
	 *  dimming them under the new heading. */
	let shownKind = '';
	$effect(() => {
		const p = currentPage;
		const d = desc;
		const size = pageSize;
		const kind = `${category}:${source}:${chainKey}`;
		total = null; // the count belongs to a source; clear it while switching
		if (kind !== shownKind) {
			shownKind = kind;
			records = [];
			msRecords = [];
			notDeployed = false;
		}
		if (browser) load(p, d, size);
	});

	// --- Navigation (writes to the URL; the effect above reacts) ---
	/** Query string for a category + chain (or the legacy index) + page + order +
	 *  size. All values are app-controlled, so no escaping is needed. Empty
	 *  (clean URL) for the defaults (Vela accounts / Gnosis / newest / page 1 / 100). */
	function queryFor(
		p: number,
		newest: boolean,
		size: number,
		to: { cat?: Category; src?: RegistrySource; ch?: MultisigChainKey } = {}
	): string {
		const cat = to.cat ?? category;
		const src = cat === 'vela' ? (to.src ?? source) : 'v2';
		let ch = to.ch ?? chainKey;
		// A chain the category has no source on falls back to the default.
		if (cat === 'vela' && multisigChain(ch).testnet) ch = DEFAULT_MULTISIG_CHAIN;
		const parts: string[] = [];
		if (cat === 'multisig') parts.push('category=multisig');
		if (src === 'legacy') parts.push('source=legacy');
		else if (ch !== DEFAULT_MULTISIG_CHAIN) parts.push(`chain=${ch}`);
		if (!newest) parts.push('order=oldest');
		if (size !== DEFAULT_SIZE) parts.push(`size=${size}`);
		if (p > 1) parts.push(`page=${p}`);
		return parts.length ? `?${parts.join('&')}` : '';
	}
	function setCategory(cat: Category) {
		// Keep the chain across categories where both have it.
		navTo(queryFor(1, desc, pageSize, { cat, src: 'v2' }));
	}
	function setChain(ch: MultisigChainKey) {
		navTo(queryFor(1, desc, pageSize, { src: 'v2', ch }));
	}
	function navTo(qs: string) {
		// resolve() is applied to the route; the lint rule just can't trace it
		// through the concatenated (app-controlled) query string.
		// eslint-disable-next-line svelte/no-navigation-without-resolve
		goto(`${resolve('/registry')}${qs}`, { keepFocus: true, noScroll: true });
	}
	function gotoPage(p: number) {
		const clamped = pageCount ? Math.min(Math.max(1, p), pageCount) : Math.max(1, p);
		navTo(queryFor(clamped, desc, pageSize));
	}
	function setOrder(newest: boolean) {
		navTo(queryFor(1, newest, pageSize)); // a new ordering starts from page 1
	}
	function setSize(size: number) {
		navTo(queryFor(1, desc, size)); // a new page size starts from page 1
	}
	function refresh() {
		client = null; // rebuild the client
		// Ask the chain again from scratch, including whether the registry is there.
		chainClients.delete(chainKey);
		deploymentChecks.delete(`${category === 'multisig' ? 'ms' : 'pk'}:${chainKey}`);
		load(currentPage, desc, pageSize);
	}

	// --- Helpers ---
	/** Stable, collision-proof row identity. walletRef+credentialId is unique on an
	 *  honest chain; the index also guards against a hostile RPC returning duplicate
	 *  records, which would otherwise crash the keyed #each. */
	function rowKey(r: WalletRecord, i: number): string {
		const id = r.source === 'legacy' ? `${r.walletRef}:${r.credentialId}` : `unit:${r.unitId}`;
		return `${r.source}:${id}:${i}`;
	}
	function toggle(id: string) {
		expandedId = expandedId === id ? null : id;
	}
	async function copy(text: string, key: string) {
		try {
			await navigator.clipboard.writeText(text);
			copiedKey = key;
			setTimeout(() => {
				if (copiedKey === key) copiedKey = null;
			}, 1200);
		} catch {
			/* clipboard unavailable */
		}
	}
	function ordinal(i: number): number | null {
		if (total === null || !loaded) return null;
		// Anchor to the page/size the shown records belong to, not the URL — otherwise
		// a still-loading page would relabel the previous page's rows.
		const pos = (loaded.page - 1) * loaded.size + i;
		return loaded.desc ? total - pos : pos + 1;
	}
	function displayName(n: string): string {
		// Drop bidi controls, zero-width chars and the BOM so an on-chain name can't
		// visually reverse or hide the row label (see untrusted-text.ts).
		const t = cleanUntrusted(n).text;
		return t.length ? t : 'Unnamed wallet';
	}
	function shortAddr(a: string): string {
		return `${a.slice(0, 6)}…${a.slice(-4)}`;
	}

	// --- Multisig helpers ---
	function msRowKey(r: MultisigRecord, i: number): string {
		return `ms:${msChain}:${r.index}:${r.safe}:${i}`;
	}
	function toggleMs(rid: string, r: MultisigRecord) {
		toggle(rid);
		if (expandedId === rid) checkSafe(msChain, r.safe);
	}
	function msName(r: MultisigRecord): string {
		return r.name.text.length ? r.name.text : 'Unnamed multisig';
	}
	function msSetup(r: MultisigRecord): string {
		return r.setup.ok ? `${r.setup.threshold} of ${r.setup.owners.length}` : 'setup not decoded';
	}
	function addrUrl(key: MultisigChainKey, a: string): string {
		return `${multisigChain(key).explorer}/address/${a}`;
	}
	function blockUrl(key: MultisigChainKey, b: bigint): string {
		return `${multisigChain(key).explorer}/block/${b}`;
	}
	function msTime(r: MultisigRecord): number | null {
		return msTimes[`${msChain}:${r.recordedAt}`] ?? null;
	}
	const ZERO_ADDR = '0x0000000000000000000000000000000000000000';
	/** A short label for the chain switcher: the testnet is always marked. */
	function chainStatus(key: MultisigChainKey): string {
		const d = deployments[`${category === 'multisig' ? 'ms' : 'pk'}:${key}`];
		return d === 'not-deployed' ? 'not deployed yet' : '';
	}
	const ELSEWHERE_TEXT: Record<Elsewhere, string> = {
		checking: 'checking…',
		recorded: 'recorded',
		'not-recorded': 'not recorded',
		'no-registry': 'registry not deployed yet',
		error: 'couldn’t check'
	};
	const dateFmt = new Intl.DateTimeFormat('en-US', {
		year: 'numeric',
		month: 'short',
		day: 'numeric'
	});
	const timeFmt = new Intl.DateTimeFormat('en-US', { dateStyle: 'medium', timeStyle: 'short' });

	const rowCount = $derived(category === 'multisig' ? msRecords.length : records.length);
	const statLabel = $derived(
		category === 'multisig'
			? `multisig accounts recorded on ${chain.label}`
			: source === 'legacy'
				? 'wallets in the legacy index'
				: chainKey === 'gnosis'
					? 'wallets created on-chain'
					: `wallets backed up to ${chain.label}`
	);
	/** The one small line under the chain switcher. */
	const sourceNote = $derived(
		category === 'multisig'
			? chain.testnet
				? `${chain.label} is a testnet: these are test records, not real accounts.`
				: 'Same registry address on every chain, each with its own records.'
			: source === 'legacy'
				? 'Legacy index on Gnosis — wallets created before the move.'
				: chainKey === 'gnosis'
					? 'Every wallet is recorded here when it is created.'
					: 'Copies made from the wallet with “Back up the account record on Ethereum”.'
	);
</script>

<Seo
	title="On-chain wallet registry"
	description="Every Vela wallet's passkey public keys, and every multisig setup, recorded on-chain. Browse them live — read straight from the contracts, from your browser."
	canonical="/registry"
/>

<SiteHeader />

<main class="registry">
	<header class="reg-head">
		<a class="back" href={resolve('/')}>← Home</a>
		<h1>Wallet registry</h1>
		<div class="seg category-seg" role="group" aria-label="Account type">
			<button
				class:active={category === 'vela'}
				aria-pressed={category === 'vela'}
				onclick={() => setCategory('vela')}>Vela accounts</button
			>
			<button
				class:active={category === 'multisig'}
				aria-pressed={category === 'multisig'}
				onclick={() => setCategory('multisig')}>Multisig accounts</button
			>
		</div>
		<!-- eslint-disable svelte/no-navigation-without-resolve -- block explorers: another origin -->
		{#if category === 'vela'}
			<p class="lede">
				Every Vela wallet's passkey public keys are recorded on Gnosis, each entry signed by the
				passkey itself — no server can forge one. Your wallet doesn't depend on it. This page reads
				the <a href={contractUrl} target="_blank" rel="noopener">contracts</a> straight from your browser.
				Don't trust us — verify.
			</p>
		{:else}
			<p class="lede">
				Multisig setups are recorded so any member's device can find them. Read live from the
				<a href={genesisUrl} target="_blank" rel="noopener">registry contract</a>.
			</p>
		{/if}
		<!-- eslint-enable svelte/no-navigation-without-resolve -->
		<div class="reg-stat" aria-live="polite">
			<span class="live-dot" class:on={total !== null && !errorMsg}></span>
			{#if notDeployed}
				<strong class="stat-number">—</strong>
				<span>Not deployed on {chain.label} yet</span>
			{:else}
				<strong class="stat-number">{total === null ? '—' : total.toLocaleString()}</strong>
				<span>{statLabel}</span>
			{/if}
		</div>
		<div class="seg source-seg chain-seg" role="group" aria-label="Chain">
			{#each category === 'multisig' ? MULTISIG_CHAINS : VELA_CHAINS as c (c.key)}
				{@const on = source === 'v2' && chainKey === c.key}
				<button class:active={on} aria-pressed={on} onclick={() => setChain(c.key)}>
					{c.label}{#if c.testnet}<span class="tag">testnet</span>{/if}{#if chainStatus(c.key)}<span
							class="seg-sub">{chainStatus(c.key)}</span
						>{/if}
				</button>
			{/each}
		</div>
		<p class="source-note">{sourceNote}</p>
	</header>

	<div class="toolbar">
		<div class="controls">
			<div class="seg" role="group" aria-label="Sort order">
				<button class:active={desc} aria-pressed={desc} onclick={() => setOrder(true)}
					>Newest</button
				>
				<button class:active={!desc} aria-pressed={!desc} onclick={() => setOrder(false)}
					>Oldest</button
				>
			</div>
			<div class="seg" role="group" aria-label="Wallets per page">
				<span class="seg-label">Per page</span>
				{#each SIZE_OPTIONS as s (s)}
					<button
						class:active={pageSize === s}
						aria-pressed={pageSize === s}
						aria-label={`${s} per page`}
						onclick={() => setSize(s)}>{s}</button
					>
				{/each}
			</div>
		</div>
		<button class="refresh" onclick={refresh} disabled={loading} aria-label="Reload from chain">
			<span class="ref-ico" class:spin={loading}>↻</span> Refresh
		</button>
	</div>

	{#if errorMsg && rowCount === 0}
		<div class="card error" role="alert">
			<p>{errorMsg}</p>
			<button class="retry" onclick={refresh}>Try again</button>
		</div>
		<!-- eslint-disable svelte/no-navigation-without-resolve -- block explorers: another origin -->
	{:else if notDeployed && !loading}
		{@const testnets = MULTISIG_CHAINS.filter(
			(c) =>
				category === 'multisig' &&
				c.testnet &&
				c.key !== chainKey &&
				deployments[`ms:${c.key}`] === 'deployed'
		)}
		<div class="card notice" role="status">
			<p class="notice-title">Not deployed on {chain.label} yet</p>
			<p>
				The {category === 'multisig' ? 'multisig' : 'passkey'} registry (<a
					class="mono"
					href={category === 'multisig' ? genesisUrl : contractUrl}
					target="_blank"
					rel="noopener"
					>{shortAddr(category === 'multisig' ? GENESIS_REGISTRY_ADDRESS : CONTRACT_ADDRESS)}</a
				>) has no contract on {chain.label} yet, so there is nothing to list here.
			</p>
			{#each testnets as t (t.key)}
				<button class="retry" onclick={() => setChain(t.key)}>See the {t.label} records</button>
			{/each}
		</div>
	{:else}
		<ol class="rows" aria-busy={loading}>
			{#if loading && rowCount === 0}
				{#each skeletonRows as i (i)}
					<li class="row skeleton"><span class="sk name"></span><span class="sk date"></span></li>
				{/each}
			{:else if category === 'multisig'}
				{#each msRecords as r, i (msRowKey(r, i))}
					{@const rid = msRowKey(r, i)}
					{@const sid = `${msChain}:${r.safe}`}
					{@const when = msTime(r)}
					<li class="row" class:open={expandedId === rid}>
						<button
							class="row-main"
							aria-expanded={expandedId === rid}
							onclick={() => toggleMs(rid, r)}
						>
							<span class="ord">#{r.index + 1}</span>
							<span class="who">
								<span class="name">{msName(r)}</span>
								<span class="sub">
									<span class="addr">{shortAddr(r.safe)}</span>
									<span class="chip" class:warn={!r.setup.ok}>{msSetup(r)}</span>
									{#if !r.name.utf8}<span class="chip warn">name not UTF-8</span>{/if}
									{#if r.name.hadHidden}<span class="chip warn">hidden characters</span>{/if}
									{#if r.selfNamed}<span class="chip sync">named by the Safe</span>{/if}
								</span>
							</span>
							{#if when !== null}
								<time datetime={new Date(when).toISOString()}>{dateFmt.format(when)}</time>
							{:else}
								<span class="block-no">block {r.recordedAt}</span>
							{/if}
							<svg
								class="chev"
								width="16"
								height="16"
								viewBox="0 0 24 24"
								fill="none"
								aria-hidden="true"
							>
								<path
									d="M6 9l6 6 6-6"
									stroke="currentColor"
									stroke-width="2"
									stroke-linecap="round"
									stroke-linejoin="round"
								/>
							</svg>
						</button>

						{#if expandedId === rid}
							<dl class="details">
								<div class="detail">
									<dt>Safe</dt>
									<dd>
										<a
											href={addrUrl(msChain, r.safe)}
											target="_blank"
											rel="noopener"
											class="mono wrap">{r.safe} ↗</a
										>
										<button
											class="cp"
											aria-label="Copy Safe address"
											onclick={() => copy(r.safe, `${rid}:s`)}
										>
											{copiedKey === `${rid}:s` ? 'Copied' : 'Copy'}
										</button>
									</dd>
								</div>
								<div class="detail">
									<dt>Name</dt>
									<dd class="stack">
										<span class="ms-name">{msName(r)}</span>
										<span class="fine">
											{#if r.renamedAt > 0n}
												Set by the Safe itself, at
												<a href={blockUrl(msChain, r.renamedAt)} target="_blank" rel="noopener"
													>block {r.renamedAt}</a
												>.
											{:else if r.selfNamed}
												Set by the Safe itself, which wrote the record.
											{:else}
												Set by whoever recorded it (<a
													class="mono"
													href={addrUrl(msChain, r.recorder)}
													target="_blank"
													rel="noopener">{shortAddr(r.recorder)}</a
												>) — not verified.
											{/if}
										</span>
										{#if !r.name.utf8}
											<span class="fine warn-text"
												>Not valid UTF-8. Raw bytes: <code class="mono wrap">{r.name.raw}</code
												></span
											>
										{/if}
										{#if r.name.hadHidden}
											<span class="fine warn-text"
												>Invisible or direction-changing characters were removed for display.</span
											>
										{/if}
									</dd>
								</div>
								{#if r.setup.ok}
									{@const s = r.setup}
									<div class="detail">
										<dt>Setup</dt>
										<dd class="stack">
											<span
												><strong>{s.threshold} of {s.owners.length}</strong>
												{s.owners.length === 1 ? 'owner' : 'owners'} must sign</span
											>
											{#if s.problems.length}
												<span class="fine warn-text"
													>Safe would reject this setup: {s.problems.join('; ')}.</span
												>
											{/if}
										</dd>
									</div>
									<div class="detail detail-block">
										<dt>Owners</dt>
										<dd>
											<ol class="owners">
												{#each s.owners as o, oi (`${o}:${oi}`)}
													<li>
														<a
															class="mono"
															href={addrUrl(msChain, o)}
															target="_blank"
															rel="noopener"
															title={o}
															aria-label={`Owner ${o}`}>{shortAddr(o)} ↗</a
														>
														<button
															class="cp"
															aria-label={`Copy owner ${oi + 1} address`}
															onclick={() => copy(o, `${rid}:o${oi}`)}
														>
															{copiedKey === `${rid}:o${oi}` ? 'Copied' : 'Copy'}
														</button>
													</li>
												{/each}
											</ol>
										</dd>
									</div>
									<div class="detail">
										<dt>Fallback handler</dt>
										<dd>
											{#if s.fallbackHandler === ZERO_ADDR}
												none
											{:else}
												<a
													class="mono"
													href={addrUrl(msChain, s.fallbackHandler)}
													target="_blank"
													rel="noopener"
													title={s.fallbackHandler}>{shortAddr(s.fallbackHandler)} ↗</a
												>
												{#if knownContract(s.fallbackHandler)}<span class="chip"
														>{knownContract(s.fallbackHandler)}</span
													>{/if}
											{/if}
										</dd>
									</div>
									{#if s.to !== ZERO_ADDR || s.data !== '0x'}
										<div class="detail">
											<dt>Setup hook</dt>
											<dd>
												<a
													class="mono"
													href={addrUrl(msChain, s.to)}
													target="_blank"
													rel="noopener"
													title={s.to}>{shortAddr(s.to)} ↗</a
												>
												<span>{(s.data.length - 2) / 2} bytes of call data</span>
											</dd>
										</div>
									{/if}
									{#if s.payment > 0n}
										<div class="detail">
											<dt>Setup payment</dt>
											<dd>
												{s.payment.toString()} (smallest unit) of {s.paymentToken === ZERO_ADDR
													? 'the native coin'
													: shortAddr(s.paymentToken)} to {shortAddr(s.paymentReceiver)}
											</dd>
										</div>
									{/if}
								{:else}
									<div class="detail">
										<dt>Setup</dt>
										<dd class="stack">
											<span class="warn-text"
												>Couldn't read owners and threshold: {r.setup.reason}.</span
											>
											<span class="fine"
												>Selector {r.setup.selector ?? '—'} · {r.setup.byteLength} bytes</span
											>
										</dd>
									</div>
								{/if}
								<div class="detail">
									<dt>Initializer</dt>
									<dd>
										<details class="raw">
											<summary>{(r.initializer.length - 2) / 2} bytes, raw</summary>
											<code class="mono wrap">{r.initializer}</code>
											<button
												class="cp"
												aria-label="Copy initializer"
												onclick={() => copy(r.initializer, `${rid}:i`)}
											>
												{copiedKey === `${rid}:i` ? 'Copied' : 'Copy'}
											</button>
										</details>
									</dd>
								</div>
								<div class="detail">
									<dt>Safe version</dt>
									<dd>
										{knownContract(r.singleton) ?? 'Unknown singleton'}
										<a
											class="mono"
											href={addrUrl(msChain, r.singleton)}
											target="_blank"
											rel="noopener"
											title={r.singleton}>{shortAddr(r.singleton)} ↗</a
										>
									</dd>
								</div>
								<div class="detail">
									<dt>Factory</dt>
									<dd>
										{knownContract(r.factory) ?? 'Unknown factory'}
										<a
											class="mono"
											href={addrUrl(msChain, r.factory)}
											target="_blank"
											rel="noopener"
											title={r.factory}>{shortAddr(r.factory)} ↗</a
										>
									</dd>
								</div>
								<div class="detail">
									<dt>Position</dt>
									<dd>
										#{r.index + 1} in {multisigChain(msChain).label}'s write order
										<code class="mono">safeAt({r.index})</code>
									</dd>
								</div>
								<div class="detail detail-block">
									<dt>Recorded on</dt>
									<dd>
										<ul class="chains">
											<li>
												<span class="chain-name">{multisigChain(msChain).label}</span>
												<span class="ok-text">recorded</span>
												{#if deployedHere[sid] === 'yes'}
													<span class="fine">· Safe deployed here</span>
												{:else if deployedHere[sid] === 'no'}
													<span class="fine">· Safe not deployed here yet</span>
												{:else if deployedHere[sid] === 'error'}
													<span class="fine">· couldn’t check deployment</span>
												{/if}
											</li>
											{#each MULTISIG_CHAINS.filter((c) => c.key !== msChain) as c (c.key)}
												{@const st = elsewhere[sid]?.[c.key] ?? 'checking'}
												<li>
													<span class="chain-name"
														>{c.label}{#if c.testnet}<span class="tag">testnet</span>{/if}</span
													>
													<span class:ok-text={st === 'recorded'} class:fine={st !== 'recorded'}
														>{ELSEWHERE_TEXT[st]}</span
													>
												</li>
											{/each}
										</ul>
									</dd>
								</div>
								<div class="detail">
									<dt>Recorded</dt>
									<dd>
										{#if when !== null}{timeFmt.format(when)} ·{/if}
										<a href={blockUrl(msChain, r.recordedAt)} target="_blank" rel="noopener"
											>block {r.recordedAt}</a
										>
										· by
										<a
											class="mono"
											href={addrUrl(msChain, r.recorder)}
											target="_blank"
											rel="noopener"
											title={r.recorder}>{shortAddr(r.recorder)}</a
										>
									</dd>
								</div>
							</dl>
						{/if}
					</li>
				{/each}
				<!-- eslint-enable svelte/no-navigation-without-resolve -->
			{:else}
				{#each records as r, i (rowKey(r, i))}
					{@const rid = rowKey(r, i)}
					<li class="row" class:open={expandedId === rid}>
						<button class="row-main" aria-expanded={expandedId === rid} onclick={() => toggle(rid)}>
							<span class="ord">#{ordinal(i) ?? ''}</span>
							<span class="who">
								<span class="name">{displayName(r.name)}</span>
								<span class="addr">{shortAddr(r.walletAddress)}</span>
							</span>
							<time datetime={new Date(r.createdAt).toISOString()}
								>{dateFmt.format(r.createdAt)}</time
							>
							<svg
								class="chev"
								width="16"
								height="16"
								viewBox="0 0 24 24"
								fill="none"
								aria-hidden="true"
							>
								<path
									d="M6 9l6 6 6-6"
									stroke="currentColor"
									stroke-width="2"
									stroke-linecap="round"
									stroke-linejoin="round"
								/>
							</svg>
						</button>

						{#if expandedId === rid}
							<dl class="details">
								<div class="detail">
									<dt>Wallet</dt>
									<dd>
										<a
											href={`https://blockscan.com/address/${r.walletAddress}`}
											target="_blank"
											rel="noopener"
											class="mono wrap">{r.walletAddress} ↗</a
										>
										<button
											class="cp"
											aria-label="Copy wallet address"
											onclick={() => copy(r.walletAddress, `${rid}:w`)}
										>
											{copiedKey === `${rid}:w` ? 'Copied' : 'Copy'}
										</button>
									</dd>
								</div>
								{#if r.source === 'legacy'}
									<div class="detail">
										<dt>Passkey credential</dt>
										<dd>
											<code class="mono">{r.credentialId}</code>
											<button
												class="cp"
												aria-label="Copy passkey credential ID"
												onclick={() => copy(r.credentialId ?? '', `${rid}:c`)}
											>
												{copiedKey === `${rid}:c` ? 'Copied' : 'Copy'}
											</button>
										</dd>
									</div>
								{:else}
									<div class="detail">
										<dt>Group</dt>
										<dd>
											#{r.unitId} · {r.memberCount} passkey{r.memberCount === 1
												? ''
												: 's'}{r.walletVersion ? ` · ${r.walletVersion}` : ''}
										</dd>
									</div>
								{/if}
								{#if r.source === 'legacy'}
									<div class="detail">
										<dt>P-256 public key</dt>
										<dd>
											<code class="mono wrap">{r.publicKey}</code>
											<button
												class="cp"
												aria-label="Copy P-256 public key"
												onclick={() => copy(r.publicKey, `${rid}:k`)}
											>
												{copiedKey === `${rid}:k` ? 'Copied' : 'Copy'}
											</button>
										</dd>
									</div>
								{:else}
									<div class="detail detail-block">
										<dt>{(r.members?.length ?? 0) === 1 ? 'Passkey' : 'Passkeys'}</dt>
										<dd>
											<ul class="pk-list">
												{#each r.members ?? [] as m, mi (m.entryId ?? mi)}
													{@const a = memberAuth(m)}
													<li class="pk">
														<div class="pk-head">
															<span class="auth-glyph" aria-hidden="true">
																{#if a.label.iconUrl}
																	<picture>
																		{#if a.label.iconUrlDark}
																			<source
																				srcset={a.label.iconUrlDark}
																				media="(prefers-color-scheme: dark)"
																			/>
																		{/if}
																		<img
																			src={a.label.iconUrl}
																			alt=""
																			width="20"
																			height="20"
																			loading="lazy"
																		/>
																	</picture>
																{:else}
																	<span class="glyph">{a.label.glyph}</span>
																{/if}
															</span>
															<span class="pk-name">{a.label.name}</span>
															{#if a.att}
																{#each securityChips(a.att) as chip (chip.label)}
																	<span class="chip {chip.tone}" title={chip.title}
																		>{chip.label}</span
																	>
																{/each}
															{/if}
														</div>
														<div class="pk-facts">
															<div class="pk-fact">
																<span class="pk-label">Public key</span>
																<span class="pk-value">
																	<code class="mono wrap">{m.publicKey}</code>
																	<button
																		class="cp"
																		aria-label="Copy public key"
																		onclick={() => copy(m.publicKey, `${rid}:k${mi}`)}
																	>
																		{copiedKey === `${rid}:k${mi}` ? 'Copied' : 'Copy'}
																	</button>
																</span>
															</div>
															{#if m.credentialId}
																<div class="pk-fact">
																	<span class="pk-label">Credential</span>
																	<span class="pk-value">
																		<code class="mono wrap">{m.credentialId}</code>
																		<button
																			class="cp"
																			aria-label="Copy credential id"
																			onclick={() => copy(m.credentialId, `${rid}:c${mi}`)}
																		>
																			{copiedKey === `${rid}:c${mi}` ? 'Copied' : 'Copy'}
																		</button>
																	</span>
																</div>
															{/if}
															{#if a.att?.aaguid}
																<div class="pk-fact">
																	<span class="pk-label">AAGUID</span>
																	<span class="pk-value"
																		><code class="mono">{a.att.aaguid}</code></span
																	>
																</div>
															{/if}
															{#if m.authenticatorAttachment || m.transports}
																<div class="pk-fact">
																	<span class="pk-label">Transport</span>
																	<span class="pk-value"
																		>{[m.authenticatorAttachment, m.transports]
																			.filter(Boolean)
																			.join(' · ')}</span
																	>
																</div>
															{/if}
															{#if m.attestation && m.attestation !== '0x'}
																<div class="pk-fact">
																	<span class="pk-label">Attestation</span>
																	<span class="pk-value"
																		><code class="mono wrap">{m.attestation}</code></span
																	>
																</div>
															{/if}
														</div>
													</li>
												{/each}
											</ul>
										</dd>
									</div>
								{/if}
								<div class="detail">
									<dt>Relying party</dt>
									<dd>{r.rpId}</dd>
								</div>
								<div class="detail">
									<dt>Registered</dt>
									<dd>{timeFmt.format(r.createdAt)}</dd>
								</div>
							</dl>
						{/if}
					</li>
				{/each}
			{/if}
		</ol>

		{#if rowCount === 0 && !loading && !errorMsg}
			{#if category === 'vela'}
				<p class="empty">No wallets on this page.</p>
			{:else if total === 0}
				<p class="empty">No multisig accounts recorded on {chain.label} yet.</p>
			{:else}
				<p class="empty">No multisig accounts on this page.</p>
			{/if}
		{/if}

		<nav class="pager" aria-label="Pagination">
			<button
				class="pg"
				onclick={() => gotoPage(currentPage - 1)}
				disabled={loading || currentPage <= 1}
			>
				‹ Prev
			</button>
			<span class="pageinfo">Page {currentPage}{pageCount ? ` of ${pageCount}` : ''}</span>
			<button
				class="pg"
				onclick={() => gotoPage(currentPage + 1)}
				disabled={loading || (pageCount !== null && currentPage >= pageCount)}
			>
				Next ›
			</button>
		</nav>
	{/if}

	{#if category === 'vela'}
		<!-- The legacy index, out of the main switch. A real link: old
		     ?source=legacy URLs keep working, and it opens in a new tab. -->
		<!-- eslint-disable svelte/no-navigation-without-resolve -- a resolve()d route plus a fixed query -->
		<p class="legacy-link">
			{#if source === 'legacy'}
				<a href={resolve('/registry')}>← Back to the current registry</a>
			{:else}
				<a href={`${resolve('/registry')}?source=legacy`}
					>Legacy index — wallets created before the move</a
				>
			{/if}
		</p>
		<!-- eslint-enable svelte/no-navigation-without-resolve -->
	{/if}
</main>

<SiteFooter />

<style>
	.registry {
		max-width: 840px;
		margin: 0 auto;
		padding: 48px 24px 80px;
	}

	/* ── Header ── */
	.back {
		display: inline-block;
		font-size: max(0.85rem, var(--floor-note));
		color: var(--text-muted);
		margin-bottom: 20px;
		transition: color 0.15s ease;
	}
	.back:hover {
		color: var(--text);
	}
	h1 {
		font-size: 2rem;
		letter-spacing: -0.02em;
		margin-bottom: 12px;
	}
	.lede {
		color: var(--text-secondary);
		line-height: 1.65;
		max-width: 640px;
		font-size: max(0.98rem, var(--floor-read));
	}
	.lede a {
		color: var(--link);
	}
	.lede a:hover {
		text-decoration: underline;
	}
	.reg-stat {
		display: flex;
		align-items: center;
		gap: 9px;
		margin-top: 22px;
		font-size: max(0.95rem, var(--floor-read));
		color: var(--text-secondary);
	}
	.stat-number {
		font-size: 1.15rem;
		font-weight: 700;
		font-variant-numeric: tabular-nums;
		color: var(--accent);
	}
	.live-dot {
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: var(--text-muted);
	}
	.live-dot.on {
		background: var(--green);
		box-shadow: 0 0 7px color-mix(in srgb, var(--green) 55%, transparent);
		animation: pulse-dot 2s ease-in-out infinite;
	}
	@keyframes pulse-dot {
		0%,
		100% {
			opacity: 1;
		}
		50% {
			opacity: 0.45;
		}
	}

	/* ── Category + source switchers ── */
	.category-seg {
		margin-bottom: 16px;
	}
	.source-seg {
		margin-top: 18px;
		flex-wrap: wrap;
		max-width: 100%;
	}
	/* Three chains on a phone: let the pill become a rounded box that wraps. */
	.chain-seg {
		border-radius: 18px;
	}
	.seg .tag,
	.chains .tag {
		margin-left: 6px;
		font-size: max(0.66rem, var(--floor-label));
		font-weight: 600;
		letter-spacing: 0.04em;
		text-transform: uppercase;
		color: var(--status-warn-text);
	}
	.seg-sub {
		display: block;
		font-size: max(0.68rem, var(--floor-label));
		font-weight: 400;
		color: var(--text-muted);
	}
	.legacy-link {
		margin-top: 28px;
		text-align: center;
		font-size: max(0.85rem, var(--floor-meta));
	}
	.legacy-link a {
		color: var(--text-muted);
		text-decoration: underline;
		text-underline-offset: 3px;
	}
	.legacy-link a:hover {
		color: var(--text);
	}
	.source-note {
		margin-top: 8px;
		font-size: max(0.82rem, var(--floor-meta));
		color: var(--text-muted);
	}

	/* ── Toolbar ── */
	.toolbar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		margin: 32px 0 4px;
	}
	.controls {
		display: flex;
		align-items: center;
		gap: 10px;
		flex-wrap: wrap;
	}
	.seg {
		display: inline-flex;
		align-items: center;
		background: var(--bg-raised);
		border: 1px solid var(--border);
		border-radius: 999px;
		padding: 3px;
	}
	.seg-label {
		font-size: 0.75rem;
		color: var(--text-muted);
		padding: 0 8px 0 10px;
	}
	.seg button {
		border: none;
		background: none;
		color: var(--text-secondary);
		font-size: max(0.85rem, var(--floor-meta));
		font-weight: 500;
		font-variant-numeric: tabular-nums;
		padding: 6px 14px;
		border-radius: 999px;
		cursor: pointer;
		transition:
			background 0.15s ease,
			color 0.15s ease;
	}
	.seg button.active {
		background: var(--bg-card);
		color: var(--text);
		box-shadow: var(--shadow-sm);
	}
	.refresh {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		background: none;
		border: 1px solid var(--border);
		color: var(--text-secondary);
		font-size: max(0.85rem, var(--floor-meta));
		padding: 7px 14px;
		border-radius: var(--radius-sm);
		cursor: pointer;
		transition:
			color 0.15s ease,
			border-color 0.15s ease;
	}
	.refresh:hover:not(:disabled) {
		color: var(--text);
		border-color: var(--border-strong);
	}
	.refresh:disabled {
		opacity: 0.5;
		cursor: default;
	}
	.ref-ico {
		display: inline-block;
	}
	.ref-ico.spin {
		animation: spin 0.9s linear infinite;
	}
	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	/* ── Rows ── */
	.rows {
		list-style: none;
		margin: 20px 0 0;
		border-top: 1px solid var(--border);
		transition: opacity 0.15s ease;
	}
	/* Dim the current page while the next one loads — keeps context, signals work. */
	.rows[aria-busy='true'] {
		opacity: 0.5;
	}
	.row {
		border-bottom: 1px solid var(--border);
	}
	.row-main {
		width: 100%;
		display: grid;
		grid-template-columns: auto 1fr auto auto;
		align-items: center;
		gap: 16px;
		background: none;
		border: none;
		cursor: pointer;
		text-align: left;
		padding: 16px 6px;
		color: inherit;
		transition: background 0.12s ease;
	}
	.row-main:hover {
		background: var(--bg-raised);
	}
	.ord {
		font-size: max(0.8rem, var(--floor-meta));
		font-variant-numeric: tabular-nums;
		color: var(--text-muted);
		min-width: 3ch;
	}
	.who {
		display: flex;
		flex-direction: column;
		gap: 3px;
		min-width: 0;
	}
	.name {
		font-weight: 600;
		font-size: 0.98rem;
		color: var(--text);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		/* Keep any residual bidi in an on-chain name from reordering the layout. */
		unicode-bidi: isolate;
	}
	.addr {
		font-family: var(--font-mono);
		font-size: max(0.8rem, var(--floor-meta));
		color: var(--text-muted);
	}
	time {
		font-size: max(0.85rem, var(--floor-meta));
		color: var(--text-secondary);
		white-space: nowrap;
	}
	.chev {
		color: var(--text-muted);
		transition: transform 0.18s ease;
	}
	.row.open .chev {
		transform: rotate(180deg);
	}

	/* ── Expanded details ── */
	.details {
		margin: 0;
		padding: 4px 6px 20px;
		display: grid;
		gap: 12px;
	}
	.detail {
		display: grid;
		grid-template-columns: 150px 1fr;
		gap: 12px;
		align-items: baseline;
	}
	.detail dt {
		font-size: max(0.8rem, var(--floor-meta));
		color: var(--text-muted);
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}
	.detail dd {
		margin: 0;
		display: flex;
		align-items: baseline;
		gap: 10px;
		flex-wrap: wrap;
		min-width: 0;
		font-size: max(0.9rem, var(--floor-note));
		color: var(--text-secondary);
	}
	.mono {
		font-family: var(--font-mono);
		font-size: max(0.82rem, var(--floor-meta));
	}
	.detail dd code.mono {
		color: var(--code-inline-text);
		background: var(--code-inline-bg);
		padding: 2px 6px;
		border-radius: 5px;
	}
	.detail dd a.mono {
		color: var(--link);
	}
	.detail dd a.mono:hover {
		text-decoration: underline;
	}
	.wrap {
		word-break: break-all;
		line-height: 1.5;
	}
	.cp {
		background: none;
		border: 1px solid var(--border);
		color: var(--text-muted);
		font-size: max(0.72rem, var(--floor-label));
		padding: 2px 9px;
		border-radius: 6px;
		cursor: pointer;
		transition:
			color 0.15s ease,
			border-color 0.15s ease;
	}
	.cp:hover {
		color: var(--text);
		border-color: var(--border-strong);
	}

	/* ── Per-passkey list (multi-key wallets) ── */
	.detail-block {
		align-items: start;
	}
	.pk-list {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: 10px;
		width: 100%;
		min-width: 0;
	}
	.pk {
		display: grid;
		gap: 12px;
		padding: 14px 16px;
		border: 1px solid var(--border);
		border-radius: 12px;
		background: var(--bg-sunken);
		min-width: 0;
	}
	.pk-head {
		display: flex;
		align-items: center;
		gap: 9px;
		flex-wrap: wrap;
		padding-bottom: 11px;
		border-bottom: 1px solid var(--border);
	}
	.auth-glyph {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 22px;
		height: 22px;
		flex: 0 0 22px;
	}
	.auth-glyph img {
		width: 100%;
		height: 100%;
		object-fit: contain;
		display: block;
	}
	.auth-glyph .glyph {
		font-size: 1.1rem;
		line-height: 1;
	}
	.pk-name {
		font-weight: 650;
		font-size: max(0.94rem, var(--floor-note));
		color: var(--text);
		margin-right: auto;
	}
	.chip {
		font-size: max(0.68rem, var(--floor-label));
		font-weight: 500;
		padding: 2px 9px;
		border-radius: 999px;
		border: 1px solid var(--border);
		color: var(--text-secondary);
		white-space: nowrap;
	}
	.chip.sync {
		color: #1f9e6b;
		border-color: rgba(31, 158, 107, 0.35);
		background: rgba(31, 158, 107, 0.08);
	}
	.chip.verify {
		color: #2b72e8;
		border-color: rgba(43, 114, 232, 0.35);
		background: rgba(43, 114, 232, 0.08);
	}
	.chip.bound {
		color: var(--text-muted);
	}
	.pk-facts {
		display: grid;
		gap: 8px;
	}
	.pk-fact {
		display: grid;
		grid-template-columns: 88px 1fr;
		gap: 12px;
		align-items: baseline;
	}
	.pk-label {
		font-size: max(0.7rem, var(--floor-label));
		text-transform: uppercase;
		letter-spacing: 0.04em;
		color: var(--text-tertiary);
		padding-top: 2px;
	}
	.pk-value {
		display: flex;
		align-items: baseline;
		gap: 8px;
		flex-wrap: wrap;
		min-width: 0;
		font-size: max(0.85rem, var(--floor-meta));
		color: var(--text-secondary);
	}
	.pk-value code.mono {
		color: var(--code-inline-text);
		background: var(--code-inline-bg);
		padding: 2px 6px;
		border-radius: 5px;
	}
	@media (max-width: 560px) {
		.pk-fact {
			grid-template-columns: 1fr;
			gap: 3px;
		}
	}

	/* ── Multisig rows ── */
	.sub {
		display: flex;
		align-items: center;
		gap: 6px;
		flex-wrap: wrap;
		min-width: 0;
	}
	.chip.warn {
		color: var(--status-warn-text);
		border-color: color-mix(in srgb, var(--status-warn) 40%, transparent);
		background: color-mix(in srgb, var(--status-warn) 9%, transparent);
	}
	.block-no {
		font-size: max(0.8rem, var(--floor-meta));
		font-variant-numeric: tabular-nums;
		color: var(--text-muted);
		white-space: nowrap;
	}
	.detail dd.stack {
		flex-direction: column;
		align-items: flex-start;
		gap: 4px;
	}
	.ms-name {
		color: var(--text);
		font-weight: 600;
		overflow-wrap: anywhere;
		unicode-bidi: isolate;
	}
	.fine {
		font-size: max(0.82rem, var(--floor-meta));
		color: var(--text-muted);
	}
	.fine a,
	.detail dd > a:not(.mono) {
		color: var(--link);
	}
	.detail dd a.mono,
	.owners a,
	.fine a.mono {
		color: var(--link);
	}
	.warn-text {
		color: var(--status-warn-text);
	}
	.ok-text {
		color: var(--green);
		font-weight: 500;
	}
	.owners,
	.chains {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: 6px;
		min-width: 0;
	}
	.owners li,
	.chains li {
		display: flex;
		align-items: baseline;
		gap: 8px;
		flex-wrap: wrap;
	}
	.chain-name {
		color: var(--text);
		font-weight: 500;
	}
	.raw summary {
		cursor: pointer;
		color: var(--text-secondary);
	}
	.raw[open] summary {
		margin-bottom: 8px;
	}
	.raw code {
		display: block;
		max-height: 12rem;
		overflow: auto;
		margin-bottom: 8px;
	}
	.card.notice {
		border: 1px solid var(--border-strong);
		background: var(--bg-raised);
		border-radius: var(--radius);
		padding: 24px;
		margin-top: 24px;
		color: var(--text-secondary);
		line-height: 1.6;
	}
	.card.notice .notice-title {
		color: var(--text);
		font-weight: 650;
		font-size: 1.05rem;
		margin-bottom: 6px;
	}
	.card.notice a {
		color: var(--link);
	}

	/* ── Skeleton ── */
	.row.skeleton {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 20px 6px;
	}
	.sk {
		height: 12px;
		border-radius: 6px;
		background: linear-gradient(90deg, var(--bg-raised), var(--bg-card), var(--bg-raised));
		background-size: 200% 100%;
		animation: shimmer 1.4s ease-in-out infinite;
	}
	.sk.name {
		width: 40%;
	}
	.sk.date {
		width: 68px;
	}
	@keyframes shimmer {
		0% {
			background-position: 200% 0;
		}
		100% {
			background-position: -200% 0;
		}
	}

	/* ── Error / empty ── */
	.card.error {
		border: 1px solid var(--border-strong);
		background: var(--bg-raised);
		border-radius: var(--radius);
		padding: 28px;
		text-align: center;
		margin-top: 24px;
		color: var(--text-secondary);
	}
	.retry,
	.card.error .retry {
		margin-top: 14px;
		background: var(--accent);
		color: var(--text-on-accent);
		border: none;
		padding: 9px 18px;
		border-radius: var(--radius-sm);
		font-weight: 600;
		cursor: pointer;
	}
	.retry:hover {
		background: var(--accent-hover);
	}
	.empty {
		text-align: center;
		color: var(--text-muted);
		padding: 40px 0;
	}

	/* ── Pager ── */
	.pager {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 18px;
		margin-top: 28px;
	}
	.pg {
		background: var(--bg-raised);
		border: 1px solid var(--border);
		color: var(--text);
		font-size: 0.88rem;
		padding: 9px 18px;
		border-radius: var(--radius-sm);
		cursor: pointer;
		transition:
			border-color 0.15s ease,
			background 0.15s ease;
	}
	.pg:hover:not(:disabled) {
		border-color: var(--border-strong);
		background: var(--bg-card);
	}
	.pg:disabled {
		opacity: 0.4;
		cursor: default;
	}
	.pageinfo {
		font-size: 0.88rem;
		color: var(--text-secondary);
		font-variant-numeric: tabular-nums;
	}

	@media (max-width: 560px) {
		h1 {
			font-size: 1.6rem;
		}
		.row-main {
			grid-template-columns: auto 1fr auto;
			gap: 10px;
		}
		.row-main time,
		.row-main .block-no {
			display: none;
		}
		.detail {
			grid-template-columns: 1fr;
			gap: 4px;
		}
	}
</style>
