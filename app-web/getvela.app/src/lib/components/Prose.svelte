<script lang="ts">
	import type { Snippet } from 'svelte';

	/**
	 * `serif` switches the body copy to the reading serif — used for blog
	 * articles, where people read long passages. Docs keep the sans default
	 * for reference-style scanning; headings are serif in both.
	 */
	let { children, serif = false }: { children: Snippet; serif?: boolean } = $props();
</script>

<div class="prose" class:serif>
	{@render children()}
</div>

<style>
	.prose {
		color: var(--text);
		font-size: 1.0625rem;
		line-height: 1.75;
		word-wrap: break-word;
	}
	.prose.serif {
		font-family: var(--font-serif);
		font-size: 1.125rem;
		line-height: 1.8;
	}

	/* Headings — warm serif display, generous breathing room above. */
	.prose :global(h1) {
		font-family: var(--font-serif);
		font-size: 2.25rem;
		line-height: 1.2;
		font-weight: 600;
		letter-spacing: -0.015em;
		margin: 0 0 0.6em;
	}
	.prose :global(h2) {
		font-family: var(--font-serif);
		font-size: 1.55rem;
		font-weight: 600;
		letter-spacing: -0.01em;
		margin: 2.4em 0 0.7em;
		padding-bottom: 0.3em;
		border-bottom: 1px solid var(--border);
	}
	.prose :global(h3) {
		font-family: var(--font-serif);
		font-size: 1.25rem;
		font-weight: 600;
		margin: 1.9em 0 0.5em;
	}
	.prose :global(h4) {
		font-family: var(--font-sans);
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
	.prose :global(a) {
		color: var(--link);
		text-decoration: underline;
		text-decoration-color: color-mix(in srgb, var(--link) 35%, transparent);
		text-decoration-thickness: 1px;
		text-underline-offset: 3px;
		transition: text-decoration-color 0.15s ease;
	}
	.prose :global(a:hover) {
		text-decoration-color: var(--link);
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
	.prose :global(blockquote) {
		margin: 1.6em 0;
		padding: 0.2em 1.2em;
		border-left: 3px solid var(--accent);
		color: var(--text-secondary);
		font-style: italic;
	}
	.prose :global(blockquote p) {
		margin: 0.4em 0;
	}

	/* Inline code */
	.prose :global(:not(pre) > code) {
		font-family: var(--font-mono);
		font-size: 0.84em;
		background: var(--code-inline-bg);
		border: 1px solid var(--border);
		border-radius: 5px;
		padding: 0.12em 0.4em;
		color: var(--code-inline-text);
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
		font-family: var(--font-mono);
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

	/* Tables — sans even inside serif articles: tabular data scans better. */
	.prose :global(table) {
		width: 100%;
		border-collapse: collapse;
		margin: 1.5em 0;
		font-family: var(--font-sans);
		font-size: max(0.92rem, var(--floor-note));
		line-height: 1.6;
		display: block;
		overflow-x: auto;
	}
	.prose :global(th),
	.prose :global(td) {
		border: 1px solid var(--border);
		padding: 9px 13px;
		text-align: left;
	}
	.prose :global(th) {
		background: var(--bg-sunken);
		font-weight: 600;
	}

	/* Rule */
	.prose :global(hr) {
		border: none;
		border-top: 1px solid var(--border);
		margin: 2.4em 0;
	}

	/* Keyboard */
	.prose :global(kbd) {
		font-family: var(--font-mono);
		font-size: 0.8em;
		background: var(--bg-sunken);
		border: 1px solid var(--border-strong);
		border-bottom-width: 2px;
		border-radius: 5px;
		padding: 0.1em 0.45em;
	}
</style>
