<script lang="ts">
	import type { Snippet } from 'svelte';

	let { children }: { children: Snippet } = $props();
</script>

<div class="prose">
	{@render children()}
</div>

<style>
	.prose {
		color: var(--text);
		font-size: 1.02rem;
		line-height: 1.75;
		word-wrap: break-word;
	}

	/* Headings */
	.prose :global(h1) {
		font-size: 2rem;
		line-height: 1.2;
		font-weight: 700;
		letter-spacing: -0.02em;
		margin: 0 0 0.6em;
	}
	.prose :global(h2) {
		font-size: 1.5rem;
		font-weight: 700;
		letter-spacing: -0.01em;
		margin: 2.2em 0 0.7em;
		padding-bottom: 0.3em;
		border-bottom: 1px solid var(--border);
	}
	.prose :global(h3) {
		font-size: 1.2rem;
		font-weight: 650;
		margin: 1.8em 0 0.5em;
	}
	.prose :global(h4) {
		font-size: 1.05rem;
		font-weight: 600;
		margin: 1.5em 0 0.4em;
	}
	.prose :global(h2:first-child),
	.prose :global(h3:first-child) {
		margin-top: 0;
	}

	/* Text */
	.prose :global(p) {
		margin: 0 0 1.15em;
	}
	.prose :global(a) {
		color: var(--link);
		text-decoration: none;
		border-bottom: 1px solid color-mix(in srgb, var(--link) 35%, transparent);
		transition: border-color 0.15s ease;
	}
	.prose :global(a:hover) {
		border-bottom-color: var(--link);
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
		margin: 0 0 1.15em;
		padding-left: 1.4em;
	}
	.prose :global(li) {
		margin: 0.35em 0;
	}
	.prose :global(li::marker) {
		color: var(--text-muted);
	}

	/* Quote */
	.prose :global(blockquote) {
		margin: 1.5em 0;
		padding: 0.2em 1.1em;
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
		font-size: 0.86em;
		background: var(--bg-raised);
		border: 1px solid var(--border);
		border-radius: 5px;
		padding: 0.12em 0.4em;
		color: #f0b9a4;
	}

	/* Code blocks (Shiki output) */
	.prose :global(pre) {
		margin: 1.4em 0;
		padding: 16px 18px;
		border-radius: var(--radius);
		border: 1px solid var(--border);
		overflow-x: auto;
		font-size: 0.86rem;
		line-height: 1.6;
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

	/* Tables */
	.prose :global(table) {
		width: 100%;
		border-collapse: collapse;
		margin: 1.5em 0;
		font-size: 0.92rem;
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
		background: var(--bg-raised);
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
		background: var(--bg-raised);
		border: 1px solid var(--border-strong);
		border-bottom-width: 2px;
		border-radius: 5px;
		padding: 0.1em 0.45em;
	}
</style>
