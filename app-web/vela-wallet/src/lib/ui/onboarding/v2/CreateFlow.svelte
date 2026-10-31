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
	import { markHistory } from '$lib/ui/history';
	import { onMount, untrack, type Snippet } from 'svelte';
	import FlowShell from './FlowShell.svelte';
	import { RAIL_STEPS, type RailSlot } from './rail';
	import NameScreen from './NameScreen.svelte';
	import KeysScreen from './KeysScreen.svelte';
	import ProgressScreen from './ProgressScreen.svelte';
	import DoneScreen from './DoneScreen.svelte';
	import RetryScreen from './RetryScreen.svelte';
	import { loadOnboardingCore } from '$lib/onboarding/core/wasm-client';
	import { loadAccounts } from '$lib/onboarding/core/storage';
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
	import { SIGNING_SURFACE } from '$lib/signing/surface';
	import { harnessSigningPage } from '$lib/dev/harness-page';

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

	/**
	 * The wallets already on this device, by name (ob-12): a second wallet
	 * under a name already taken is allowed, and said, so the two can still
	 * be told apart in the switcher. Read once — creating does not add one
	 * until it is done.
	 */
	const takenNames: string[] = (() => {
		try {
			return loadAccounts().map((account) => account.name);
		} catch {
			return [];
		}
	})();

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
				// Spec 102: no `signing_page_chosen` — the web offers no "Use my own
				// signing page", so every wallet made here is a `getvela.app`
				// wallet. Its ceremonies run in the app, except where this build's
				// pages cannot use a passkey (the Safari package, R8): there the
				// core sends each one to the official signing page.
				session.start({ type: 'start' });
				send({ type: 'surface_known', surface: SIGNING_SURFACE });
				// The device harness's loopback page (development package only).
				const harnessPage = harnessSigningPage();
				if (harnessPage) send({ type: 'signing_page_chosen', url: harnessPage });
				// ob-9: a reload (or a return) mid-create finds the name and the
				// agreements where they were — one step back from the key list,
				// whose passkeys lived only in the page that minted them.
				const kept = readForm();
				if (kept) {
					if (kept.name) send({ type: 'name_changed', name: kept.name });
					kept.acks.forEach((checked, index) => {
						if (checked) send({ type: 'ack_toggled', index });
					});
				}
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

	/**
	 * ob-9: the form, kept for this tab (sessionStorage — never the passkeys
	 * or anything signed, only what the person typed and ticked). Cleared once
	 * the wallet exists — saved on this device is existing (ob-2), whether or
	 * not its sync has landed.
	 */
	const FORM_KEY = 'vela.create.form';

	function readForm(): { name: string; acks: boolean[] } | null {
		try {
			const raw = sessionStorage.getItem(FORM_KEY);
			if (!raw) return null;
			const parsed = JSON.parse(raw) as { name?: unknown; acks?: unknown };
			return {
				name: typeof parsed.name === 'string' ? parsed.name : '',
				acks: Array.isArray(parsed.acks) ? parsed.acks.map((ack) => ack === true) : []
			};
		} catch {
			return null;
		}
	}

	$effect(() => {
		if (!view) return;
		try {
			if (view.stage === 'created' || view.stage === 'sync_failed')
				sessionStorage.removeItem(FORM_KEY);
			else sessionStorage.setItem(FORM_KEY, JSON.stringify({ name: view.name, acks: view.acks }));
		} catch {
			// Storage refused: a reload starts blank, as it always did.
		}
	});

	/**
	 * ob-9: the key list is a step in the browser's history too, so Back from
	 * it returns to the name (kept) instead of leaving the flow. `pushState`
	 * without a URL, as the wallet's own flows do (`flows/nav.svelte.ts`).
	 * The entry is taken back once the list is left by other means — and
	 * before the wallet replaces this page, so Back from the wallet never
	 * lands on a blank form.
	 */
	let stepMarked = false;

	/** ob-2: how often the retry screen has come back — its next wait is longer. */
	let retryRound = $state(0);
	let lastScreen: string | null = null;
	$effect(() => {
		const current = screen;
		if (lastScreen === 'retry' && current === 'progress') untrack(() => (retryRound += 1));
		lastScreen = current;
	});
	$effect(() => {
		if (screen === 'keys' && !stepMarked) {
			markHistory({ velaCreate: 'keys' });
			stepMarked = true;
		} else if ((screen === 'form' || screen === 'done') && stepMarked) {
			stepMarked = false;
			history.back();
		}
	});

	function onPopState() {
		if (!stepMarked) return;
		stepMarked = false;
		if (view?.can_go_back) send({ type: 'go_back' });
	}

	/**
	 * Keys exist that only this page holds (the list, the publish, the retry
	 * screen): a reload or a closed tab would drop them, so the browser asks
	 * first (ob-2, ob-9).
	 */
	function onBeforeUnload(event: BeforeUnloadEvent) {
		if (!view || view.keys.length === 0 || view.stage === 'created') return;
		event.preventDefault();
	}

	function send(event: Parameters<CreateWalletSession['dispatch']>[0]) {
		session?.dispatch(event);
	}

	function back() {
		// ob-6: the wallet exists and is saved — Back from "Wallet created"
		// goes where the person now lives, not to the landing page.
		if (screen === 'done') return send({ type: 'enter_wallet' });
		// The core owns whether there is anywhere to go back TO; leaving the
		// flow entirely is the host's, because the core has no idea what
		// contains it.
		if (view?.can_go_back) send({ type: 'go_back' });
		else onExit();
	}
</script>

<svelte:window onpopstate={onPopState} onbeforeunload={onBeforeUnload} />

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
			nameBytes={view.name_bytes}
			nameLimit={view.name_limit}
			{takenNames}
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
			addHeadingKey={view.add_heading_key}
			keyCountShown={view.key_count_shown}
			methodsPinned={view.methods_pinned}
			signingDomain={view.signing_domain}
			signingPage={view.signing_page ?? null}
			{strings}
			onAddKey={addKey}
			onConfirmKey={(index) => send({ type: 'confirm_key', index })}
			onRemoveKey={(index) => send({ type: 'remove_key', index })}
			onFinish={() => send({ type: 'finish_keys' })}
			canCancel={view.can_cancel}
			onCancel={() => send({ type: 'cancel_pending' })}
			{statusText}
		/>
	{:else if screen === 'progress'}
		<ProgressScreen position={progressFor(view.status)!} keyCount={view.keys.length} {strings} />
	{:else if screen === 'retry'}
		<RetryScreen
			round={retryRound}
			detail={view.sync_error_detail}
			busy={view.busy}
			{strings}
			onRetry={() => send({ type: 'retry_upload' })}
			onEnter={() => send({ type: 'enter_wallet' })}
			showStartOver={view.show_start_over}
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
		font-size: calc(var(--text-base) * var(--text-scale, 1));
	}

	.fatal {
		color: var(--color-error-base);
	}
</style>
