<script lang="ts">
	import { onMount } from 'svelte';

	let { containerSelector = '.doc-body' }: { containerSelector?: string } = $props();

	type Heading = { id: string; text: string; level: number };

	let headings = $state<Heading[]>([]);
	let activeId = $state('');

	onMount(() => {
		const container = document.querySelector(containerSelector);
		if (!container) return;

		const els = Array.from(container.querySelectorAll('h2, h3')).filter(
			(el): el is HTMLElement => el instanceof HTMLElement && !!el.id
		);

		headings = els.map((el) => ({
			id: el.id,
			text: el.textContent?.trim() ?? '',
			level: el.tagName === 'H2' ? 2 : 3
		}));

		if (els.length === 0) return;

		const observer = new IntersectionObserver(
			(entries) => {
				for (const entry of entries) {
					if (entry.isIntersecting) activeId = entry.target.id;
				}
			},
			{ rootMargin: '-80px 0px -70% 0px', threshold: 0 }
		);
		for (const el of els) observer.observe(el);

		return () => observer.disconnect();
	});
</script>

{#if headings.length > 1}
	<nav class="toc" aria-label="On this page">
		<p class="toc-title">On this page</p>
		<ul>
			{#each headings as h (h.id)}
				<li class:sub={h.level === 3}>
					<a href={`#${h.id}`} class:active={activeId === h.id}>{h.text}</a>
				</li>
			{/each}
		</ul>
	</nav>
{/if}

<style>
	.toc {
		position: sticky;
		top: calc(var(--header-h) + 28px);
		font-size: 0.85rem;
	}
	.toc-title {
		font-size: 0.72rem;
		text-transform: uppercase;
		letter-spacing: 0.08em;
		color: var(--text-muted);
		font-weight: 600;
		margin-bottom: 12px;
	}
	ul {
		list-style: none;
		border-left: 1px solid var(--border);
	}
	li {
		margin: 0;
	}
	li a {
		display: block;
		padding: 5px 0 5px 14px;
		margin-left: -1px;
		border-left: 1px solid transparent;
		color: var(--text-secondary);
		line-height: 1.4;
		transition: color 0.15s ease;
	}
	li.sub a {
		padding-left: 28px;
		font-size: 0.82rem;
	}
	li a:hover {
		color: var(--text);
	}
	li a.active {
		color: var(--accent);
		border-left-color: var(--accent);
	}
</style>
