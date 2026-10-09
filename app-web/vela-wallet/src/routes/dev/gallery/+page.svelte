<script lang="ts">
	/**
	 * The v2 onboarding state gallery — dev-only.
	 *
	 * Every state is a `CreateView` the core could emit, rendered through the
	 * REAL screens (spec 019, `v2-fixtures.ts`). That is the whole design of
	 * this page: spec 014's gallery had its own state vocabulary because no core
	 * existed to render from, so a screen could pass the gallery and still fail
	 * in production. It cannot now — a fixture that renders wrong here renders
	 * the real machine wrong too.
	 *
	 * No passkey is touched, no network is called and nothing is written: the
	 * views are values, not a running machine.
	 */
	import { onMount } from 'svelte';
	import type { PageProps } from './$types';
	import FlowShell from '$lib/ui/onboarding/v2/FlowShell.svelte';
	import NameScreen from '$lib/ui/onboarding/v2/NameScreen.svelte';
	import KeysScreen from '$lib/ui/onboarding/v2/KeysScreen.svelte';
	import ProgressScreen from '$lib/ui/onboarding/v2/ProgressScreen.svelte';
	import RetryScreen from '$lib/ui/onboarding/v2/RetryScreen.svelte';
	import DoneScreen from '$lib/ui/onboarding/v2/DoneScreen.svelte';
	import PromptSheet from '$lib/ui/onboarding/v2/PromptSheet.svelte';
	import { CREATE_FIXTURES, PROMPT_FIXTURES } from '$lib/onboarding/v2-fixtures';
	import {
		progressFor,
		promptCopy,
		statusKeyToI18n,
		submitLabelToI18n
	} from '$lib/onboarding/core/copy';
	import { fillTemplate } from '$lib/i18n/fill';
	import { pluralTemplate } from '$lib/i18n/plural';
	import { loadOnboardingCore } from '$lib/onboarding/core/wasm-client';
	import { RAIL_STEPS, type RailSlot } from '$lib/ui/onboarding/v2/rail';

	let { data }: PageProps = $props();

	/*
	 * The core is loaded but never RUN: no machine is constructed, no passkey
	 * touched, no network called. The done screen draws its identicon with the
	 * same kernel that ships, so a gallery that skipped this would show a blank
	 * circle where production shows artwork — a gallery lying about the one
	 * thing it exists to be honest about.
	 */
	let coreReady = $state(false);
	onMount(() => {
		loadOnboardingCore().then(() => (coreReady = true));
	});

	let locale = $state('zh');
	let theme = $state<'dark' | 'light'>('dark');
	let selected = $state(CREATE_FIXTURES[0].code);
	let promptCode = $state<string | null>(null);
	let lastEvent = $state('—');

	const catalog = $derived((data.catalogs as Record<string, unknown>)[locale] ?? {});

	function lookup(key: string): string | undefined {
		let node: unknown = catalog;
		for (const part of key.split('.')) {
			if (typeof node !== 'object' || node === null) return undefined;
			node = (node as Record<string, unknown>)[part];
		}
		return typeof node === 'string' ? node : undefined;
	}

	function strings(key: string, params?: Record<string, string | number>): string {
		// A plural key (issue 409) has no bare value; the core picks its form.
		const template =
			lookup(key) ??
			(typeof params?.count === 'number'
				? pluralTemplate(lookup, locale, key, params.count)
				: undefined);
		return template === undefined ? key : fillTemplate(template, params);
	}

	const fixture = $derived(CREATE_FIXTURES.find((f) => f.code === selected) ?? CREATE_FIXTURES[0]);
	const view = $derived(fixture.view);
	const prompt = $derived(PROMPT_FIXTURES.find((p) => p.code === promptCode) ?? null);

	const screen = $derived.by(() => {
		if (view.stage === 'created') return 'done' as const;
		if (view.stage === 'sync_failed') return 'retry' as const;
		if (view.busy && progressFor(view.status)) return 'progress' as const;
		return view.stage === 'add_keys' ? ('keys' as const) : ('form' as const);
	});

	const statusText = $derived(
		view.status && !progressFor(view.status) ? strings(statusKeyToI18n(view.status)) : undefined
	);

	// The same rail copy CreateFlow derives, so a fixture shows the rail the
	// real flow would — the gallery must not invent its own vocabulary.
	const railSlot = $derived.by((): RailSlot => {
		const step = (ordinal: number, key: string): RailSlot => ({
			kind: 'step',
			ordinal,
			total: RAIL_STEPS,
			name: strings(`onboarding.create.step${key}Label`),
			detail: strings(`onboarding.create.step${key}Detail`)
		});
		switch (screen) {
			case 'form':
				return step(1, 'Naming');
			case 'keys':
				return step(2, 'Keys');
			case 'progress':
			case 'retry':
				return step(3, 'Create');
			default:
				return { kind: 'tagline', text: strings('onboarding.welcome.heroSubtitle') };
		}
	});

	function log(what: string) {
		lastEvent = what;
	}
</script>

<svelte:head><title>Onboarding v2 — state gallery</title></svelte:head>

<div class="gallery" data-theme={theme}>
	<aside class="rail">
		<div class="switches">
			<label>
				<span>Locale</span>
				<select bind:value={locale}>
					{#each data.locales as l (l)}<option value={l}>{l}</option>{/each}
				</select>
			</label>
			<label>
				<span>Theme</span>
				<select bind:value={theme}>
					<option value="dark">dark</option>
					<option value="light">light</option>
				</select>
			</label>
		</div>

		<h2>Create</h2>
		<ul>
			{#each CREATE_FIXTURES as f (f.code)}
				<li>
					<button
						class:active={selected === f.code && promptCode === null}
						onclick={() => {
							selected = f.code;
							promptCode = null;
						}}
					>
						<span class="code">{f.code}</span>
						<span class="label">{f.label}</span>
					</button>
				</li>
			{/each}
		</ul>

		<h2>Prompts</h2>
		<ul>
			{#each PROMPT_FIXTURES as p (p.code)}
				<li>
					<button class:active={promptCode === p.code} onclick={() => (promptCode = p.code)}>
						<span class="code">{p.code}</span>
						<span class="label">{p.label}</span>
					</button>
				</li>
			{/each}
		</ul>

		<p class="lastevent">last event: <code>{lastEvent}</code></p>
	</aside>

	<main class="stage">
		<FlowShell
			backLabel={strings('onboarding.common.back')}
			canGoBack={view.can_go_back}
			onBack={() => log('go_back')}
			{railSlot}
		>
			{#if screen === 'form'}
				<NameScreen
					name={view.name}
					nameEditable={view.name_editable}
					nameTooLong={view.name_too_long}
					acks={view.acks}
					canSubmit={view.can_submit}
					busy={view.busy}
					submitLabel={strings(submitLabelToI18n(view.submit_label))}
					{statusText}
					showStartOver={view.show_start_over}
					{strings}
					privacyUrl="https://getvela.app/privacy"
					termsUrl="https://getvela.app/terms"
					onName={() => log('name_changed')}
					onToggleAck={(i) => log(`ack_toggled ${i}`)}
					onSubmit={() => log('submit')}
					onStartOver={() => log('start_over')}
				/>
			{:else if screen === 'keys'}
				<KeysScreen
					keys={view.keys}
					canAddKey={view.can_add_key}
					canFinish={view.can_finish}
					needsSecondKey={view.needs_second_key}
					busy={view.busy}
					maxKeys={7}
					addMethods={view.add_methods}
					signingDomain={view.signing_domain}
					signingPage={view.signing_page ?? null}
					{strings}
					onAddKey={(m) => log(`add_key ${m}`)}
					onConfirmKey={(i) => log(`confirm_key ${i}`)}
					onRemoveKey={(i) => log(`remove_key ${i}`)}
					onFinish={() => log('finish_keys')}
				/>
			{:else if screen === 'progress'}
				<ProgressScreen
					position={progressFor(view.status)!}
					keyCount={view.keys.length}
					{strings}
				/>
			{:else if screen === 'retry'}
				<RetryScreen
					detail={view.sync_error_detail}
					busy={view.busy}
					{strings}
					onRetry={() => log('retry_upload')}
					onStartOver={() => log('start_over')}
				/>
			{:else if screen === 'done' && coreReady}
				<DoneScreen
					address={view.address ?? ''}
					walletName={view.keys[0]?.name ?? view.name}
					keys={view.keys}
					{strings}
					onEnter={() => log('enter_wallet')}
				/>
			{:else if screen === 'done'}
				<p class="waiting">loading the core for the identicon…</p>
			{/if}
		</FlowShell>
	</main>
</div>

{#if prompt}
	<PromptSheet
		copy={promptCopy(prompt.kind, strings)}
		dismissLabel={strings('onboarding.common.back')}
		onAnswer={(accepted) => {
			log(`prompt_answered ${accepted}`);
			promptCode = null;
		}}
	/>
{/if}

<style>
	.gallery {
		display: grid;
		grid-template-columns: var(--layout-settingsNavW) 1fr;
		min-height: 100dvh;
		background: var(--color-bg-base);
		color: var(--color-fg-base);
	}

	.rail {
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
		padding: var(--space-3xl) var(--space-xl);
		border-right: var(--border-hairline) solid var(--color-border-base);
		background: var(--color-bg-sunken);
		overflow-y: auto;
	}

	.switches {
		display: flex;
		gap: var(--space-xl);
	}

	.switches label {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
		font-size: var(--text-sm);
		color: var(--color-fg-muted);
	}

	h2 {
		margin: 0;
		font-size: var(--text-sm);
		font-weight: var(--weight-semibold);
		letter-spacing: 0.06em;
		text-transform: uppercase;
		color: var(--color-fg-muted);
	}

	ul {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
		margin: 0;
		padding: 0;
		list-style: none;
	}

	ul button {
		display: flex;
		gap: var(--space-md);
		width: 100%;
		padding: var(--space-md) var(--space-lg);
		border: 0;
		border-radius: var(--radius-md);
		background: none;
		color: var(--color-fg-muted);
		font-family: var(--font-ui);
		font-size: var(--text-base);
		text-align: start;
		cursor: pointer;
	}

	ul button:hover {
		background: var(--color-bg-raised);
	}

	ul button.active {
		background: var(--color-accent-soft);
		color: var(--color-fg-base);
	}

	.code {
		flex: 0 0 2.5em;
		font-family: var(--font-mono);
		font-size: var(--text-sm);
	}

	.lastevent {
		margin: auto 0 0;
		color: var(--color-fg-subtle);
		font-size: var(--text-sm);
	}

	.stage {
		display: flex;
		flex-direction: column;
		padding: var(--space-4xl);
	}

	.waiting {
		color: var(--color-fg-subtle);
		font-size: var(--text-base);
	}
</style>
