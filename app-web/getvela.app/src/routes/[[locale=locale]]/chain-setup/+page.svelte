<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { page } from '$app/state';
	import Seo from '$lib/components/Seo.svelte';
	import SiteFooter from '$lib/components/SiteFooter.svelte';
	import SiteHeader from '$lib/components/SiteHeader.svelte';
	import TranslationNotice from '$lib/components/TranslationNotice.svelte';
	import { pathFor } from '$lib/i18n/locales';
	import { catalog, namespaceState, translatedLocales } from '$lib/i18n/resolve';
	import { SAFE_FACTORY_GUIDE_URL, SAFE_FACTORY_REQUEST_URL } from '$lib/chain-setup/deployers';
	import { RIP_7212_URL } from '$lib/chain-setup/required-contracts';
	import { ChainSetup, formatCoin } from '$lib/chain-setup/setup.svelte';
	import type { Step } from '$lib/chain-setup/verdict';
	import type { PageData } from './$types';

	/**
	 * Set up a chain for Vela.
	 *
	 * Two phases, kept visibly apart. First a verdict — can Vela run here, and
	 * if not, is that fixable — laid out so a person can read the top card and
	 * stop. Then, only when something is missing AND fixable, the steps: each
	 * one action, one status, and a line saying who can do it. The list of what
	 * is required is the wallet's own (`chain-setup/required-contracts.ts`), so
	 * this page can never call ready what the wallet would refuse.
	 */
	let { data }: { data: PageData } = $props();

	const m = $derived(catalog(data.locale));
	const t = $derived(m.chainSetup);
	const translation = $derived(namespaceState('chainSetup', data.locale));
	const alternates = $derived(translatedLocales('chainSetup'));

	const setup = new ChainSetup();
	let copied = $state<string | null>(null);
	let sweepTo = $state('');

	const fill = (s: string, vars: Record<string, string | number>) =>
		s.replace(/\{\{(\w+)\}\}/g, (_, k: string) => String(vars[k] ?? ''));

	const symbol = $derived(setup.chain?.nativeSymbol ?? '');
	const decimals = $derived(setup.chain?.nativeDecimals ?? 18);
	const coin = (wei: bigint | null | undefined) => (wei == null ? '—' : formatCoin(wei, decimals));
	const host = (url: string) => {
		try {
			return new URL(url).host;
		} catch {
			return url;
		}
	};
	const txUrl = (hash: string) =>
		setup.chain?.explorer ? `${setup.chain.explorer}/tx/${hash}` : null;

	const verdict = $derived(setup.plan?.verdict ?? null);
	const create2Steps = $derived(
		(setup.plan?.steps ?? []).filter(
			(s): s is Extract<Step, { kind: 'create2' }> => s.kind === 'create2'
		)
	);
	const needsDeployer = $derived(create2Steps.length > 0);
	const anyDone = $derived(Object.values(setup.steps).some((r) => r.status === 'done'));

	// Poll balances only while something on the page is waiting on money.
	$effect(() => {
		const waitingOnFunds =
			(setup.plan?.steps ?? []).some(
				(s) => s.kind === 'fund-and-broadcast' && setup.stepRun(s.contract.key).status !== 'done'
			) || setup.deployer !== null;
		setup.watchBalances(setup.phase === 'done' && waitingOnFunds);
	});

	onMount(() => {
		const q = page.url.searchParams.get('chain') ?? page.url.searchParams.get('rpc');
		if (q) void setup.start(q);
	});
	onDestroy(() => setup.watchBalances(false));

	async function copy(text: string) {
		try {
			await navigator.clipboard.writeText(text);
			copied = text;
			setTimeout(() => (copied = null), 1500);
		} catch {
			/* clipboard blocked — the text is on screen */
		}
	}

	function download(name: string, text: string) {
		const blob = new Blob([text], { type: 'text/plain' });
		const url = URL.createObjectURL(blob);
		const a = document.createElement('a');
		a.href = url;
		a.download = name;
		a.click();
		URL.revokeObjectURL(url);
	}

	function submit(e: SubmitEvent) {
		e.preventDefault();
		void setup.start();
	}
</script>

<Seo
	title={t.meta.title}
	description={t.meta.description}
	canonical={pathFor(data.locale, '/chain-setup')}
	locale={data.locale}
	{alternates}
	englishPath="/chain-setup"
/>

{#if translation === 'fallback'}
	<TranslationNotice locale={data.locale} />
{/if}

<SiteHeader locale={data.locale} />

<main class="wrap">
	<header class="intro">
		<h1>{t.heading}</h1>
		<p class="lede">{t.lede}</p>
	</header>

	<!-- Phase 1 — the question -->
	<form class="ask" onsubmit={submit}>
		<label class="field">
			<span class="label">{t.input.label}</span>
			<input
				type="text"
				bind:value={setup.input}
				oninput={() => setup.suggest()}
				placeholder={t.input.placeholder}
				autocomplete="off"
				spellcheck="false"
				inputmode="url"
				aria-autocomplete="list"
				aria-controls="chain-suggestions"
			/>
		</label>
		<button
			class="btn btn-primary"
			type="submit"
			disabled={setup.phase === 'resolving' || setup.phase === 'checking'}
		>
			{t.input.action}
		</button>
	</form>
	{#if setup.suggestions.length}
		<ul class="suggestions" id="chain-suggestions" role="listbox" aria-label={t.input.suggestions}>
			{#each setup.suggestions as hit (hit.chainId)}
				<li role="option" aria-selected="false">
					<button type="button" class="suggestion" onclick={() => setup.pick(hit)}>
						<span class="s-name">{hit.name}</span>
						<span class="s-meta"
							><span class="mono">{hit.chainId}</span> · {hit.nativeCurrencySymbol}</span
						>
					</button>
				</li>
			{/each}
		</ul>
	{/if}
	<p class="hint">{t.input.hint}</p>

	{#if setup.phase === 'error'}
		<div class="note note-warn" role="alert">
			{t.errors[(setup.error ?? 'unknown') as keyof typeof t.errors] ?? t.errors.unknown}
		</div>
	{/if}

	{#if setup.phase === 'resolving' && !setup.chain}
		<p class="muted resolving" aria-live="polite">{t.resolving}</p>
	{/if}

	{#if setup.chain}
		<section class="chain">
			<div class="who">
				<h2>{setup.chain.name ?? `Chain ${setup.chain.chainId}`}</h2>
				<span class="id">Chain {setup.chain.chainId}</span>
			</div>
			{#if setup.phase === 'resolving'}
				<p class="muted">{t.resolving}</p>
			{:else if setup.phase === 'checking'}
				<p class="muted">{t.checking}</p>
			{:else if setup.rpcUrl}
				{@const used = setup.rpcs.find((r) => r.url === setup.rpcUrl)}
				<details class="rpc">
					<summary>
						{fill(t.rpcUsed, { host: host(setup.rpcUrl), ms: used?.latencyMs ?? '—' })}
						<span class="link">{t.changeRpc}</span>
					</summary>
					<ul>
						{#each setup.rpcs as r (r.url)}
							<li>
								<button
									type="button"
									class="rpc-opt"
									disabled={r.latencyMs === null}
									aria-current={r.url === setup.rpcUrl}
									onclick={() => setup.useRpc(r.url)}
								>
									<span class="mono">{host(r.url)}</span>
									<span class="muted">{r.latencyMs === null ? '—' : `${r.latencyMs} ms`}</span>
								</button>
							</li>
						{/each}
					</ul>
				</details>
			{/if}
		</section>
	{/if}

	<!-- The verdict -->
	{#if setup.phase === 'done' && setup.plan && verdict}
		{@const p = setup.plan}
		<section class="verdict {verdict}" aria-live="polite">
			{#if verdict === 'ready'}
				<h2>{t.verdict.ready.title}</h2>
				<p>{t.verdict.ready.body}</p>
				<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -->
				<a class="btn" href={pathFor(data.locale, '/docs/networks-and-fees')}
					>{t.verdict.ready.action}</a
				>
			{:else if verdict === 'needs-setup'}
				<h2>{fill(t.verdict.needsSetup.title, { count: p.missing.length })}</h2>
				<p>{t.verdict.needsSetup.body}</p>
			{:else}
				<h2>{t.verdict.blocked.title}</h2>
				<p>{t.verdict.blocked.body}</p>
				<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -->
				<a class="link" href={RIP_7212_URL} target="_blank" rel="noopener"
					>{t.verdict.blocked.link} ↗</a
				>
			{/if}
			{#if p.unknown.length}
				<p class="sub">{t.verdict.provisional}</p>
			{/if}
			{#if p.mismatched.length}
				<p class="sub">{t.verdict.mismatch}</p>
			{/if}
			<button type="button" class="link recheck" onclick={() => setup.recheck()}>{t.recheck}</button
			>
		</section>

		<!-- The checklist: P-256 first because it is the one nobody can fix -->
		<section class="checklist">
			<h2>{t.checklist.heading}</h2>
			<ol>
				<li class="row" data-state={p.p256 === 'missing' ? 'missing' : 'present'}>
					<span class="mark" aria-hidden="true"></span>
					<div class="text">
						<span class="name">{t.checklist.p256}</span>
						<span class="what">{t.checklist.p256What}</span>
					</div>
					<span class="state"
						>{p.p256 === 'missing' ? t.checklist.missing : t.checklist.present}</span
					>
				</li>
				{#each setup.result?.contracts ?? [] as c (c.contract.key)}
					{@const st = c.unknown
						? 'unknown'
						: c.mismatch
							? 'mismatch'
							: c.deployed
								? 'present'
								: 'missing'}
					<li class="row" data-state={st}>
						<span class="mark" aria-hidden="true"></span>
						<div class="text">
							<span class="name">{c.contract.name}</span>
							<span class="what">{c.contract.what}</span>
							<code class="addr">{c.contract.address}</code>
						</div>
						<span class="state">{t.checklist[st]}</span>
					</li>
				{/each}
			</ol>
		</section>

		<!-- Phase 2 — only when there is somewhere to go -->
		{#if (verdict === 'needs-setup' && p.steps.length) || (setup.completed.length && verdict !== 'blocked')}
			<section class="plan">
				<h2>{t.plan.heading}</h2>
				<p class="lede">{t.plan.lede}</p>

				<ol class="steps">
					{#each [...setup.completed, ...p.steps] as step, i (step.contract.key)}
						{@const run = setup.stepRun(step.contract.key)}
						<li class="step" data-status={run.status}>
							<div class="step-head">
								<span class="n">{i + 1}</span>
								<div class="text">
									<span class="name">{step.contract.name}</span>
									<span class="who">
										{step.kind === 'external'
											? t.plan.who.safe
											: step.kind === 'fund-and-broadcast'
												? t.plan.who.anyone
												: t.plan.who.deployer}
									</span>
								</div>
								<span class="status">
									{#if run.status === 'done'}{t.plan.status.done}
									{:else if run.status === 'sending'}{t.plan.status.sending}
									{:else if run.status === 'confirming'}{t.plan.status.confirming}
									{:else if run.status === 'failed'}{t.plan.status.failed}{/if}
								</span>
							</div>

							{#if run.status !== 'done'}
								{#if step.kind === 'fund-and-broadcast'}
									{@const bal = setup.senderBalances[step.deployer] ?? 0n}
									{@const ready = setup.senderReady(step)}
									<div class="step-body">
										<p>{fill(t.plan.fund.sendTo, { amount: coin(step.fundingWei), symbol })}</p>
										{@render address(step.deployer)}
										<p class="muted small">{t.plan.fund.why}</p>
										<p class="line">
											<span>{fill(t.plan.fund.balance, { amount: coin(bal), symbol })}</span>
											<span class="muted">· {ready ? t.plan.fund.ready : t.plan.fund.waiting}</span>
										</p>
										<button
											type="button"
											class="btn btn-primary"
											disabled={!ready || run.status === 'sending' || run.status === 'confirming'}
											onclick={() => setup.broadcast(step)}>{t.plan.fund.action}</button
										>
									</div>
								{:else if step.kind === 'external'}
									{@const waiting = p.steps.filter(
										(s) => s.kind === 'create2' && s.blockedBy?.key === step.contract.key
									).length}
									<div class="step-body">
										<p>{t.plan.external.body}</p>
										{#if waiting}<p class="muted small">
												{fill(t.plan.external.blocked, { count: waiting })}
											</p>{/if}
										<div class="actions">
											<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -->
											<a class="btn" href={SAFE_FACTORY_REQUEST_URL} target="_blank" rel="noopener"
												>{t.plan.external.action} ↗</a
											>
											<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -->
											<a class="link" href={SAFE_FACTORY_GUIDE_URL} target="_blank" rel="noopener"
												>{t.plan.external.guide} ↗</a
											>
											<button type="button" class="link" onclick={() => setup.recheck()}
												>{t.recheck}</button
											>
										</div>
									</div>
								{:else}
									<div class="step-body">
										{#if step.blockedBy}
											<p class="muted">
												{fill(t.plan.create2.blockedBy, { name: step.blockedBy.name })}
											</p>
										{:else}
											<button
												type="button"
												class="btn"
												disabled={setup.deployingAll ||
													run.status === 'sending' ||
													run.status === 'confirming'}
												onclick={() => setup.deploy(step)}>{t.plan.create2.action}</button
											>
										{/if}
									</div>
								{/if}
							{/if}

							{#if run.error}
								<p class="fail">
									{t.plan.failures[run.error as keyof typeof t.plan.failures] ?? run.error}
								</p>
							{/if}
							{#if run.hash}
								{@const u = txUrl(run.hash)}
								<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -->
								{#if u}<a class="link small" href={u} target="_blank" rel="noopener"
										>{t.plan.status.view} ↗</a
									>
								{:else}<code class="addr">{run.hash}</code>{/if}
							{/if}
						</li>
					{/each}
				</ol>
			</section>
		{/if}
		{#if verdict === 'ready' && anyDone}
			<section class="note note-ok">
				<strong>{t.plan.complete.title}</strong> — {t.plan.complete.body}
			</section>
		{/if}
		<!-- Shown whenever a key exists for this chain, not only while there is work
		     for it: a person who reloads after deploying everything still has gas on
		     that key, and the verdict flipping to "ready" must never hide the way to
		     get it back. -->
		{#if needsDeployer || setup.deployer}
			<aside class="deployer">
				<h3>{t.plan.deployer.heading}</h3>
				<p>{t.plan.deployer.body}</p>
				{#if !setup.deployer}
					<button type="button" class="btn" onclick={() => setup.ensureDeployer()}
						>{t.plan.deployer.create}</button
					>
				{:else}
					<dl>
						<dt>{t.plan.deployer.address}</dt>
						<dd>{@render address(setup.deployer.address)}</dd>
						<dt>{t.plan.deployer.balance}</dt>
						<dd>{coin(setup.deployerBalance)} {symbol}</dd>
					</dl>
					{#if setup.fundingEstimate() !== null && setup.fundingEstimate() !== 0n}
						<p class="muted small">
							{fill(t.plan.deployer.estimate, {
								amount: coin(setup.fundingEstimate()),
								symbol
							})}
						</p>
					{/if}
					<div class="actions">
						{#if create2Steps.length}
							<button
								type="button"
								class="btn btn-primary"
								disabled={setup.deployingAll ||
									create2Steps.every(
										(s) => s.blockedBy || setup.stepRun(s.contract.key).status === 'done'
									)}
								onclick={() => setup.deployAll()}
							>
								{setup.deployingAll
									? t.plan.create2.deploying
									: fill(t.plan.create2.all, {
											count: create2Steps.filter(
												(s) => !s.blockedBy && setup.stepRun(s.contract.key).status !== 'done'
											).length
										})}
							</button>
						{/if}
						<button
							type="button"
							class="link"
							onclick={() => {
								const text = setup.exportText();
								if (text) download(`vela-chain-setup-${setup.chain?.chainId}.txt`, text);
							}}>{t.plan.deployer.export}</button
						>
					</div>
					<p class="muted small">{t.plan.deployer.keyWarn}</p>

					<div class="sweep">
						<label class="field">
							<span class="label">{t.plan.deployer.sweepTo}</span>
							<input type="text" bind:value={sweepTo} placeholder="0x…" spellcheck="false" />
						</label>
						<button
							type="button"
							class="btn"
							disabled={!/^0x[0-9a-fA-F]{40}$/.test(sweepTo)}
							onclick={() => setup.sweep(sweepTo as `0x${string}`)}>{t.plan.deployer.sweep}</button
						>
					</div>
					{#if setup.sweepHash}
						{@const u = txUrl(setup.sweepHash)}
						<p class="small">
							{t.plan.deployer.swept}
							<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -->
							{#if u}<a class="link" href={u} target="_blank" rel="noopener"
									>{t.plan.status.view} ↗</a
								>{/if}
						</p>
					{:else if setup.sweepHash === null && setup.deployerBalance === 0n}
						<p class="muted small">{t.plan.deployer.nothingToSweep}</p>
					{/if}
					<p class="muted small">
						<button type="button" class="link danger" onclick={() => setup.forgetKey()}
							>{t.plan.deployer.forget}</button
						>
						— {t.plan.deployer.forgetWarn}
					</p>
				{/if}
			</aside>
		{/if}
	{/if}

	<section class="about">
		<h2>{t.about.heading}</h2>
		{#each t.about.items as item (item.q)}
			<details>
				<summary>{item.q}</summary>
				<p>{item.a}</p>
			</details>
		{/each}
	</section>
</main>

<SiteFooter locale={data.locale} />

{#snippet address(value: string)}
	<button type="button" class="address" onclick={() => copy(value)} title={t.copy}>
		<code>{value}</code>
		<span class="copy">{copied === value ? t.copied : t.copy}</span>
	</button>
{/snippet}

<style>
	.wrap {
		max-width: var(--max-w-prose);
		margin: 0 auto;
		padding: 48px 20px 96px;
	}
	.intro h1 {
		margin: 0 0 12px;
		font-size: clamp(1.8rem, 4vw, 2.6rem);
		letter-spacing: -0.02em;
	}
	.lede {
		margin: 0;
		color: var(--text-secondary);
		font-size: 1.05rem;
		line-height: 1.6;
	}

	/* the question */
	.ask {
		display: flex;
		gap: 10px;
		align-items: end;
		margin-top: 32px;
	}
	.field {
		display: flex;
		flex-direction: column;
		gap: 6px;
		flex: 1;
	}
	.label {
		font-size: 0.8rem;
		font-weight: 600;
		color: var(--text-secondary);
	}
	input[type='text'] {
		width: 100%;
		padding: 12px 14px;
		border: 1px solid var(--border-strong);
		border-radius: var(--radius-sm);
		background: var(--bg-card);
		color: var(--text);
		font: inherit;
		font-family: var(--font-mono);
		font-size: 0.95rem;
	}
	input[type='text']:focus {
		outline: 2px solid var(--accent);
		outline-offset: 1px;
	}
	.suggestions {
		list-style: none;
		margin: 6px 0 0;
		padding: 4px;
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		background: var(--bg-card);
		box-shadow: var(--shadow-md);
	}
	.suggestion {
		display: flex;
		justify-content: space-between;
		gap: 12px;
		width: 100%;
		padding: 8px 10px;
		border: none;
		border-radius: var(--radius-sm);
		background: none;
		color: var(--text);
		font: inherit;
		text-align: left;
		cursor: pointer;
	}
	.suggestion:hover,
	.suggestion:focus-visible {
		background: var(--accent-soft);
	}
	.s-name {
		font-weight: 600;
	}
	.s-meta {
		color: var(--text-tertiary);
		font-size: 0.85rem;
		white-space: nowrap;
	}
	.hint {
		margin: 8px 0 0;
		font-size: 0.85rem;
		color: var(--text-tertiary);
	}

	.btn {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		padding: 11px 18px;
		border: 1px solid var(--border-strong);
		border-radius: var(--radius-sm);
		background: var(--bg-card);
		color: var(--text);
		font: inherit;
		font-weight: 600;
		text-decoration: none;
		cursor: pointer;
		white-space: nowrap;
	}
	.btn-primary {
		background: var(--accent);
		border-color: var(--accent);
		color: var(--text-on-accent);
	}
	.btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
	.link {
		background: none;
		border: none;
		padding: 0;
		font: inherit;
		color: var(--link);
		text-decoration: underline;
		cursor: pointer;
	}
	.link.danger {
		color: var(--status-warn-text);
	}
	.muted {
		color: var(--text-tertiary);
	}
	.small {
		font-size: 0.85rem;
	}
	.mono,
	code {
		font-family: var(--font-mono);
		font-size: 0.85em;
	}

	.note {
		margin-top: 20px;
		padding: 14px 16px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--border);
		background: var(--bg-raised);
	}
	.note-warn {
		border-color: var(--status-warn);
		color: var(--status-warn-text);
	}
	.note-ok {
		border-color: var(--green);
	}

	.resolving {
		margin-top: 24px;
	}

	/* chain identity */
	.chain {
		margin-top: 32px;
		padding: 16px 18px;
		border: 1px solid var(--border);
		border-radius: var(--radius);
		background: var(--bg-card);
	}
	.who {
		display: flex;
		align-items: baseline;
		gap: 12px;
	}
	.who h2 {
		margin: 0;
		font-size: 1.2rem;
	}
	.id {
		color: var(--text-tertiary);
		font-family: var(--font-mono);
		font-size: 0.85rem;
	}
	.rpc {
		margin-top: 8px;
		font-size: 0.9rem;
		color: var(--text-secondary);
	}
	.rpc summary {
		cursor: pointer;
		list-style: none;
	}
	.rpc summary .link {
		margin-left: 8px;
		font-size: 0.85rem;
	}
	.rpc ul {
		list-style: none;
		margin: 8px 0 0;
		padding: 0;
	}
	.rpc-opt {
		display: flex;
		justify-content: space-between;
		width: 100%;
		padding: 6px 8px;
		border: none;
		border-radius: var(--radius-sm);
		background: none;
		color: var(--text);
		font: inherit;
		cursor: pointer;
		text-align: left;
	}
	.rpc-opt[aria-current='true'] {
		background: var(--accent-soft);
	}
	.rpc-opt:disabled {
		opacity: 0.45;
		cursor: default;
	}

	/* verdict */
	.verdict {
		margin-top: 20px;
		padding: 22px 24px;
		border-radius: var(--radius);
		border: 1px solid var(--border);
		border-left-width: 6px;
		background: var(--bg-card);
	}
	.verdict.ready {
		border-left-color: var(--green);
	}
	.verdict.needs-setup {
		border-left-color: var(--status-warn);
	}
	.verdict.blocked {
		border-left-color: var(--status-warn-text);
	}
	.verdict h2 {
		margin: 0 0 8px;
		font-size: 1.35rem;
	}
	.verdict p {
		margin: 0 0 12px;
		line-height: 1.6;
	}
	.verdict .sub {
		font-size: 0.9rem;
		color: var(--text-secondary);
	}
	.verdict .recheck {
		display: block;
		margin-top: 8px;
		font-size: 0.85rem;
	}

	/* checklist */
	.checklist {
		margin-top: 36px;
	}
	.checklist h2,
	.plan h2,
	.about h2 {
		font-size: 1.15rem;
		margin: 0 0 12px;
	}
	.checklist ol,
	.steps {
		list-style: none;
		margin: 0;
		padding: 0;
		border-top: 1px solid var(--border);
	}
	.row {
		display: grid;
		grid-template-columns: 18px 1fr auto;
		gap: 14px;
		align-items: start;
		padding: 14px 0;
		border-bottom: 1px solid var(--border);
	}
	.mark {
		width: 12px;
		height: 12px;
		margin-top: 5px;
		border-radius: 50%;
		background: var(--text-tertiary);
	}
	.row[data-state='present'] .mark {
		background: var(--green);
	}
	.row[data-state='missing'] .mark,
	.row[data-state='mismatch'] .mark {
		background: var(--status-warn-text);
	}
	.row .text,
	.step .text {
		display: flex;
		flex-direction: column;
		gap: 3px;
		min-width: 0;
	}
	.name {
		font-weight: 600;
	}
	.what {
		font-size: 0.9rem;
		color: var(--text-secondary);
	}
	.addr {
		font-size: 0.75rem;
		color: var(--text-tertiary);
		overflow-wrap: anywhere;
	}
	.state {
		font-size: 0.85rem;
		color: var(--text-secondary);
		white-space: nowrap;
	}
	.row[data-state='missing'] .state,
	.row[data-state='mismatch'] .state {
		color: var(--status-warn-text);
		font-weight: 600;
	}

	/* plan */
	.plan {
		margin-top: 44px;
	}
	.plan > .lede {
		font-size: 0.95rem;
		margin-bottom: 16px;
	}
	.step {
		padding: 16px 0;
		border-bottom: 1px solid var(--border);
	}
	.step-head {
		display: grid;
		grid-template-columns: 28px 1fr auto;
		gap: 12px;
		align-items: start;
	}
	.n {
		width: 26px;
		height: 26px;
		display: grid;
		place-items: center;
		border-radius: 50%;
		background: var(--bg-sunken);
		font-size: 0.8rem;
		font-weight: 700;
	}
	.step[data-status='done'] .n {
		background: var(--green-soft);
		color: var(--green);
	}
	.step .who {
		font-size: 0.85rem;
		color: var(--text-tertiary);
	}
	.status {
		font-size: 0.85rem;
		color: var(--text-secondary);
	}
	.step[data-status='done'] .status {
		color: var(--green);
		font-weight: 600;
	}
	.step-body {
		margin: 12px 0 0 40px;
		display: flex;
		flex-direction: column;
		gap: 10px;
	}
	.step-body p {
		margin: 0;
	}
	.line {
		display: flex;
		gap: 6px;
		flex-wrap: wrap;
		font-size: 0.9rem;
	}
	.actions {
		display: flex;
		gap: 14px;
		align-items: center;
		flex-wrap: wrap;
	}
	.fail {
		margin: 10px 0 0 40px;
		color: var(--status-warn-text);
		font-size: 0.9rem;
	}
	.step > .link.small,
	.step > .addr {
		display: inline-block;
		margin: 6px 0 0 40px;
	}

	.address {
		display: inline-flex;
		align-items: center;
		gap: 10px;
		max-width: 100%;
		padding: 8px 12px;
		border: 1px solid var(--border);
		border-radius: var(--radius-sm);
		background: var(--bg-sunken);
		color: var(--text);
		font: inherit;
		cursor: pointer;
		text-align: left;
	}
	.address code {
		overflow-wrap: anywhere;
	}
	.address .copy {
		font-size: 0.8rem;
		color: var(--link);
		white-space: nowrap;
	}

	.deployer {
		margin-top: 28px;
		padding: 18px 20px;
		border: 1px solid var(--border-accent);
		border-radius: var(--radius);
		background: var(--bg-card);
	}
	.deployer h3 {
		margin: 0 0 6px;
		font-size: 1.05rem;
	}
	.deployer p {
		margin: 0 0 12px;
		line-height: 1.55;
	}
	.deployer dl {
		display: grid;
		grid-template-columns: auto 1fr;
		gap: 8px 16px;
		margin: 0 0 12px;
		align-items: center;
	}
	.deployer dt {
		font-size: 0.85rem;
		color: var(--text-secondary);
	}
	.deployer dd {
		margin: 0;
	}
	.sweep {
		display: flex;
		gap: 10px;
		align-items: end;
		margin-top: 14px;
	}

	.about {
		margin-top: 56px;
	}
	.about details {
		padding: 12px 0;
		border-bottom: 1px solid var(--border);
	}
	.about summary {
		cursor: pointer;
		font-weight: 600;
	}
	.about p {
		margin: 10px 0 0;
		color: var(--text-secondary);
		line-height: 1.6;
	}

	@media (max-width: 560px) {
		.ask,
		.sweep {
			flex-direction: column;
			align-items: stretch;
		}
		.row {
			grid-template-columns: 18px 1fr;
		}
		.row .state {
			grid-column: 2;
		}
		.step-body,
		.fail {
			margin-left: 0;
		}
	}
</style>
