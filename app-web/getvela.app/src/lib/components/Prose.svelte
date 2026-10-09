<script lang="ts">
	import type { Snippet } from 'svelte';

	/**
	 * The reading surface for docs, blog posts and notes: one style for all
	 * three (spec 101, founder 2026-10-09, after bun.sh/guides). Running text in
	 * the reader's system face, a shade off full ink; headings in condensed
	 * Archivo; code in Martian Mono. The blog's serif body is gone with it:
	 * the three are read the same way, so they look the same.
	 */
	let { children }: { children: Snippet } = $props();
</script>

<div class="prose">
	{@render children()}
</div>

<style>
	.prose {
		font-family: var(--font-system);
		color: var(--ink-read);
		font-size: 1.0625rem;
		line-height: 1.75;
		word-wrap: break-word;
	}

	/* Headings — condensed Archivo, full ink, generous room above. */
	.prose :global(h1) {
		font-family: var(--font-display);
		font-stretch: 75%;
		font-size: 2.65rem;
		line-height: 1.06;
		font-weight: 800;
		letter-spacing: -0.01em;
		color: var(--text);
		margin: 0 0 0.6em;
	}
	.prose :global(h2) {
		font-family: var(--font-display);
		font-stretch: 75%;
		font-size: 1.75rem;
		line-height: 1.15;
		font-weight: 750;
		letter-spacing: -0.005em;
		color: var(--text);
		margin: 2.2em 0 0.6em;
		padding-bottom: 0.3em;
		border-bottom: 1px solid var(--border);
	}
	.prose :global(h3) {
		font-family: var(--font-display);
		font-stretch: 87.5%;
		font-size: 1.3rem;
		line-height: 1.25;
		font-weight: 700;
		color: var(--text);
		margin: 1.8em 0 0.5em;
	}
	.prose :global(h4) {
		font-family: var(--font-system);
		font-size: 1rem;
		font-weight: 600;
		margin: 1.6em 0 0.4em;
	}
	.prose :global(h2:first-child),
	.prose :global(h3:first-child) {
		margin-top: 0;
	}

	/* Text */
	.prose :global(p) {
		margin: 0 0 1.2em;
	}
	/* Links in running text are ink with a quiet underline: findable, without
	   pulling the eye off the sentence the way a coloured word does. */
	.prose :global(a) {
		color: var(--text);
		text-decoration: underline;
		text-decoration-color: color-mix(in srgb, var(--text) 30%, transparent);
		text-decoration-thickness: 1px;
		text-underline-offset: 3px;
		transition: text-decoration-color 0.15s ease;
	}
	.prose :global(a:hover) {
		text-decoration-color: var(--text);
	}
	.prose :global(strong) {
		color: var(--text);
		font-weight: 650;
	}
	.prose :global(small) {
		color: var(--text-secondary);
		font-size: 0.85em;
	}

	/* Lists */
	.prose :global(ul),
	.prose :global(ol) {
		margin: 0 0 1.2em;
		padding-left: 1.4em;
	}
	.prose :global(li) {
		margin: 0.4em 0;
	}
	.prose :global(li::marker) {
		color: var(--text-muted);
	}

	/* Quote */
	/* A quote is evidence, not decoration: a neutral rule and upright text. */
	.prose :global(blockquote) {
		margin: 1.4em 0;
		padding: 0.1em 1.1em;
		border-left: 3px solid var(--border-strong);
		color: var(--text-secondary);
	}
	.prose :global(blockquote p) {
		margin: 0.4em 0;
	}

	/* Inline code */
	.prose :global(:not(pre) > code) {
		font-family: var(--font-code);
		font-size: 0.8em;
		background: var(--code-inline-bg);
		border: 1px solid var(--border);
		border-radius: 5px;
		padding: 0.12em 0.4em;
		color: var(--text);
	}

	/* Code blocks (Shiki dual-theme output; colors resolve in tokens.css) */
	.prose :global(pre) {
		margin: 1.5em 0;
		padding: 16px 18px;
		border-radius: var(--radius);
		border: 1px solid var(--border);
		overflow-x: auto;
		font-size: max(0.86rem, var(--floor-meta));
		line-height: 1.6;
		tab-size: 2;
		-webkit-overflow-scrolling: touch;
	}
	.prose :global(pre code) {
		font-family: var(--font-code);
		background: none;
		border: none;
		padding: 0;
		font-size: inherit;
		color: inherit;
	}
	.prose :global(pre .line) {
		display: inline-block;
		width: 100%;
	}

	/* Media */
	.prose :global(img),
	.prose :global(video) {
		max-width: 100%;
		height: auto;
		border-radius: var(--radius);
		border: 1px solid var(--border);
		margin: 1.4em 0;
	}

	/* Tables — a step smaller than the running text: tabular data scans. */
	.prose :global(table) {
		width: 100%;
		border-collapse: collapse;
		margin: 1.5em 0;
		font-family: var(--font-system);
		font-size: max(0.92rem, var(--floor-note));
		line-height: 1.6;
		display: block;
		overflow-x: auto;
	}
	/* Rows, not a grid: horizontal hairlines only, so the eye runs along a row
	   instead of being boxed into cells. */
	.prose :global(th),
	.prose :global(td) {
		border: 0;
		border-bottom: 1px solid var(--border);
		padding: 10px 16px 10px 0;
		text-align: left;
		vertical-align: top;
	}
	.prose :global(th) {
		border-bottom-color: var(--border-strong);
		font-weight: 600;
		color: var(--text-secondary);
	}

	/* Rule */
	.prose :global(hr) {
		border: none;
		border-top: 1px solid var(--border);
		margin: 2.4em 0;
	}

	/* Keyboard */
	.prose :global(kbd) {
		font-family: var(--font-code);
		font-size: 0.8em;
		background: var(--bg-sunken);
		border: 1px solid var(--border-strong);
		border-bottom-width: 2px;
		border-radius: 5px;
		padding: 0.1em 0.45em;
	}
</style>
