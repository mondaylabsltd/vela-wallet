<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';

	let open = $state(false);

	const links = [
		{ href: '/blog', label: 'Blog' },
		{ href: '/docs', label: 'Docs' },
		{ href: '/about', label: 'About' }
	] as const;

	function isActive(href: string): boolean {
		const path = page.url.pathname;
		return path === href || path.startsWith(href + '/');
	}
</script>

<header class="site-header">
	<div class="bar">
		<a href={resolve('/')} class="logo" onclick={() => (open = false)}>
			<img src="/vela-logo.png" alt="Vela Wallet" width="28" height="28" />
			<span>Vela Wallet</span>
		</a>

		<nav class="links" class:open aria-label="Primary">
			{#each links as link (link.href)}
				<a
					href={resolve(link.href)}
					class:active={isActive(link.href)}
					onclick={() => (open = false)}
				>
					{link.label}
				</a>
			{/each}
			<a
				href="https://github.com/atshelchin/vela-wallet"
				target="_blank"
				rel="noopener"
				onclick={() => (open = false)}>GitHub</a
			>
			<a
				class="cta"
				href="https://wallet.getvela.app/onboarding?mode=create"
				target="_blank"
				rel="noopener"
				data-rybbit-event="cta_click"
				data-rybbit-prop-location="header"
				onclick={() => (open = false)}>Create wallet</a
			>
		</nav>

		<button
			class="menu"
			aria-label="Toggle menu"
			aria-expanded={open}
			onclick={() => (open = !open)}
		>
			<span></span><span></span><span></span>
		</button>
	</div>
</header>

<style>
	.site-header {
		position: sticky;
		top: 0;
		z-index: 50;
		background: rgba(15, 14, 12, 0.85);
		backdrop-filter: blur(12px);
		-webkit-backdrop-filter: blur(12px);
		border-bottom: 1px solid var(--border);
	}
	.bar {
		max-width: var(--max-w);
		margin: 0 auto;
		height: var(--header-h);
		padding: 0 24px;
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 16px;
	}
	.logo {
		display: flex;
		align-items: center;
		gap: 8px;
		font-weight: 700;
		font-size: 1.05rem;
		letter-spacing: -0.01em;
	}
	.logo img {
		border-radius: 7px;
	}
	.links {
		display: flex;
		align-items: center;
		gap: 26px;
	}
	.links a {
		font-size: 0.92rem;
		color: var(--text-secondary);
		transition: color 0.15s ease;
	}
	.links a:hover {
		color: var(--text);
	}
	.links a.active {
		color: var(--text);
	}
	.links a.cta {
		padding: 8px 16px;
		border-radius: var(--radius-sm);
		background: var(--accent);
		color: #fff;
		font-weight: 600;
	}
	.links a.cta:hover {
		background: var(--accent-hover);
	}
	.menu {
		display: none;
		flex-direction: column;
		justify-content: center;
		gap: 5px;
		width: 40px;
		height: 40px;
		background: none;
		border: none;
		cursor: pointer;
	}
	.menu span {
		display: block;
		height: 2px;
		width: 22px;
		margin: 0 auto;
		background: var(--text);
		border-radius: 2px;
	}

	@media (max-width: 820px) {
		.menu {
			display: flex;
		}
		.links {
			position: absolute;
			top: var(--header-h);
			left: 0;
			right: 0;
			flex-direction: column;
			align-items: stretch;
			gap: 0;
			padding: 8px 24px 20px;
			background: rgba(15, 14, 12, 0.98);
			border-bottom: 1px solid var(--border);
			display: none;
		}
		.links.open {
			display: flex;
		}
		.links a {
			padding: 13px 0;
			font-size: 1rem;
			border-bottom: 1px solid var(--border);
		}
		.links a.cta {
			margin-top: 14px;
			text-align: center;
			border-bottom: none;
		}
	}
</style>
