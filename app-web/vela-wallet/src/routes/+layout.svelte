<script lang="ts">
	import '@fontsource/plus-jakarta-sans/400.css';
	import '@fontsource/plus-jakarta-sans/500.css';
	import '@fontsource/plus-jakarta-sans/600.css';
	import '@fontsource/plus-jakarta-sans/700.css';
	import '@fontsource/noto-sans-sc/400.css';
	import '@fontsource/noto-sans-sc/500.css';
	import '@fontsource/noto-sans-sc/700.css';
	import '@fontsource/ibm-plex-mono/400.css';
	import '@fontsource/ibm-plex-mono/500.css';
	import '$lib/tokens/tokens.css';
	import '../app.css';
	import { onMount } from 'svelte';
	import LaunchAnimation from '$lib/launch/LaunchAnimation.svelte';
	import { markPlayed, shouldPlay, type Appearance } from '$lib/launch/constants';
	import { page } from '$app/state';
	import { resolve } from '$app/paths';
	import { afterNavigate, beforeNavigate, goto } from '$app/navigation';
	import { updated } from '$app/state';
	import { ensureCustomNetworks } from '$lib/services/networks';
	import { invalidateAllPools } from '$lib/services/rpc-pool';
	import { normalizePackagedUrl } from '$lib/extension/page-url';
	import ParallelSpaceBadge from '$lib/dev/ParallelSpaceBadge.svelte';
	import ReportToastHost from '$lib/settings/ui/ReportToastHost.svelte';
	import ExtensionNotices from '$lib/extension/ExtensionNotices.svelte';
	import type { ExtensionMessages } from '$lib/extension/messages';
	import { parallelFlagSet } from '$lib/dev/parallel-flag.svelte';
	import { isPanelDocument, panelNeedsWallet, panelSurface } from '$lib/dapp/panel-surface.svelte';
	import { inExtension } from '$lib/dapp/transport';
	import { session } from '$lib/session/core/session.svelte';
	import { localeOfPath } from '$lib/i18n/locales';
	import { SITE_ORIGIN } from '$lib/site';

	let { children } = $props();

	/**
	 * The offline line's words. The layout has no corpus of its own (it is the
	 * one route outside `[locale]`), so the sentence is the settings screen's
	 * `offline` string, resolved by the page data when a locale page is up and
	 * English otherwise.
	 */
	const offlineLabel = $derived(
		(page.data as { flow?: Record<string, string> } | undefined)?.flow?.[
			'onboarding.common.networkBody'
		] ?? 'The request never arrived — check your network and try again.'
	);

	/**
	 * Spec 027 D42. Under the packaged extension a route path is not a file, so
	 * after a client navigation the address bar names a document that does not
	 * exist and a reload dies on `chrome-error://`. Put the document's own name
	 * back. Identity on the hosted site — decided by this page's origin, not by
	 * a build flag.
	 */
	afterNavigate(() => normalizePackagedUrl());

	/**
	 * Page analytics: Rybbit, the cookieless counter getvela.app uses
	 * (`tj.appsdata.org`, site 51bb55d72d55), declared in the privacy policy.
	 * Only on the hosted wallet itself — never in the extension, whose build
	 * drops this (MV3 refuses remote code, and so does the store), and never
	 * on a local, preview or e2e origin, which would write test runs into the
	 * production account and hold `networkidle` open.
	 */
	onMount(() => {
		if (__VELA_EXTENSION__ || location.origin !== SITE_ORIGIN) return;
		const script = document.createElement('script');
		script.src = 'https://tj.appsdata.org/api/script.js';
		script.dataset.siteId = '51bb55d72d55';
		script.defer = true;
		document.head.appendChild(script);
	});

	/**
	 * The locale this document is in — from its path, not `page.params`, which
	 * a page the packaged extension loads fresh does not have yet: a Japanese
	 * side panel read 'en' there and was sent to the ENGLISH wallet the moment
	 * a request was owed, its sheet with it (issue 317, `localeOfPath`).
	 */
	const locale = $derived(page.params.locale ?? localeOfPath(page.url.pathname) ?? 'en');

	const parallelHref = $derived(resolve('/[locale]/parallel', { locale }));

	// Spec 012. Client-only by construction: `launching` starts false, so the
	// prerendered HTML contains no overlay and the page is complete without it
	// — it can never be the LCP element, and a visitor without scripting sees
	// the normal page (spec Edge Cases).
	let launching = $state(false);
	let pageOpacity = $state(1);
	let appearance = $state<Appearance>('dark');

	/**
	 * Spec 038 (Part B): the browser's own word on connectivity. Offline is
	 * named on screen instead of surfacing as a dozen RPC endpoints banned on
	 * their cooldown schedule; a reconnect lifts the pools so the next read
	 * does not wait out a ban earned while the cable was unplugged.
	 */
	let offline = $state(false);

	/**
	 * A deploy while the wallet is open: the next lazy chunk would be a 404
	 * and the app broken until somebody thinks to reload. SvelteKit polls the
	 * version (`kit.version.pollInterval` in vite.config.ts); when it moved,
	 * the next navigation is a full document load rather than a client one.
	 */
	beforeNavigate(({ to, willUnload }) => {
		if (updated.current && to?.url && !willUnload) location.href = to.url.href;
	});

	/**
	 * Spec 082 RB9: the extension's side panel keeps its port to the worker for
	 * as long as the document lives — started HERE, above every route, so
	 * Wallet → Settings → Wallet does not drop it — and a request the worker
	 * says this window owes brings the panel back to the wallet, which is where
	 * a request is answered.
	 *
	 * Every fact is read on every run (G55: the effect used to read a field
	 * that was not reactive first, return while it was null, and never run
	 * again — a request that arrived on Settings waited for a tap).
	 */
	const walletHref = $derived(resolve('/[locale]/wallet', { locale }));
	$effect(() => {
		const needed = panelNeedsWallet({
			caller: panelSurface.caller,
			current: panelSurface.current,
			routeId: page.route.id,
			// Only where the core lets the app be: with no wallet yet the wallet
			// route sends the panel back to Welcome — a loop, not a request.
			allowedRoute: session.view.allowed_route
		});
		if (needed) void goto(walletHref);
	});

	/**
	 * Spec 082 RJ20 (G58): in the extension, the worker's snapshot and every
	 * connected site follow the signed-in account from EVERY screen — a switch
	 * made in Settings (切换账户) reached no site while this lived in the
	 * wallet page — and from the first session this document settles on (spec
	 * 086, issue 315). Grants written before 082 get the core's EIP-55
	 * spelling at boot, whichever screen the wallet opens on. Both are the dApp
	 * channel's code, so they are loaded only inside the extension: Welcome
	 * never carries them (`budgets.e2e.ts`).
	 */
	onMount(() => {
		if (!inExtension()) return;
		let stop: (() => void) | undefined;
		let gone = false;
		void import('$lib/dapp/follow').then((follow) => {
			if (gone) return;
			void follow.normalizeGrantSpelling();
			stop = $effect.root(() => {
				$effect(() => {
					void follow.sessionFollower.note(session.view, { locale: page.params.locale });
				});
			});
		});
		return () => {
			gone = true;
			stop?.();
		};
	});

	/**
	 * Issue 317: the side panel opens in the person's language (its doorway,
	 * `panel.js`), but a panel already OPEN when the language is changed in the
	 * wallet tab kept drawing every sheet in the old one. It goes back through
	 * its doorway — once it owes nothing and shows nothing over the wallet: a
	 * sheet a person is reading is never redrawn under them, and a request is
	 * never dropped for it. The rule is the doorways' own
	 * (`extension/lib/locales.js`), loaded only by a panel that heard a write.
	 */
	onMount(() => {
		if (!isPanelDocument()) return;
		let retry: ReturnType<typeof setTimeout> | undefined;
		const chrome = (
			globalThis as {
				chrome?: {
					i18n?: { getUILanguage?: () => string };
					runtime?: { getURL?: (path: string) => string };
				};
			}
		).chrome;
		const follow = async (key: string | null) => {
			const doors = await import('../../extension/lib/locales.js');
			if (key !== null && key !== doors.LANGUAGE_KEY) return;
			clearTimeout(retry);
			const step = doors.panelLanguageStep({
				pinned: doors.pinnedLanguage(),
				uiLanguage: chrome?.i18n?.getUILanguage?.(),
				current: locale,
				busy:
					panelSurface.current !== null ||
					document.querySelector('[role="dialog"], [data-testid="dapp-receipt"]') !== null
			});
			if (step === 'wait') retry = setTimeout(() => void follow(null), 2000);
			else if (step === 'reopen' && chrome?.runtime?.getURL) {
				location.replace(chrome.runtime.getURL(doors.PANEL_DOOR));
			}
		};
		const heard = (event: StorageEvent) => void follow(event.key);
		window.addEventListener('storage', heard);
		return () => {
			window.removeEventListener('storage', heard);
			clearTimeout(retry);
		};
	});

	onMount(() => {
		if (isPanelDocument()) void panelSurface.start();
		// The custom-network snapshot, warmed once per document (spec 038 #E9)
		// so send, signing and the RPC pool see an added network on a fresh
		// load, not only after the wallet page or a Settings write.
		void ensureCustomNetworks();
		offline = typeof navigator !== 'undefined' && navigator.onLine === false;
		const wentOffline = () => (offline = true);
		const cameBack = () => {
			offline = false;
			// Bans and cooldowns earned while offline are not facts about the
			// endpoints; drop them so the first read after reconnect is live.
			invalidateAllPools();
		};
		window.addEventListener('offline', wentOffline);
		window.addEventListener('online', cameBack);
		// A lazy import that failed to fetch (a deploy moved the hashes) is
		// unrecoverable in place; the document is.
		const preloadFailed = () => location.reload();
		window.addEventListener('vite:preloadError', preloadFailed);

		// The dev/e2e console (spec 025) is a DYNAMIC import behind its gate:
		// a static one would drag the core's JS glue into the first-paint
		// chunk of every page, Welcome included.
		try {
			if (import.meta.env.DEV || localStorage.getItem('vela.dev.console') === '1') {
				void import('$lib/services/dev-console').then((m) => m.maybeInstallDevConsole());
			}
		} catch {
			// storage denied — no console, no harm
		}
		// The parallel space re-arms itself on EVERY boot, gate or no gate
		// (spec 026 US4): a reload inside it must stay inside it, and a wallet
		// wearing fixture keys must never boot unmarked. The fixture signer and
		// its key material stay behind this dynamic import, so a real visit
		// never loads them.
		if (parallelFlagSet()) {
			void import('$lib/dev/parallel-space').then((m) => {
				void m.applyParallelSpaceOnBoot();
				m.installParallelConsole();
			});
		}
		// The inline script in app.html already made this decision before paint
		// and recorded it on <html>. Reading it back — rather than re-deciding —
		// is what guarantees the two cannot disagree and leave the page hidden
		// behind an animation that never starts.
		if (document.documentElement.dataset.launch !== 'playing') return;
		if (!shouldPlay()) return;
		markPlayed();
		// The effective appearance the rest of the page already resolves, not the
		// raw OS setting (FR-009): tokens.css keys off `data-theme` first and
		// `prefers-color-scheme` second, so read it the same way round.
		const pinned = document.documentElement.dataset.theme;
		appearance =
			pinned === 'light' || pinned === 'dark'
				? (pinned as Appearance)
				: matchMedia('(prefers-color-scheme: light)').matches
					? 'light'
					: 'dark';
		pageOpacity = 0;
		launching = true;
	});
</script>

<!--
	One continuous surface. The page content and the launch screen sit on this
	exact colour, which is what lets them cross-dissolve without a washed-out
	middle where both are half-transparent over the bare document (FR-012).
-->
<div class="surface">
	<div class="page" data-launch-page style:opacity={pageOpacity}>
		{#if offline}
			<p class="offline" role="status">{offlineLabel}</p>
		{/if}
		<!-- The packaged extension's own notices (spec 094); nothing elsewhere. -->
		<ExtensionNotices
			messages={(page.data as { extension?: ExtensionMessages } | undefined)?.extension}
		/>
		{@render children()}
	</div>

	<ParallelSpaceBadge onopen={() => goto(parallelHref)} />

	<!-- A bug report whose sheet was closed mid-send says how it ended here,
	     on whatever page the person went on to (078). -->
	<ReportToastHost />

	{#if launching}
		<LaunchAnimation
			{appearance}
			onprogress={(value) => (pageOpacity = value)}
			onfinished={() => {
				pageOpacity = 1;
				launching = false;
				// Release the pre-paint hold, or the CSS rule keeps the page at
				// opacity 0 for the rest of the visit.
				delete document.documentElement.dataset.launch;
			}}
		/>
	{/if}
</div>

<style>
	.surface {
		min-height: 100dvh;
		background: oklch(from var(--color-bg-base) calc(l + 0.02) c h);
	}

	.page {
		/* Driven per-frame by the overlay's dissolve, so no CSS transition here —
		   two animations on one property would fight. */
		min-height: 100dvh;
	}
	.offline {
		margin: 0;
		padding: var(--space-sm) var(--layout-screenPaddingX);
		background: var(--color-warning-soft, var(--color-bg-sunken));
		color: var(--color-warning-base);
		font-size: var(--text-sm);
		text-align: center;
	}
</style>
