<script lang="ts">
	import { resolve } from '$app/paths';
	import SiteFooter from '$lib/components/SiteFooter.svelte';
	import SiteHeader from '$lib/components/SiteHeader.svelte';
</script>

<svelte:head>
	<title>Support — Vela Wallet</title>
	<meta
		name="description"
		content="Contact MONDAY LABS LTD, the maker of Vela Wallet: email, postal address, bug reports, community channels, and how to delete your data."
	/>
</svelte:head>

<SiteHeader />

<main class="container">
	<h1>Support</h1>
	<p class="lede">
		Vela Wallet is made by MONDAY LABS LTD. Here is how to reach us, report a problem, or find the
		community.
	</p>

	<section>
		<h2>Contact us</h2>
		<ul>
			<li>
				<strong>Email:</strong> <a href="mailto:hello@mondaylabs.ltd">hello@mondaylabs.ltd</a>
			</li>
			<li>
				<strong>Post:</strong> MONDAY LABS LTD, 61 Bridge Street, Kington, HR5 3DJ, United Kingdom
			</li>
			<li>
				<strong>Company:</strong> registered in England and Wales, company number 16988118
			</li>
		</ul>
	</section>

	<section>
		<h2>Report a problem</h2>
		<ul>
			<li>
				<strong>In the app:</strong> Settings → <strong>Send feedback</strong>. Describe what
				happened; the form shows the device details it adds before you send. Reports, and any
				screenshots you attach, become <strong>public</strong> issues on GitHub, so leave out anything
				you'd rather keep private.
			</li>
			<li>
				<strong>On GitHub:</strong> open an issue at
				<a href="https://github.com/mondaylabsltd/vela-wallet/issues" target="_blank" rel="noopener"
					>github.com/mondaylabsltd/vela-wallet/issues</a
				>.
			</li>
		</ul>
	</section>

	<section>
		<h2>Community</h2>
		<ul>
			<li>
				<strong>X:</strong>
				<a href="https://x.com/realvelawallet" target="_blank" rel="noopener">@realvelawallet</a>
			</li>
			<li>
				<strong>Telegram:</strong>
				<a href="https://t.me/velawallet" target="_blank" rel="noopener">@velawallet</a>
			</li>
			<li>
				<strong>Discord:</strong>
				<a href="https://discord.gg/23gWrtaYSa" target="_blank" rel="noopener"
					>discord.gg/23gWrtaYSa</a
				>
			</li>
		</ul>
	</section>

	<section>
		<h2>Help and policies</h2>
		<ul>
			<li><a href={resolve('/docs')}>Docs</a> and the <a href={resolve('/docs/faq')}>FAQ</a></li>
			<li>
				<a href={resolve('/delete')}>Delete your data</a> — what Vela keeps, for how long, and how to
				delete it
			</li>
			<li><a href={resolve('/privacy')}>Privacy Policy</a></li>
			<li><a href={resolve('/terms')}>Terms of Service</a></li>
		</ul>
	</section>
</main>

<SiteFooter />

<style>
	/* Same reading page as /privacy and /terms. */
	main.container {
		max-width: var(--max-w-prose);
		margin: 0 auto;
		padding: 48px 24px 80px;
	}

	h1 {
		font-size: 2rem;
		margin-bottom: 8px;
		letter-spacing: -0.02em;
	}
	.lede {
		margin-bottom: 40px;
	}

	section {
		margin-bottom: 40px;
	}
	h2 {
		font-size: 1.25rem;
		margin-bottom: 12px;
		text-align: left;
	}
	p {
		color: var(--text-secondary);
		line-height: 1.75;
		font-size: max(0.95rem, var(--floor-read));
		margin-bottom: 12px;
	}

	/* Every link here is a way to reach someone, so it looks like a link. */
	main a {
		color: var(--link);
		text-decoration: underline;
		text-decoration-color: color-mix(in srgb, var(--link) 35%, transparent);
		text-decoration-thickness: 1px;
		text-underline-offset: 3px;
		overflow-wrap: anywhere;
	}
	main a:hover {
		text-decoration-color: var(--link);
	}

	ul {
		list-style: none;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
	li {
		color: var(--text-secondary);
		font-size: max(0.95rem, var(--floor-read));
		line-height: 1.7;
		padding-left: 20px;
		position: relative;
	}
	li::before {
		content: '';
		position: absolute;
		left: 0;
		top: 10px;
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: var(--accent);
	}

	@media (max-width: 768px) {
		h1 {
			font-size: 1.5rem;
		}
	}
</style>
