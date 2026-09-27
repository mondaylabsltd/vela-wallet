<script lang="ts">
	/**
	 * Where the report's toast appears (078): mounted once, in the root layout,
	 * so a report whose sheet was closed mid-send says how it ended on whatever
	 * page the person went on to — Settings, the wallet, the address book.
	 *
	 * The live region is always in the page, and the toast is put INTO it: a
	 * region inserted together with its words is not reliably read out.
	 *
	 * A filed report's toast leaves after {@link LINGER_MS} — held while the
	 * pointer or focus is in it — and a fallback's stays until it is used or
	 * closed: "Open GitHub form" is the one road that report still has.
	 */
	import { onDestroy } from 'svelte';
	import Toast from '$lib/ui/Toast.svelte';
	import { reportToast } from '../report-toast.svelte';

	/** How long a filed report's toast stays, untouched. */
	const LINGER_MS = 8000;
	/** After the pointer or focus leaves it, a little longer to finish reading. */
	const AFTER_HOLD_MS = 4000;

	const current = $derived(reportToast.current);
	const anchor = $derived(reportToast.anchor);
	let timer: ReturnType<typeof setTimeout> | undefined;

	/** The anchor column's left edge and width, kept current as it moves. */
	let column = $state<{ left: number; width: number } | null>(null);
	/** The toast's own height (the region hugs it; 0 when there is none). */
	let regionHeight = $state(0);

	$effect(() => {
		const el = anchor;
		if (el === null) {
			column = null;
			return;
		}
		const measure = () => {
			const box = el.getBoundingClientRect();
			column = { left: box.left, width: box.width };
		};
		measure();
		const observer = new ResizeObserver(measure);
		observer.observe(el);
		window.addEventListener('resize', measure);
		return () => {
			observer.disconnect();
			window.removeEventListener('resize', measure);
		};
	});

	// While a toast shows, the column's scroll area makes room for it at its
	// end — its height and a gap — so nothing is left stuck underneath it.
	$effect(() => {
		const el = anchor;
		if (el === null || current === null || regionHeight === 0) return;
		el.style.setProperty('--toast-room', `calc(${regionHeight}px + var(--space-xl))`);
		return () => el.style.removeProperty('--toast-room');
	});

	function schedule(ms: number): void {
		clearTimeout(timer);
		const shown = reportToast.current;
		if (shown === null || shown.persistent) return;
		timer = setTimeout(() => {
			if (reportToast.current?.id === shown.id) reportToast.dismiss();
		}, ms);
	}

	$effect(() => {
		// A new toast (its id) starts its own clock.
		void current?.id;
		schedule(LINGER_MS);
		return () => clearTimeout(timer);
	});

	onDestroy(() => clearTimeout(timer));
</script>

<div
	class="region"
	class:anchored={column !== null}
	style:left={column === null ? undefined : `${column.left}px`}
	style:width={column === null ? undefined : `${column.width}px`}
	aria-live="polite"
	bind:clientHeight={regionHeight}
>
	{#if current !== null}
		{#key current.id}
			<Toast
				tone={current.tone}
				title={current.title}
				action={current.action}
				closeLabel={current.closeLabel}
				onclose={() => reportToast.dismiss()}
				onhold={(held) => (held ? clearTimeout(timer) : schedule(AFTER_HOLD_MS))}
			/>
		{/key}
	{/if}
</div>

<style>
	/* Above the phone's tab bar (and its safe area); near the bottom edge on a
	   desktop, where there is no bar. Centred — on the page's own column when
	   it names one (the desktop settings pane: never straddling the sidebar
	   and the nav), else on the window. The region itself takes no clicks —
	   only the toast in it does. */
	.region {
		position: fixed;
		inset-inline: 0;
		bottom: calc(var(--layout-tabBarHeight) + env(safe-area-inset-bottom, 0%) + var(--space-lg));
		z-index: 30;
		display: flex;
		justify-content: center;
		padding-inline: var(--space-lg);
		pointer-events: none;
	}

	/* Its width is the column's, padding included. */
	.region.anchored {
		box-sizing: border-box;
		inset-inline: auto;
	}

	@media (min-width: 1280px) {
		.region {
			bottom: var(--space-3xl);
		}
	}
</style>
