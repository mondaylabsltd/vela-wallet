<script lang="ts">
	/**
	 * The intro — three slides between the launch animation and the front door
	 * (spec 020).
	 *
	 * It is shown ONCE, on the first run, and it is the thing a person meets
	 * before they have any reason to trust us. So it argues rather than
	 * onboards: no seed phrase, the keys stay yours, one address everywhere.
	 * The last slide carries the two real ways in, because by then the argument
	 * is made and sending someone to a fourth screen that repeats it would be
	 * asking them to press the same button twice.
	 *
	 * Paging is a real drag, not a button that jumps: the track follows the
	 * finger and settles, so the dots are a readout of something the person did
	 * rather than a decoration. Below `prefers-reduced-motion` the settle is
	 * instant and the drag still works.
	 */
	import Button from '$lib/ui/Button.svelte';
	import OnboardingRail from '$lib/ui/onboarding/v2/OnboardingRail.svelte';
	import IntroSlide from './IntroSlide.svelte';
	import PageDots from './PageDots.svelte';
	import { INTRO_SLIDES } from '$lib/intro/slides';
	import { fillTemplate } from '$lib/i18n/fill';

	interface Props {
		/** Resolved corpus strings, dotted key → text. */
		strings: Readonly<Record<string, string>>;
		/**
		 * The rail's line at desktop widths — Welcome's own tagline, so the
		 * intro, Welcome and the create journey read as three screens of one
		 * app (spec 038). Below the breakpoint the rail is not drawn.
		 */
		tagline: string;
		/** Sign-in is running: the last slide's secondary button IS the spinner. */
		signingIn?: boolean;
		/** Href for "create a wallet" — a journey with a URL, so it stays a link. */
		createHref: string;
		/** Leave the intro without reading it. Marks it seen, same as finishing. */
		onSkip: () => void;
		onSignIn: () => void;
		/** Fires when the last slide's primary is pressed, before navigation. */
		onCreate: () => void;
	}

	let {
		strings,
		tagline,
		signingIn = false,
		createHref,
		onSkip,
		onSignIn,
		onCreate
	}: Props = $props();

	const t = (key: string) => strings[key] ?? key;

	let index = $state(0);
	/** Pixels the track is dragged from its resting position; 0 when settled. */
	let drag = $state(0);
	let dragging = $state(false);
	let viewport = $state<HTMLDivElement>();

	const total = INTRO_SLIDES.length;
	const last = $derived(index === total - 1);

	function go(next: number) {
		index = Math.min(Math.max(next, 0), total - 1);
	}

	// --- drag ---------------------------------------------------------------
	//
	// Pointer events rather than touch: the same code then answers a trackpad
	// drag and a stylus. `touch-action: pan-y` leaves vertical scrolling to the
	// browser, so a drag that is mostly downward never becomes a page turn.

	let startX = 0;
	let pointer: number | null = null;

	function down(event: PointerEvent) {
		if (event.button !== 0) return;
		pointer = event.pointerId;
		startX = event.clientX;
		dragging = true;
	}

	function move(event: PointerEvent) {
		if (pointer !== event.pointerId || !dragging) return;
		const width = viewport?.clientWidth ?? 1;
		const raw = event.clientX - startX;
		// Rubber-band at the two ends: there is nothing past them, and a track
		// that slides freely into empty space says there is.
		const overshoot = (index === 0 && raw > 0) || (last && raw < 0);
		drag = overshoot ? raw / 3 : raw;
		if (Math.abs(drag) > width) drag = Math.sign(drag) * width;
	}

	function up(event: PointerEvent) {
		if (pointer !== event.pointerId) return;
		const width = viewport?.clientWidth ?? 1;
		// A quarter of the width, or the gesture was a hesitation rather than a
		// page turn. The same threshold both ways, so it never feels lopsided.
		if (Math.abs(drag) > width / 4) go(index + (drag < 0 ? 1 : -1));
		drag = 0;
		dragging = false;
		pointer = null;
	}

	function key(event: KeyboardEvent) {
		if (event.key === 'ArrowRight') go(index + 1);
		else if (event.key === 'ArrowLeft') go(index - 1);
		else return;
		event.preventDefault();
	}
</script>

<svelte:window onkeydown={key} />

<main class="intro">
	<!-- Desktop widths only (the rail hides itself below 1280): the same rail
	     Welcome and the create journey stand beside, so the first screen a
	     desktop visitor meets is composed like the second (spec 038 SC-411). -->
	<OnboardingRail rail={{ kind: 'tagline', text: tagline }} />

	<div class="column">
		<!-- The escape hatch sits where a phone's thumb does not: this is the one
	     control on the screen that must not be pressed by accident. It goes
	     with the last slide, whose two buttons ARE the way out. -->
		<div class="header">
			{#if !last}
				<button class="skip" type="button" onclick={onSkip}>{t('onboarding.intro.skip')}</button>
			{/if}
		</div>

		<!-- `role="group"`: the drag is a redundant affordance — the button and the
	     arrow keys turn the page too — but a listener still owes the machine a
	     role, and this box really is one group of related content. -->
		<div
			class="viewport"
			role="group"
			bind:this={viewport}
			onpointerdown={down}
			onpointermove={move}
			onpointerup={up}
			onpointercancel={up}
		>
			<div
				class="track"
				class:dragging
				style:transform="translateX(calc({-index * 100}% + {drag}px))"
			>
				{#each INTRO_SLIDES as slide (slide.art)}
					<div class="cell">
						<IntroSlide art={slide.art} title={t(slide.titleKey)} body={t(slide.bodyKey)} />
					</div>
				{/each}
			</div>
		</div>

		<div class="footer">
			<PageDots
				{total}
				current={index}
				label={fillTemplate(t('onboarding.intro.pageOf'), { current: index + 1, total })}
			/>

			<div class="actions">
				{#if last}
					<Button variant="primary" shape="rounded" href={createHref} onclick={onCreate}>
						{t('onboarding.welcome.createWallet')}
					</Button>
					<Button variant="secondary" shape="rounded" loading={signingIn} onclick={onSignIn}>
						{t('onboarding.welcome.alreadyHaveWallet')}
					</Button>
				{:else}
					<Button variant="secondary" shape="rounded" onclick={() => go(index + 1)}>
						{t('onboarding.intro.next')}
					</Button>
				{/if}
			</div>
		</div>
	</div>
</main>

<style>
	.intro {
		display: flex;
		min-height: 100dvh;
		/* Rail and column together are capped and centred past the widest
		   the mocks were drawn for (spec 038 T078). */
		width: 100%;
		max-width: var(--layout-frameMax);
		margin-inline: auto;
		background: var(--color-bg-base);
	}

	/*
	 * PHONE WIDTHS: the column fills the height so the slides ride the middle
	 * and the dots and buttons ride the bottom, within a thumb — the layout
	 * spec 020 drew. Below the breakpoint the rail is not drawn, so this IS the
	 * page.
	 */
	.column {
		display: flex;
		flex: 1;
		flex-direction: column;
		min-width: 0;
		padding: var(--space-md) var(--layout-screenPaddingX) var(--space-3xl);
	}

	.header {
		display: flex;
		flex: none;
		justify-content: flex-end;
		/* A full tap target's worth of row whether or not the link is in it, so
		   the composition below does not jump on the last slide. */
		min-height: var(--size-hitTarget);
	}

	.skip {
		padding: 0 var(--space-md);
		border: none;
		background: none;
		color: var(--color-fg-muted);
		font-family: var(--font-ui);
		font-size: var(--text-lg);
		line-height: var(--leading-normal);
		cursor: pointer;
	}

	.skip:hover {
		opacity: var(--opacity-hover);
	}

	.skip:active {
		transform: scale(var(--motion-press-button));
	}

	/*
	 * The slides run full-bleed inside a padded page, so the outgoing slide
	 * leaves at the screen edge rather than at the text margin.
	 */
	.viewport {
		flex: 1;
		min-height: 0;
		margin-inline: calc(var(--layout-screenPaddingX) * -1);
		overflow: hidden;
		touch-action: pan-y;
	}

	.track {
		display: flex;
		height: 100%;
		transition: transform var(--motion-duration-slow) cubic-bezier(0.22, 1, 0.36, 1);
	}

	/* While a finger is down the track IS the finger — a transition here would
	   make it lag behind by exactly its duration. */
	.track.dragging {
		transition: none;
	}

	.cell {
		display: flex;
		flex: 0 0 100%;
		align-items: center;
		justify-content: center;
		padding-inline: var(--layout-screenPaddingX);
	}

	.footer {
		display: flex;
		flex: none;
		flex-direction: column;
		gap: var(--space-lg);
	}

	.actions {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
	}

	@media (prefers-reduced-motion: reduce) {
		.track {
			transition: none;
		}
	}

	/* ------------------------------------------------------------------ */
	/* Desktop: the rail on the left, the column beside it at its NATURAL   */
	/* height — the same composition as Welcome and the create journey.    */
	/* Stretching the phone layout to a desktop window is what opened the  */
	/* hole in the middle of the page and made it read as a phone screen   */
	/* pulled tall (spec 038 finding 1).                                    */
	/* ------------------------------------------------------------------ */
	@media (min-width: 1280px) {
		.column {
			flex: 0 1 auto;
			/* Centred beside the rail at its NATURAL height. The flex default
			   (`stretch`) is what pulled the column to the viewport's bottom in
			   the first pass — the hole, again. */
			align-self: center;
			justify-content: center;
			/* box-sizing is border-box globally, so the measure carries its own
			   padding — the onboarding column plus a gutter a side, exactly as
			   Welcome and FlowShell size theirs. */
			max-width: calc(var(--layout-onboardingColumn) + var(--layout-onboardingFrameGutter) * 2);
			padding: var(--space-5xl) var(--layout-onboardingFrameGutter);
		}

		/* The slides no longer need to bleed to the screen edge: the column
		   has a gutter, and the outgoing slide leaves at the column's edge. */
		.viewport {
			flex: 0 0 auto;
			margin-inline: 0;
		}

		.cell {
			padding-inline: 0;
		}

		.footer {
			margin-block-start: var(--space-4xl);
		}

		/* Side by side, each at ITS LABEL'S width — a desktop dialog sizes a
		   button to what it says; a full-width button is a phone's answer to
		   a thumb. */
		.actions {
			flex-direction: row;
		}

		.actions :global(.button) {
			flex: 0 0 auto;
			width: auto;
			min-width: var(--layout-welcomeCtaMin);
		}
	}
</style>
