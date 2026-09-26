<script lang="ts">
	/**
	 * Welcome — the v2 design (spec 019).
	 *
	 * One column at every width: brand, headline, and the two ways in. Desktop
	 * gets a wider column and a side-by-side button row; below the breakpoint
	 * the column narrows and the buttons stack. Nothing reflows into a second
	 * pane, because there is no second pane — the flow this page starts is a
	 * full page of its own.
	 *
	 * Creating a wallet NAVIGATES: it is a stepped journey, so it owns a URL and
	 * back works. Signing in has no steps — one system passkey sheet and you are
	 * either in or you are not — so it runs here, in place, and speaks only
	 * through the button's busy state and the failure sheet.
	 *
	 * This page is also the site's landing page: prerendered in 15 locales with
	 * canonical + hreflang. The wasm the flow needs is fetched by the flow, not
	 * by this page — `e2e/welcome-ssr.e2e.ts` holds that line.
	 *
	 * On a FIRST run this page opens on the intro instead (spec 020): three
	 * slides that argue the product before asking for anything. It is decided
	 * in `onMount` and never on the server, for the same reason the launch
	 * animation is — the prerendered HTML is one document served to everybody,
	 * and what it must contain is the landing page. The intro's own two CTAs
	 * reach the same `signIn` and the same create href as the ones below it, so
	 * there is one implementation of "the two ways in", not two.
	 */
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import type { PageProps } from './$types';
	import BrandMark from '$lib/ui/BrandMark.svelte';
	import Button from '$lib/ui/Button.svelte';
	import OnboardingRail from '$lib/ui/onboarding/v2/OnboardingRail.svelte';
	import IntroCarousel from '$lib/ui/intro/IntroCarousel.svelte';
	import AddMethodPicker from '$lib/ui/onboarding/v2/AddMethodPicker.svelte';
	import Sheet from '$lib/ui/onboarding/Sheet.svelte';
	import SocialMeta from '$lib/ui/SocialMeta.svelte';
	import type { KeyMethod } from '$lib/onboarding/generated/KeyMethod';
	import { markIntroSeen, shouldShowIntro } from '$lib/intro/gate';
	import PromptSheet from '$lib/ui/onboarding/v2/PromptSheet.svelte';
	import { fillTemplate } from '$lib/i18n/fill';
	import { SUPPORTED_LOCALES, FALLBACK_LOCALE } from '$lib/i18n/locales';
	import { SITE_ORIGIN } from '$lib/site';
	import { loadOnboardingCore } from '$lib/onboarding/core/wasm-client';
	import { createLoginSession, type LoginSession } from '$lib/onboarding/core/sessions';
	import { promptCopy, type PromptCopy } from '$lib/onboarding/core/copy';
	import { session } from '$lib/session/core/session.svelte';
	import type { CompletionMode } from '$lib/onboarding/generated/CompletionMode';
	import type { LoginView } from '$lib/onboarding/generated/LoginView';
	import type { PromptKind } from '$lib/onboarding/generated/PromptKind';

	let { data }: PageProps = $props();

	const m = $derived(data.messages);
	const locale = $derived(data.locale);

	/** Serialized flow copy from the layout load; numbers filled here. */
	const strings = (key: string, params?: Record<string, string | number>) =>
		fillTemplate(data.flow[key] ?? key, params);

	const createHref = $derived(resolve('/[locale]/create', { locale }));
	const walletHref = $derived(resolve('/[locale]/wallet', { locale }));

	let loginView = $state<LoginView | null>(null);
	let login: LoginSession | null = null;
	let pending = $state<{ copy: PromptCopy; resolve: (accepted: boolean) => void } | null>(null);

	/**
	 * The press has been accepted but the core is not up yet.
	 *
	 * `loginView.busy` cannot cover this window: the view does not exist until
	 * the 3.4 MB wasm has been fetched and the machine constructed, so the
	 * SLOWEST part of signing in was also the only part with no feedback at all
	 * — and the guard below could not hold, which let a second press build a
	 * second login session.
	 */
	let starting = $state(false);
	/**
	 * Spec 048: the core refused this browser's stored records (the retired
	 * client's spelling, or plain damage). The loop answered the machine with
	 * its failure; this is the sentence and the two ways out.
	 */
	let storageFault = $state<string | null>(null);
	const storageCopy: PromptCopy = $derived({
		title: strings('onboarding.storage.unreadableTitle'),
		message: strings('onboarding.storage.unreadableBody'),
		confirm: {
			confirmLabel: strings('onboarding.storage.resetCopy'),
			cancelLabel: strings('onboarding.storage.signInAgain')
		}
	});
	async function answerStorage(reset: boolean) {
		storageFault = null;
		session.fault = null;
		login?.dispose();
		login = null;
		loginView = null;
		starting = false;
		if (reset) await session.resetLocalCopy();
	}

	/**
	 * The sign-in method picker is open (spec 038 finding 20). The web used to
	 * dispatch `platform` and let the browser's own sheet cover the rest; a
	 * wallet living on a phone or a key deserves a row that says so — the same
	 * three rows creating a wallet shows, and the desktop's sign-in shows.
	 */
	let methodsOpen = $state(false);

	/**
	 * The first-run intro is up. Starts false so the prerendered document is the
	 * landing page; `onMount` raises it when this browser has not seen it.
	 */
	let intro = $state(false);

	function leaveIntro() {
		markIntroSeen();
		intro = false;
	}

	const signingIn = $derived(starting || (loginView?.busy ?? false));

	/**
	 * The registry is unreachable. Sign-in stays attemptable — the core decides
	 * that, not this screen — so this only surfaces the warning.
	 */
	const endpointUnreachable = $derived(loginView?.endpoint_unreachable ?? false);
	/**
	 * The probe could not get out of THIS machine (spec 038). A browser cannot
	 * usually tell this apart from the service being down — only the desktop
	 * produces it today — but the sentence is different, so it is read here too.
	 */
	const transportFailed = $derived(loginView?.transport_failed ?? false);

	function prompt(kind: PromptKind): Promise<boolean> {
		return new Promise((settle) => {
			pending = { copy: promptCopy(kind, strings), resolve: settle };
		});
	}

	async function complete(mode: CompletionMode): Promise<void> {
		await session.boot();
		session.accountEstablished(mode);
		// Signing in ends where the wallet is, not back on the page that
		// started it — the same landing all three native clients make.
		await goto(walletHref, { replaceState: true });
	}

	/**
	 * Sign-in loads the core on FIRST USE, never on mount: this page is
	 * prerendered and must stay wasm-free until someone commits. The health
	 * probe the core starts is part of that commitment.
	 */
	async function signIn(method: KeyMethod) {
		if (signingIn) return;
		methodsOpen = false;
		starting = true;
		try {
			if (!login) {
				await loadOnboardingCore();
				login = createLoginSession({
					onView: (next) => (loginView = next),
					deps: { prompt, complete },
					onError: (error) => {
						console.error('[login] core fault:', error);
						storageFault = error instanceof Error ? error.message : String(error);
						starting = false;
					}
				});
				login.start({ type: 'start' });
			}
			login.dispatch({ type: 'sign_in', method });
		} finally {
			// Handed over to `loginView.busy` — or released, if the core never
			// came up, so the button can be pressed again.
			starting = false;
		}
	}

	onMount(() => {
		if (shouldShowIntro()) intro = true;
		// The inline script in app.html hid Welcome before paint on the same
		// rule; whichever way the gate went, the decision is now this
		// component's, and the attribute must not outlive it.
		delete document.documentElement.dataset.intro;
		return () => {
			login?.dispose();
			login = null;
		};
	});
</script>

<svelte:head>
	<title>{m.metaTitle}</title>
	<meta name="description" content={m.metaDescription} />
	<link rel="canonical" href="{SITE_ORIGIN}/{locale}" />
	{#each SUPPORTED_LOCALES as alternate (alternate)}
		<link rel="alternate" hreflang={alternate} href="{SITE_ORIGIN}/{alternate}" />
	{/each}
	<link rel="alternate" hreflang="x-default" href="{SITE_ORIGIN}/{FALLBACK_LOCALE}" />
</svelte:head>

<SocialMeta
	{locale}
	title={m.metaTitle}
	description={m.metaDescription}
	url="{SITE_ORIGIN}/{locale}"
/>

{#if intro}
	<IntroCarousel
		strings={data.intro}
		tagline={strings('onboarding.welcome.desktopTagline')}
		{signingIn}
		{createHref}
		onSkip={leaveIntro}
		onCreate={leaveIntro}
		onSignIn={() => {
			// Seen: whichever way this ends, they have read it. Marking it on the
			// press rather than on success means a cancelled passkey prompt drops
			// them on Welcome, not back into the introduction they just read —
			// where the method picker is waiting, open.
			leaveIntro();
			methodsOpen = true;
		}}
	/>
{:else}
	<main class="welcome" data-intro-page>
		<OnboardingRail
			rail={{ kind: 'tagline', text: strings('onboarding.welcome.desktopTagline') }}
		/>

		<div class="column">
			<div class="top">
				<!-- The rail carries the brand at desktop widths; below the breakpoint
			     there is no rail, and it belongs here as it always did. -->
				<header class="brand">
					<BrandMark size={60} />
					<span class="wordmark">VELA WALLET</span>
				</header>

				<div class="hero">
					<h1 class="headline" class:long={m.heroTitleFit === 'long'}>{m.heroTitle}</h1>
					<p class="sub">{m.heroSubtitle}</p>
				</div>
			</div>

			<div class="actions">
				<Button variant="primary" shape="rounded" disabled={signingIn} href={createHref}>
					{m.createWallet}
				</Button>
				<Button
					variant="secondary"
					shape="rounded"
					loading={signingIn}
					onclick={() => (methodsOpen = !methodsOpen)}
				>
					{m.alreadyHaveWallet}
				</Button>
			</div>

			{#if transportFailed}
				<p class="endpointWarning" role="status">
					{strings('onboarding.common.networkBody')}
				</p>
			{:else if endpointUnreachable}
				<p class="endpointWarning" role="status">
					{strings('onboarding.settings.warningText')}
				</p>
			{/if}
		</div>
	</main>
{/if}

{#if methodsOpen}
	<!-- The three ways in, in a sheet (a dialog on desktop): a wallet on a
	     phone or a security key is reachable by name, not only through
	     whatever the browser's own sheet defaults to (spec 038 SC-428;
	     founder: 弹框 on phone web and desktop web alike). -->
	<Sheet label={strings('onboarding.login.header')} onClose={() => (methodsOpen = false)}>
		<div class="methodsSheet">
			<h2 class="methodsTitle">{strings('onboarding.login.header')}</h2>
			<AddMethodPicker open={true} {strings} onPick={(method) => void signIn(method)} />
		</div>
	</Sheet>
{/if}

<!-- Spec 075: signing in can run on the Trusted Signer's page — here is where
     the person is asked where it is, and where they compare the code. -->

{#if pending}
	<PromptSheet
		copy={pending.copy}
		dismissLabel={strings('onboarding.common.back')}
		onAnswer={(accepted) => {
			pending?.resolve(accepted);
			pending = null;
		}}
	/>
{/if}

{#if storageFault !== null || session.fault !== null}
	<PromptSheet
		copy={storageCopy}
		dismissLabel={strings('onboarding.storage.signInAgain')}
		onAnswer={(accepted) => void answerStorage(accepted)}
	/>
{/if}

<style>
	.welcome {
		display: flex;
		justify-content: center;
		min-height: 100dvh;
		padding: var(--space-4xl) var(--layout-screenPaddingX) var(--space-5xl);
		background: var(--color-bg-base);
	}

	/* The rail brings its own padding and has to reach both edges, so the page
	   gives up its own once the rail is showing. */
	@media (min-width: 1280px) {
		.welcome {
			justify-content: flex-start;
			padding: 0;
			/* Rail and column together, capped and centred past the widest the
			   mocks were drawn for (spec 038 T078). */
			width: 100%;
			max-width: var(--layout-frameMax);
			margin-inline: auto;
		}
	}

	/*
	 * PHONE WIDTHS: `space-between`, so the brand and headline sit at the top
	 * of the frame and the two ways in ride the bottom, within a thumb. That
	 * only works if the column fills the height — centre it and space-between
	 * has nothing to distribute, which is how the first pass ended up with the
	 * buttons riding up under the subtitle.
	 *
	 * At desktop widths it is centred instead, beside the rail. Stretching this
	 * rule to a desktop window is what opened the hole in the middle of the
	 * page and made it read like a phone screen pulled tall.
	 */
	.column {
		display: flex;
		flex: 1;
		flex-direction: column;
		justify-content: space-between;
		gap: var(--space-5xl);
		width: 100%;
		max-width: var(--layout-flowColumn);
	}

	.top {
		display: flex;
		flex-direction: column;
		gap: var(--space-3xl);
	}

	.brand {
		display: flex;
		gap: var(--space-lg);
		align-items: center;
	}

	.wordmark {
		color: var(--color-fg-base);
		font-size: var(--text-xl);
		font-weight: var(--weight-bold);
		letter-spacing: 0.11em;
	}

	.hero {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
	}

	.headline {
		margin: 0;
		color: var(--color-fg-base);
		font-size: var(--text-heroCompact);
		font-weight: var(--weight-bold);
		/* The design breaks the headline across two lines — three where a
		   locale's sentence fits no two-line split (fr, id, ja, ru). The break
		   lives in the corpus as a newline rather than as markup, so each
		   locale picks its own — a Chinese line length is not a German one. */
		white-space: pre-line;
		line-height: var(--leading-tight);
		letter-spacing: -0.02em;
	}

	/* A locale whose headline is too wide for its rung drops one step down the
	   ladder — 46 → 38 → 31 — rather than wrapping into a line it did not
	   author. Which locales those are is not guessed here: the corpus
	   carries `heroTitleFit` beside the string it describes. */
	.headline.long {
		font-size: var(--text-heroTight);
	}

	/* Two sentences that overrun the column by a word or two. Wrapped greedily
	   they leave that word alone on the last line («… never goes to / Vela.»,
	   «…ことはあ / りません。»); balanced, the lines come out near-even — the
	   desktop does the same with `balanced_wrap_width`. Japanese breaks at
	   phrase boundaries and Korean between words, as the site's headings do. */
	.sub {
		margin: 0;
		color: var(--color-fg-muted);
		font-size: var(--text-lg);
		line-height: var(--leading-normal);
		text-wrap: balance;
	}

	:global(html[lang='ja']) .sub {
		word-break: auto-phrase;
	}

	:global(html[lang='ko']) .sub {
		word-break: keep-all;
	}

	.actions {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
	}

	/* No padding of its own any more: `Sheet` pads its content, and this rule
	   was the workaround that proved it should. */

	.methodsTitle {
		margin: 0 0 var(--space-lg);
		color: var(--color-fg-base);
		font-size: var(--text-xl);
		font-weight: var(--weight-bold);
	}

	.endpointWarning {
		margin: 0;
		color: var(--color-warning-base);
		font-size: var(--text-base);
		line-height: var(--leading-normal);
	}

	/* ------------------------------------------------------------------ */
	/* Desktop: a rail on the left, and the two ways in side by side.      */
	/* ------------------------------------------------------------------ */

	@media (min-width: 1280px) {
		/* Beside the rail: left-aligned, vertically centred, and at its natural
		   height. The mobile layout anchors the two ways in to the bottom of the
		   viewport, which is right on a phone and opens a hole on a desktop. */
		.column {
			flex: 0 1 auto;
			justify-content: center;
			gap: 0;
			/* box-sizing is border-box globally, so the measure has to carry its own
			   padding — 520 of text plus 72 a side. */
			max-width: calc(var(--layout-onboardingColumn) + var(--layout-onboardingFrameGutter) * 2);
			margin-inline: 0;
			padding: var(--space-5xl) var(--layout-onboardingFrameGutter);
		}

		/* The rail has it. */
		.brand {
			display: none;
		}

		.headline {
			font-size: var(--text-hero);
		}

		.headline.long {
			font-size: var(--text-heroCompact);
		}

		/* Side by side, each at ITS LABEL'S width. A desktop dialog sizes a
		   button to what it says; a full-width button is a phone's answer to a
		   thumb, and two of them stacked is what made this read as a phone. */
		.actions {
			flex-direction: row;
			margin-block-start: var(--space-4xl);
		}

		.actions :global(.button) {
			flex: 0 0 auto;
			/* The base button is a phone control and fills its column; here each
			   way in is sized to its label (the row's whole point), so the
			   inherited width: 100% has to be handed back. */
			width: auto;
			min-width: var(--layout-welcomeCtaMin);
		}
	}
</style>
