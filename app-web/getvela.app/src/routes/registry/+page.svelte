<script lang="ts">
	import { browser } from '$app/environment';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import Seo from '$lib/components/Seo.svelte';
	import SiteHeader from '$lib/components/SiteHeader.svelte';
	import SiteFooter from '$lib/components/SiteFooter.svelte';
	import { CONTRACT_ADDRESS, LEGACY_CONTRACT_ADDRESS, makeGnosisClient } from '$lib/chain';
	import {
		fetchWalletPage,
		type RegistrySource,
		type WalletRecord,
		type MemberPasskey
	} from '$lib/registry';
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
	/** The current possession-proven registry is the default; the legacy
	 *  server-signed index stays browsable for pre-migration wallets. */
	const source = $derived<RegistrySource>(
		page.url.searchParams.get('source') === 'legacy' ? 'legacy' : 'v2'
	);
	const contractAddr = $derived(source === 'legacy' ? LEGACY_CONTRACT_ADDRESS : CONTRACT_ADDRESS);
	const contractUrl = $derived(`https://gnosisscan.io/address/${contractAddr}`);

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

	async function load(p: number, d: boolean, size: number) {
		const id = ++reqId;
		loading = true;
		errorMsg = null;
		expandedId = null;
		try {
			const res = await fetchWalletPage(ensureClient(), p, size, d, source);
			if (id !== reqId) return; // a newer request superseded this one
			total = res.total;
			records = res.records;
			loaded = { page: p, desc: d, size };
			resolveLabels(res.records);
		} catch (e) {
			if (id !== reqId) return;
			console.error('registry read failed', e);
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
		source; // re-read when the registry source changes
		total = null; // the count belongs to a source; clear it while switching
		if (browser) load(p, d, size);
	});

	// --- Navigation (writes to the URL; the effect above reacts) ---
	/** Query string for a page + order + size. All values are app-controlled, so no
	 *  escaping is needed. Empty (clean URL) for the defaults (newest / page 1 / 100). */
	function queryFor(p: number, newest: boolean, size: number, src: RegistrySource = source): string {
		const parts: string[] = [];
		if (src === 'legacy') parts.push('source=legacy');
		if (!newest) parts.push('order=oldest');
		if (size !== DEFAULT_SIZE) parts.push(`size=${size}`);
		if (p > 1) parts.push(`page=${p}`);
		return parts.length ? `?${parts.join('&')}` : '';
	}
	function setSource(src: RegistrySource) {
		navTo(queryFor(1, desc, pageSize, src)); // a new source starts from page 1
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
			When a Vela wallet is created, its passkey public key is written to a public registry on
			Gnosis — a cache, not a dependency: if the write fails the wallet still works, and the key
			can be re-derived on-device from two passkey signatures.
			{#if source === 'v2'}
				Vela moved to a new <strong>possession-proven</strong> registry: every entry now carries a
				WebAuthn signature that proves the writer actually holds the passkey, so no server can
				forge one. Each wallet is one immutable on-chain group of its founding passkeys.
			{:else}
				This is the <strong>legacy</strong> index — a server-signed store kept readable for wallets
				created before the migration. New wallets are written to the current registry.
			{/if}
			This page reads the
			<a href={contractUrl} target="_blank" rel="noopener">registry contract</a>
			live from your browser — nothing here comes from Vela's servers. Don't trust us — verify.
		</p>
		<div class="reg-stat" aria-live="polite">
			<span class="live-dot" class:on={total !== null && !errorMsg}></span>
			<strong class="stat-number">{total === null ? '—' : total.toLocaleString()}</strong>
			<span>wallets created on-chain</span>
		</div>
		<div class="seg source-seg" role="group" aria-label="Registry source">
			<button class:active={source === 'v2'} aria-pressed={source === 'v2'} onclick={() => setSource('v2')}
				>Current registry</button
			>
			<button
				class:active={source === 'legacy'}
				aria-pressed={source === 'legacy'}
				onclick={() => setSource('legacy')}>Legacy index</button
			>
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
											#{r.unitId} · {r.memberCount} passkey{r.memberCount === 1 ? '' : 's'}{r.walletVersion
												? ` · ${r.walletVersion}`
												: ''}
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
																	<span class="pk-value"><code class="mono">{a.att.aaguid}</code></span>
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
																	<span class="pk-value"><code class="mono wrap">{m.attestation}</code></span>
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
		.row-main time {
			display: none;
		}
		.detail {
			grid-template-columns: 1fr;
			gap: 4px;
		}
	}
</style>
