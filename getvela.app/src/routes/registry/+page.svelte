<script lang="ts">
	import { browser } from '$app/environment';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import Seo from '$lib/components/Seo.svelte';
	import SiteHeader from '$lib/components/SiteHeader.svelte';
	import SiteFooter from '$lib/components/SiteFooter.svelte';
	import { CONTRACT_ADDRESS, makeGnosisClient } from '$lib/chain';
	import { fetchWalletPage, type WalletRecord } from '$lib/registry';
	import type { PublicClient } from 'viem';

	const SIZE_OPTIONS = [20, 50, 100] as const;
	const DEFAULT_SIZE = 100;
	const contractUrl = `https://gnosisscan.io/address/${CONTRACT_ADDRESS}`;

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

	async function load(p: number, d: boolean, size: number) {
		const id = ++reqId;
		loading = true;
		errorMsg = null;
		expandedId = null;
		try {
			const res = await fetchWalletPage(ensureClient(), p, size, d);
			if (id !== reqId) return; // a newer request superseded this one
			total = res.total;
			records = res.records;
			loaded = { page: p, desc: d, size };
		} catch {
			if (id !== reqId) return;
			errorMsg = 'Couldn’t reach the Gnosis network. Please try again.';
			records = [];
		} finally {
			if (id === reqId) loading = false;
		}
	}

	$effect(() => {
		const p = currentPage;
		const d = desc;
		const size = pageSize;
		if (browser) load(p, d, size);
	});

	// --- Navigation (writes to the URL; the effect above reacts) ---
	/** Query string for a page + order + size. All values are app-controlled, so no
	 *  escaping is needed. Empty (clean URL) for the defaults (newest / page 1 / 100). */
	function queryFor(p: number, newest: boolean, size: number): string {
		const parts: string[] = [];
		if (!newest) parts.push('order=oldest');
		if (size !== DEFAULT_SIZE) parts.push(`size=${size}`);
		if (p > 1) parts.push(`page=${p}`);
		return parts.length ? `?${parts.join('&')}` : '';
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
		load(currentPage, desc, pageSize);
	}

	// --- Helpers ---
	/** Stable, collision-proof row identity. walletRef+credentialId is unique on an
	 *  honest chain; the index also guards against a hostile RPC returning duplicate
	 *  records, which would otherwise crash the keyed #each. */
	function rowKey(r: WalletRecord, i: number): string {
		return `${r.walletRef}:${r.credentialId}:${i}`;
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
		// Drop bidi controls (LRM/RLM/ALM, embeddings, overrides, isolates), zero-width
		// chars and the BOM so an on-chain name can't visually reverse or hide the row
		// label. Done by code point to keep the source free of invisible characters.
		let out = '';
		for (const ch of n) {
			const c = ch.codePointAt(0) ?? 0;
			const invisible =
				c === 0x061c ||
				(c >= 0x200b && c <= 0x200f) ||
				(c >= 0x202a && c <= 0x202e) ||
				(c >= 0x2066 && c <= 0x2069) ||
				c === 0xfeff;
			if (!invisible) out += ch;
		}
		const t = out.trim();
		return t.length ? t : 'Unnamed wallet';
	}
	function shortAddr(a: string): string {
		return `${a.slice(0, 6)}…${a.slice(-4)}`;
	}
	const dateFmt = new Intl.DateTimeFormat('en-US', {
		year: 'numeric',
		month: 'short',
		day: 'numeric'
	});
	const timeFmt = new Intl.DateTimeFormat('en-US', { dateStyle: 'medium', timeStyle: 'short' });
</script>

<Seo
	title="On-chain wallet registry"
	description="Every Vela wallet registers its passkey public key on Gnosis. Browse all wallets created on-chain — read live from the index contract, straight from your browser."
	canonical="/registry"
/>

<SiteHeader />

<main class="registry">
	<header class="reg-head">
		<a class="back" href={resolve('/')}>← Home</a>
		<h1>Wallet registry</h1>
		<p class="lede">
			Every Vela wallet registers its passkey public key on Gnosis the moment it's created. This
			page reads the
			<a href={contractUrl} target="_blank" rel="noopener">index contract</a>
			live from your browser — nothing here comes from Vela's servers. Don't trust us — verify.
		</p>
		<div class="reg-stat" aria-live="polite">
			<span class="live-dot" class:on={total !== null && !errorMsg}></span>
			<strong class="stat-number">{total === null ? '—' : total.toLocaleString()}</strong>
			<span>wallets created on-chain</span>
		</div>
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

	{#if errorMsg && records.length === 0}
		<div class="card error">
			<p>{errorMsg}</p>
			<button class="retry" onclick={refresh}>Try again</button>
		</div>
	{:else}
		<ol class="rows" aria-busy={loading}>
			{#if loading && records.length === 0}
				{#each skeletonRows as i (i)}
					<li class="row skeleton"><span class="sk name"></span><span class="sk date"></span></li>
				{/each}
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
											class="mono">{r.walletAddress} ↗</a
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
								<div class="detail">
									<dt>Passkey credential</dt>
									<dd>
										<code class="mono">{r.credentialId}</code>
										<button
											class="cp"
											aria-label="Copy passkey credential ID"
											onclick={() => copy(r.credentialId, `${rid}:c`)}
										>
											{copiedKey === `${rid}:c` ? 'Copied' : 'Copy'}
										</button>
									</dd>
								</div>
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

		{#if records.length === 0 && !loading && !errorMsg}
			<p class="empty">No wallets on this page.</p>
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
		font-size: 0.85rem;
		color: var(--text-muted);
		margin-bottom: 20px;
		transition: color 0.15s ease;
	}
	.back:hover {
		color: var(--text);
	}
	h1 {
		font-size: 2rem;
		font-weight: 700;
		letter-spacing: -0.02em;
		margin-bottom: 12px;
	}
	.lede {
		color: var(--text-secondary);
		line-height: 1.65;
		max-width: 640px;
		font-size: 0.98rem;
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
		font-size: 0.95rem;
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
		background: #2d8e5f;
		box-shadow: 0 0 7px rgba(45, 142, 95, 0.55);
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
		font-size: 0.85rem;
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
		box-shadow: 0 1px 2px rgba(0, 0, 0, 0.25);
	}
	.refresh {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		background: none;
		border: 1px solid var(--border);
		color: var(--text-secondary);
		font-size: 0.85rem;
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
		background: rgba(255, 255, 255, 0.02);
	}
	.ord {
		font-size: 0.8rem;
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
		font-size: 0.8rem;
		color: var(--text-muted);
	}
	time {
		font-size: 0.85rem;
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
		font-size: 0.8rem;
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
		font-size: 0.9rem;
		color: var(--text-secondary);
	}
	.mono {
		font-family: var(--font-mono);
		font-size: 0.82rem;
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
		font-size: 0.72rem;
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
		color: #fff;
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
		.row-main time {
			display: none;
		}
		.detail {
			grid-template-columns: 1fr;
			gap: 4px;
		}
	}
</style>
