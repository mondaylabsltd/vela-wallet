<script lang="ts">
	import type { Snippet } from 'svelte';

	type Variant = 'tip' | 'info' | 'warning' | 'danger';

	let {
		type = 'info',
		title,
		children
	}: { type?: Variant; title?: string; children: Snippet } = $props();

	const defaults: Record<Variant, string> = {
		tip: 'Tip',
		info: 'Note',
		warning: 'Warning',
		danger: 'Caution'
	};
</script>

<aside class="callout {type}">
	<p class="title">{title ?? defaults[type]}</p>
	<div class="body">
		{@render children()}
	</div>
</aside>

<style>
	.callout {
		margin: 1.5em 0;
		padding: 14px 16px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--border);
		border-left-width: 3px;
		background: var(--bg-raised);
	}
	.title {
		margin: 0 0 4px;
		font-weight: 650;
		font-size: 0.85rem;
		letter-spacing: 0.01em;
	}
	.body :global(p:last-child) {
		margin-bottom: 0;
	}
	.body :global(p) {
		margin: 0 0 0.6em;
		font-size: 0.95rem;
		color: var(--text-secondary);
	}

	.tip {
		border-left-color: #3fb950;
	}
	.tip .title {
		color: #3fb950;
	}
	.info {
		border-left-color: #58a6ff;
	}
	.info .title {
		color: #58a6ff;
	}
	.warning {
		border-left-color: #d29922;
	}
	.warning .title {
		color: #e3b341;
	}
	.danger {
		border-left-color: var(--accent);
	}
	.danger .title {
		color: var(--accent-hover);
	}
</style>
