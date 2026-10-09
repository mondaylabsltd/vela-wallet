<script lang="ts">
	/**
	 * The create journey, end to end.
	 *
	 * This component holds no flow state. It constructs the core, renders
	 * whatever view the core last emitted, and sends events back — the whole
	 * mapping from `CreateView` to a screen is the `screen` derivation below,
	 * and it is the only place that decides which step is showing.
	 *
	 * The wasm is fetched HERE, on mount, because reaching this component is
	 * exactly the moment a person committed to creating a wallet. The Welcome
	 * page that hosts it never loads it.
	 */
	import { onMount, type Snippet } from 'svelte';
	import FlowShell from './FlowShell.svelte';
	import { RAIL_STEPS, type RailSlot } from './rail';
	import NameScreen from './NameScreen.svelte';
	import KeysScreen from './KeysScreen.svelte';
	import ProgressScreen from './ProgressScreen.svelte';
	import DoneScreen from './DoneScreen.svelte';
	import RetryScreen from './RetryScreen.svelte';
	import { loadOnboardingCore } from '$lib/onboarding/core/wasm-client';
	import {
		createCreateWalletSession,
		type CreateWalletSession
	} from '$lib/onboarding/core/sessions';
	import { progressFor, statusKeyToI18n, submitLabelToI18n } from '$lib/onboarding/core/copy';
	import type { CreateView } from '$lib/onboarding/generated/CreateView';
	import type { KeyMethod } from '$lib/onboarding/generated/KeyMethod';
	import type { CompletionMode } from '$lib/onboarding/generated/CompletionMode';
	import type { PromptKind } from '$lib/onboarding/generated/PromptKind';
	import { track, type AnalyticsEventProps } from '$lib/analytics';
	import { analyticsMethod } from '$lib/analytics/methods';

	interface Props {
		strings: (key: string, params?: Record<string, string | number>) => string;
		privacyUrl: string;
		termsUrl: string;
		/** Raise a notice or a question; resolves to the answer. */
		prompt: (kind: PromptKind, confirmable: boolean) => Promise<boolean>;
		/** The wallet is real — hand it to the session and leave. */
		complete: (mode: CompletionMode) => Promise<void>;
		/** Leaving the flow entirely (back from the first step). */
		onExit: () => void;
		/** Drawn on the done screen, under the keys (the app suggestion). */
		doneAside?: Snippet;
	}

	let { strings, privacyUrl, termsUrl, prompt, complete, onExit, doneAside }: Props = $props();

	/**
	 * Usage statistics (`$lib/analytics`): the attempt starts at the first key
	 * minted, completes when the core says the wallet is real, and fails on a
	 * dismissed prompt, a publish that did not land, or a refusal — each named
	 * by kind and by the method of the key that was being made, never by more.
	 */
	let keyMethod: KeyMethod | null = null;
	let createStarted = false;
	let createDone = false;
	const failed = (reason: AnalyticsEventProps<'wallet_create_failed'>['reason']) =>
		track('wallet_create_failed', { method: analyticsMethod(keyMethod), reason });

	function noteView(prev: CreateView | null, next: CreateView): void {
		if (!createDone && next.stage === 'created') {
			createDone = true;
			track('wallet_create_completed', {
				method: analyticsMethod(next.keys[0]?.method ?? keyMethod)
			});
			return;
		}
		if (next.stage === 'sync_failed' && prev?.stage !== 'sync_failed') failed('network');
		if (
			next.status !== prev?.status &&
			(next.status === 'setup_cancelled' || next.status === 'verify_cancelled')
		)
			failed('cancelled');
	}

	function notePrompt(kind: PromptKind): void {
		if (kind.type === 'create_failed') failed('error');
		else if (kind.type === 'not_supported_create' || kind.type === 'incompatible_create')
			failed('unsupported');
		else if (kind.type === 'not_discoverable') failed('not_found');
	}

	function addKey(method: KeyMethod): void {
		keyMethod = method;
		if (!createStarted) {
			createStarted = true;
			track('wallet_create_started', { method: analyticsMethod(method) });
		}
		send({ type: 'add_key', name: '', method });
	}

	/** The founding-set cap, mirroring the core's `MAX_MULTI_KEYS`. */
	const MAX_KEYS = 7;

	let view = $state<CreateView | null>(null);
	let session: CreateWalletSession | null = null;
	let fatal = $state<string | null>(null);

	onMount(() => {
		let disposed = false;
		loadOnboardingCore()
			.then(() => {
				if (disposed) return;
				session = createCreateWalletSession({
					onView: (next) => {
						const prev = view;
						view = next;
						noteView(prev, next);
					},
					deps: {
						prompt: (kind, confirmable) => {
							notePrompt(kind);
							return prompt(kind, confirmable);
						},
						complete
					},
					onError: (error) => {
						fatal = error instanceof Error ? error.message : String(error);
						failed('error');
					}
				});
				// Spec 102: no `signing_page_chosen`. Settings no longer names a page
				// a create would follow, and the web offers no "Use my own signing
				// page" (it opens no page, owner 2026-09-23) — so every wallet made
				// here is a `getvela.app` wallet whose ceremonies run in the app.
				session.start({ type: 'start' });
			})
			.catch((error) => {
				if (!disposed) fatal = error instanceof Error ? error.message : String(error);
			});

		return () => {
			disposed = true;
			session?.dispose();
			session = null;
		};
	});

	/**
	 * Which screen is showing. The core's `stage` decides, with one refinement:
	 * a busy machine reporting a progress status has left the key list and is
	 * deriving, so the progress screen takes over until it lands.
	 */
	const screen = $derived.by(() => {
		if (!view) return 'loading' as const;
		if (view.stage === 'created') return 'done' as const;
		if (view.stage === 'sync_failed') return 'retry' as const;
		if (view.busy && progressFor(view.status)) return 'progress' as const;
		return view.stage === 'add_keys' ? ('keys' as const) : ('form' as const);
	});

	/**
	 * What the rail says beside the current screen. Three steps, and the
	 * product's own line outside them — so the journey reads brand → 01 → 02 →
	 * 03 → brand. DONE returns to the brand because by then nobody is asking
	 * where they are; they are asking what they got.
	 *
	 * RETRY is not a fourth step: it is the third one having failed to land,
	 * and the rail keeps saying so.
	 */
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
				return { kind: 'tagline', text: strings('onboarding.welcome.desktopTagline') };
		}
	});

	const statusText = $derived(
		view?.status && !progressFor(view.status) ? strings(statusKeyToI18n(view.status)) : undefined
	);

	function send(event: Parameters<CreateWalletSession['dispatch']>[0]) {
		session?.dispatch(event);
	}

	function back() {
		// The core owns whether there is anywhere to go back TO; leaving the
		// flow entirely is the host's, because the core has no idea what
		// contains it.
		if (view?.can_go_back) send({ type: 'go_back' });
		else onExit();
	}
</script>

<FlowShell
	backLabel={strings('onboarding.common.back')}
	canGoBack={screen !== 'progress'}
	onBack={back}
	{railSlot}
>
	{#if fatal}
		<p class="fatal" role="alert">{fatal}</p>
	{:else if !view}
		<p class="loading">{strings('onboarding.common.confirmInPrompt')}</p>
	{:else if screen === 'form'}
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
			{privacyUrl}
			{termsUrl}
			onName={(name) => send({ type: 'name_changed', name })}
			onToggleAck={(index) => send({ type: 'ack_toggled', index })}
			onSubmit={() => send({ type: 'submit' })}
			onStartOver={() => send({ type: 'start_over' })}
		/>
	{:else if screen === 'keys'}
		<KeysScreen
			keys={view.keys}
			canAddKey={view.can_add_key}
			canFinish={view.can_finish}
			needsSecondKey={view.needs_second_key}
			busy={view.busy}
			maxKeys={MAX_KEYS}
			addMethods={view.add_methods}
			signingDomain={view.signing_domain}
			signingPage={view.signing_page ?? null}
			{strings}
			onAddKey={addKey}
			onConfirmKey={(index) => send({ type: 'confirm_key', index })}
			onRemoveKey={(index) => send({ type: 'remove_key', index })}
			onFinish={() => send({ type: 'finish_keys' })}
		/>
	{:else if screen === 'progress'}
		<ProgressScreen position={progressFor(view.status)!} keyCount={view.keys.length} {strings} />
	{:else if screen === 'retry'}
		<RetryScreen
			detail={view.sync_error_detail}
			busy={view.busy}
			{strings}
			onRetry={() => send({ type: 'retry_upload' })}
			onStartOver={() => send({ type: 'start_over' })}
		/>
	{:else if screen === 'done'}
		<DoneScreen
			address={view.address ?? ''}
			walletName={view.keys[0]?.name ?? view.name}
			keys={view.keys}
			{strings}
			onEnter={() => send({ type: 'enter_wallet' })}
			aside={doneAside}
		/>
	{/if}
</FlowShell>

<style>
	.loading,
	.fatal {
		margin: 0;
		color: var(--color-fg-muted);
		font-size: var(--text-base);
	}

	.fatal {
		color: var(--color-error-base);
	}
</style>
