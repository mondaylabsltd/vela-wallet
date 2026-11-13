<script lang="ts">
	import { untrack } from 'svelte';
	import type { CodeLine } from '../model';

	interface Props {
		lines: CodeLine[];
		/** "(80 字节)" — the size, when the content is a truncated blob. */
		note?: string;
		/**
		 * cs-10: whether the block's last line has NOT been in sight yet — said
		 * when that changes, and `false` once it has been seen (or the block
		 * goes). A block that fits its window says nothing at all. The core's
		 * gate holds the confirm while it is `true` (`ConfirmBlock::MessageUnread`).
		 */
		onunread?: (unread: boolean) => void;
	}

	let { lines, note, onunread }: Props = $props();

	/** Between two lines: drawn by the <pre>, as written in the text. */
	const BREAK = '\n';
	/** The end of the text, after its last line. */
	let end = $state<HTMLElement>();
	/** A different message is a new reading; the same one redrawn is not. */
	const content = $derived(JSON.stringify(lines));
	/** Asked to watch — not WHICH callback: a parent's new closure is no new reading. */
	const watching = $derived(onunread !== undefined);

	/** The nearest box that scrolls this one — the sheet's body — or `null`: the window. */
	function scrollerOf(node: HTMLElement): HTMLElement | null {
		for (let at = node.parentElement; at; at = at.parentElement) {
			const { overflowY } = getComputedStyle(at);
			if (overflowY === 'auto' || overflowY === 'scroll' || overflowY === 'overlay') return at;
		}
		return null;
	}

	$effect(() => {
		void content;
		const target = end;
		if (!watching || !target) return;
		const report = (unread: boolean) => untrack(() => onunread)?.(unread);
		const scroller = scrollerOf(target);
		let said = false;
		let seen = false;
		const tell = (unread: boolean) => {
			if (unread === said) return;
			said = unread;
			report(unread);
		};
		/*
		 * Read once its end has been at or above the bottom of the window it
		 * scrolls in. Both are measured on the screen, so whatever moves the
		 * whole sheet (its entry, a drag) moves both and changes nothing.
		 */
		const check = () => {
			if (seen) return;
			const bottom = scroller ? scroller.getBoundingClientRect().bottom : window.innerHeight;
			seen = target.getBoundingClientRect().bottom <= bottom + 1;
			tell(!seen);
		};
		check();
		const where: HTMLElement | Window = scroller ?? window;
		where.addEventListener('scroll', check, { passive: true });
		window.addEventListener('resize', check);
		const sizes = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(check);
		sizes?.observe(target.parentElement ?? target);
		if (scroller) sizes?.observe(scroller);
		return () => {
			where.removeEventListener('scroll', check);
			window.removeEventListener('resize', check);
			sizes?.disconnect();
			tell(false);
		};
	});
</script>

<!-- Breaks only INSIDE tags, on purpose: within <pre> every space and break
     between them is drawn. -->
<pre
	class="code">{#each lines as line, i (i)}{#if i > 0}{BREAK}{/if}{#if typeof line === 'string'}{line}{:else}{#each line as run, j (j)}{#if run.muted}<span
						class="muted">{run.text}</span
					>{:else}{run.text}{/if}{/each}{/if}{/each}<span class="end" bind:this={end}
	></span>{#if note}
		{note}{/if}</pre>

<style>
	.code {
		margin: 0;
		padding: var(--space-xl);
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
		font-family: var(--font-mono);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-relaxed);
		color: var(--color-fg-base);
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}
	/* What the core put there for the eye — a hidden character's ⟨U+…⟩, a
	   folded run of empty lines — never mistaken for the text itself. */
	.muted {
		color: var(--color-fg-subtle);
	}
</style>
