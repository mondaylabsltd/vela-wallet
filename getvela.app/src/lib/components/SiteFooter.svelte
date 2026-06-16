<script lang="ts">
	import { resolve } from '$app/paths';

	const year = new Date().getFullYear();
</script>

<footer class="site-footer">
	<div class="inner">
		<div class="left">
			<div class="brand">
				<img src="/vela-logo.png" alt="Vela" width="24" height="24" />
				<span>Vela Wallet</span>
			</div>
			<p class="copy">&copy; {year} MONDAY LABS LTD</p>
			<p class="tagline">Your keys. Your face. No seed phrase.</p>
		</div>

		<nav class="cols" aria-label="Footer">
			<div class="col">
				<h3>Product</h3>
				<a href="https://wallet.getvela.app/onboarding" target="_blank" rel="noopener">Open app</a>
				<a href={resolve('/docs')}>Docs</a>
				<a href={resolve('/roadmap')}>Roadmap</a>
			</div>
			<div class="col">
				<h3>Company</h3>
				<a href={resolve('/about')}>About</a>
				<a href={resolve('/blog')}>Blog</a>
				<a href={resolve('/docs/whitepaper')}>Whitepaper</a>
				<a href="https://github.com/atshelchin/vela-wallet" target="_blank" rel="noopener">GitHub</a
				>
			</div>
			<div class="col">
				<h3>Community</h3>
				<a href="https://x.com/realvelawallet" target="_blank" rel="noopener">X / Twitter</a>
				<a href="https://t.me/velawallet" target="_blank" rel="noopener">Telegram</a>
				<a href={resolve('/blog/rss.xml')}>RSS</a>
			</div>
			<div class="col">
				<h3>Legal</h3>
				<a href={resolve('/privacy')}>Privacy</a>
				<a href={resolve('/terms')}>Terms</a>
			</div>
		</nav>
	</div>
</footer>

<style>
	.site-footer {
		border-top: 1px solid var(--border);
		background: var(--bg);
		margin-top: 80px;
	}
	.inner {
		max-width: var(--max-w);
		margin: 0 auto;
		padding: 56px 24px 48px;
		display: flex;
		flex-wrap: wrap;
		gap: 48px;
		justify-content: space-between;
	}
	.left {
		max-width: 280px;
	}
	.brand {
		display: flex;
		align-items: center;
		gap: 8px;
		font-weight: 600;
		font-size: 1.05rem;
	}
	.brand img {
		border-radius: 6px;
	}
	.copy {
		margin-top: 14px;
		color: var(--text-secondary);
		font-size: 0.85rem;
	}
	.tagline {
		margin-top: 6px;
		color: var(--text-muted);
		font-size: 0.85rem;
	}
	.cols {
		display: flex;
		flex-wrap: wrap;
		gap: 40px;
	}
	.col {
		display: flex;
		flex-direction: column;
		gap: 10px;
		min-width: 110px;
	}
	.col h3 {
		font-size: 0.72rem;
		text-transform: uppercase;
		letter-spacing: 0.08em;
		color: var(--text-muted);
		font-weight: 600;
		margin-bottom: 2px;
	}
	.col a {
		font-size: 0.9rem;
		color: var(--text-secondary);
		transition: color 0.15s ease;
	}
	.col a:hover {
		color: var(--text);
	}

	@media (max-width: 560px) {
		.inner {
			padding: 40px 24px;
			gap: 36px;
		}
		.cols {
			gap: 32px;
		}
	}
</style>
