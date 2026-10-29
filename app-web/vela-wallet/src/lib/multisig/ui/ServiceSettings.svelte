<script lang="ts">
	/**
	 * Settings → Multisig service: which Safe Genesis Registry service this
	 * device uses to find multisigs and share their waiting transactions — the
	 * default, or the person's own (a developer's, on this computer).
	 *
	 * What is in use is shown, and asked (`/api/health`) when the page opens.
	 * Save asks the address typed first and says what answered: a multisig
	 * service is saved at once; nothing at all may still be saved, said so
	 * ("Save anyway"); something else is not saved. Use default forgets the
	 * person's own. The address rule and what counts as the service's answer
	 * are the core's (`app::genesis_service`).
	 */
	import { onMount } from 'svelte';
	import Button from '$lib/ui/Button.svelte';
	import { fillTemplate } from '$lib/i18n/fill';
	import { loadCore } from '$lib/core/client';
	import { chainName } from '$lib/services/networks';
	import type { StatusPillModel } from '$lib/settings/model';
	import StatusPill from '$lib/settings/ui/StatusPill.svelte';
	import UrlField from '$lib/settings/ui/UrlField.svelte';
	import type { MultisigMessages } from '../messages';
	import { serviceAddress, type ServiceAnswer, type ServiceChain } from '../core/kernel';
	import { checkService } from '../core/service';
	import { DEFAULT_GENESIS_SERVICE_URL, genesisService } from '../core/service-state.svelte';

	interface Props {
		m: MultisigMessages;
		locale: string;
		/** The person's own service changed (saved, or back to the default). */
		onchange?: () => void;
	}

	let { m, locale, onchange }: Props = $props();

	const s = $derived(m.service);

	/** What the service in use answered when last asked; `null` while asking. */
	let inUse = $state<ServiceAnswer | null>(null);
	let draft = $state(genesisService.custom ?? '');
	/** The typed address's problem, by the core's rule. */
	let problem = $state<'invalid' | 'insecure' | null>(null);
	let checking = $state(false);
	/** What answered at the typed address on Save, until it is edited. */
	let tried = $state<{ url: string; answer: ServiceAnswer } | null>(null);
	let saved = $state(false);

	async function askInUse() {
		inUse = null;
		const url = genesisService.url;
		const answer = await checkService(url);
		// Changed while it was asked: that answer is the last one's.
		if (genesisService.url === url) inUse = answer;
	}

	onMount(() => {
		void askInUse();
	});

	function pill(answer: ServiceAnswer | null): StatusPillModel {
		if (answer === null) return { tone: 'neutral', label: s.checking, dot: true };
		if (answer.kind === 'genesis') return { tone: 'ok', label: s.reachable, dot: true };
		if (answer.kind === 'other') return { tone: 'warn', label: s.notService, dot: true };
		return { tone: 'error', label: s.unreachable, dot: true };
	}

	/** "Serves Arc Testnet and Gnosis": the wallet's names, else the service's own. */
	function serves(chains: ServiceChain[] | null): string | null {
		if (chains === null || chains.length === 0) return null;
		const names = chains.map((c) => {
			const own = chainName(c.chainId);
			return own === `Chain ${c.chainId}` ? (c.name ?? own) : own;
		});
		let list: string;
		try {
			list = new Intl.ListFormat(locale, { type: 'conjunction' }).format(names);
		} catch {
			list = names.join(', ');
		}
		return fillTemplate(s.serves, { networks: list });
	}

	function use(url: string | null) {
		genesisService.set(url);
		draft = url ?? '';
		tried = null;
		saved = true;
		onchange?.();
		void askInUse();
	}

	async function save() {
		saved = false;
		tried = null;
		problem = null;
		await loadCore();
		const address = serviceAddress(draft);
		if (address.kind !== 'ok') {
			problem = address.kind;
			return;
		}
		// The default typed out is the default, not a service of the person's own.
		if (address.url === DEFAULT_GENESIS_SERVICE_URL) return use(null);
		checking = true;
		const answer = await checkService(address.url);
		checking = false;
		if (answer.kind === 'genesis') {
			use(address.url);
			inUse = answer;
			return;
		}
		tried = { url: address.url, answer };
	}

	const field = $derived({
		id: 'genesis-service',
		label: s.field,
		value: draft,
		placeholder: DEFAULT_GENESIS_SERVICE_URL,
		hint: s.privacy,
		tone: problem !== null || tried?.answer.kind === 'other' ? ('error' as const) : undefined
	});
	const custom = $derived(genesisService.custom);
	const inUseServes = $derived(inUse?.kind === 'genesis' ? serves(inUse.chains) : null);
</script>

<p class="description">{s.description}</p>

<section class="in-use" data-testid="genesis-service-in-use">
	<div class="head">
		<span class="label">{s.inUse}</span>
		<StatusPill pill={pill(inUse)} />
	</div>
	<p class="url">
		{genesisService.url}{#if custom === null}<span class="tag">{s.default}</span>{/if}
	</p>
	{#if inUseServes !== null}<p class="hint">{inUseServes}</p>{/if}
</section>

<div class="field">
	<UrlField
		{field}
		oninput={(value) => {
			draft = value;
			problem = null;
			tried = null;
			saved = false;
		}}
	/>
</div>

{#if problem !== null}
	<p class="problem" role="alert">{problem === 'insecure' ? s.insecure : s.invalid}</p>
{:else if tried?.answer.kind === 'other'}
	<p class="problem" role="alert">{s.unsavedNotService}</p>
{:else if tried !== null}
	<p class="problem" role="alert">{s.unsavedUnreachable}</p>
{:else if saved}
	<p class="saved" role="status">{s.saved}</p>
{/if}

<div class="actions">
	<Button variant="primary" shape="rounded" loading={checking} onclick={() => void save()}
		>{s.save}</Button
	>
	{#if tried !== null && tried.answer.kind === 'unreachable'}
		{@const url = tried.url}
		<Button variant="secondary" shape="rounded" onclick={() => use(url)}>{s.saveAnyway}</Button>
	{/if}
	{#if custom !== null}
		<Button variant="secondary" shape="rounded" onclick={() => use(null)}>{s.useDefault}</Button>
	{/if}
</div>

<style>
	.description {
		margin: 0 0 var(--space-xl);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-muted);
	}

	.in-use {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
		padding: var(--space-lg);
		border-radius: var(--radius-lg);
		background: var(--color-bg-raised);
		margin-block-end: var(--space-3xl);
	}

	.in-use p {
		margin: 0;
	}

	.head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-md);
	}

	.label {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		letter-spacing: var(--letterSpacing-sectionLabel);
		color: var(--color-fg-subtle);
	}

	.url {
		font-family: var(--font-mono);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-fg-base);
		overflow-wrap: anywhere;
	}

	.tag {
		margin-inline-start: var(--space-md);
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-subtle);
	}

	.hint {
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-subtle);
	}

	.problem,
	.saved {
		margin: var(--space-md) 0 0;
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		line-height: var(--leading-normal);
	}

	.problem {
		color: var(--color-error-base);
	}

	.saved {
		color: var(--color-fg-muted);
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-md);
		margin-block: var(--space-xl);
	}
</style>
