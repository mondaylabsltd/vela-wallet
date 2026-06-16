<script lang="ts">
	import { resolve } from '$app/paths';
	import SiteHeader from '$lib/components/SiteHeader.svelte';
	import SiteFooter from '$lib/components/SiteFooter.svelte';
	import Seo from '$lib/components/Seo.svelte';
	import { seoConfig } from '$lib/seo';

	const values = [
		{
			title: 'Self-custody, for real',
			body: 'Your keys, your coins — not a slogan but the architecture. We cannot move, freeze, or recover your funds, and we built it that way on purpose.'
		},
		{
			title: 'No seed phrases',
			body: 'The biggest cause of lost crypto is a string of words people were told to guard perfectly. We replaced it with a passkey: your face or fingerprint.'
		},
		{
			title: 'Open source',
			body: 'The wallet is public on GitHub. Trust should be verifiable, not asked for. Read the code, or follow along as we build it in the open.'
		},
		{
			title: 'Honest about trade-offs',
			body: "Every design choice gives something up. We write down what, and why — in the docs and on the blog — instead of pretending there's no cost."
		}
	];

	const team = [
		{
			name: 'Shelchin',
			role: 'Founder & Engineer',
			bio: 'Builds Vela end to end — the wallet, the contracts, and this site. Writing about the process as it happens.',
			initials: 'S',
			links: [
				{ label: 'GitHub', href: 'https://github.com/atshelchin' },
				{ label: 'X', href: 'https://x.com/realvelawallet' }
			]
		}
	];

	const jsonLd = [
		{
			'@context': 'https://schema.org',
			'@type': 'AboutPage',
			name: 'About Vela Wallet',
			url: `${seoConfig.domain}/about`
		},
		{
			'@context': 'https://schema.org',
			'@type': 'Organization',
			name: seoConfig.siteName,
			legalName: 'MONDAY LABS LTD',
			url: seoConfig.domain,
			logo: `${seoConfig.domain}/vela-logo.png`,
			sameAs: [
				'https://github.com/atshelchin/vela-wallet',
				'https://x.com/realvelawallet',
				'https://t.me/velawallet'
			]
		}
	];
</script>

<Seo
	title="About"
	description="The team and mission behind Vela Wallet — a self-custodial, open-source wallet with no seed phrase, built in the open."
	canonical="/about"
	{jsonLd}
/>

<SiteHeader />

<main>
	<section class="hero">
		<p class="eyebrow">About Vela</p>
		<h1>A wallet you can trust because you can verify it.</h1>
		<p class="lede">
			Vela is a self-custodial wallet for ETH and ERC-20 tokens with no seed phrase. We're a small
			team building it in the open — code, decisions, and mistakes included.
		</p>
	</section>

	<section class="story">
		<h2>Why we built Vela</h2>
		<p>
			Self-custody is supposed to mean freedom: no company between you and your money. In practice,
			it has meant handing normal people a twelve-word secret and telling them to guard it
			perfectly, forever. Most people can't, and a staggering amount of crypto has been lost to
			exactly that.
		</p>
		<p>
			We thought the trade-off was wrong. You shouldn't have to choose between real ownership and a
			wallet you can actually use safely. So we built Vela around passkeys — the same hardware-backed
			security that protects Apple Pay and your phone's unlock — and removed the seed phrase
			entirely.
		</p>
		<p>
			The result is a wallet where signing a transaction feels like unlocking your phone, your keys
			never leave your device's secure hardware, and there's nothing for a scammer to phish out of
			you.
		</p>
	</section>

	<section class="values">
		<h2>What we believe</h2>
		<div class="value-grid">
			{#each values as value (value.title)}
				<div class="value-card">
					<h3>{value.title}</h3>
					<p>{value.body}</p>
				</div>
			{/each}
		</div>
	</section>

	<section class="team">
		<h2>The team</h2>
		<p class="team-intro">
			Vela is built by a small, hands-on team. We answer for our own work — find us on GitHub or X.
		</p>
		<div class="team-grid">
			{#each team as member (member.name)}
				<div class="member">
					<div class="avatar" aria-hidden="true">{member.initials}</div>
					<div class="member-body">
						<h3>{member.name}</h3>
						<p class="role">{member.role}</p>
						<p class="bio">{member.bio}</p>
						<div class="member-links">
							{#each member.links as link (link.href)}
								<a href={link.href} target="_blank" rel="noopener">{link.label}</a>
							{/each}
						</div>
					</div>
				</div>
			{/each}
		</div>
	</section>

	<section class="opensource">
		<div class="os-card">
			<h2>Built in the open</h2>
			<p>
				Vela is open source. The wallet, the smart contracts, and this website are public — so you
				don't have to take our word for any of it. Read the code, open an issue, or follow the
				build on the blog.
			</p>
			<div class="os-links">
				<a class="btn primary" href="https://github.com/atshelchin/vela-wallet" target="_blank" rel="noopener">
					View on GitHub
				</a>
				<a class="btn" href={resolve('/blog')}>Read the blog</a>
				<a class="btn" href={resolve('/docs')}>Read the docs</a>
			</div>
		</div>
	</section>

	<section class="cta">
		<h2>Your keys. Your face. No seed phrase.</h2>
		<p>Create a self-custodial wallet in under a minute.</p>
		<a
			class="btn primary lg"
			href="https://wallet.getvela.app/onboarding?mode=create"
			target="_blank"
			rel="noopener"
			data-rybbit-event="cta_click"
			data-rybbit-prop-location="about"
		>
			Create wallet
		</a>
	</section>
</main>

<SiteFooter />

<style>
	main {
		max-width: 860px;
		margin: 0 auto;
		padding: 0 24px;
	}
	section {
		padding: 56px 0;
		border-bottom: 1px solid var(--border);
	}
	section:last-of-type {
		border-bottom: none;
	}
	h2 {
		font-size: 1.7rem;
		font-weight: 700;
		letter-spacing: -0.02em;
		margin-bottom: 20px;
	}

	.hero {
		padding-top: 72px;
	}
	.eyebrow {
		font-size: 0.8rem;
		text-transform: uppercase;
		letter-spacing: 0.1em;
		color: var(--accent);
		font-weight: 600;
		margin-bottom: 16px;
	}
	.hero h1 {
		font-size: 2.8rem;
		line-height: 1.1;
		font-weight: 700;
		letter-spacing: -0.03em;
		max-width: 16ch;
	}
	.lede {
		margin-top: 20px;
		font-size: 1.2rem;
		line-height: 1.6;
		color: var(--text-secondary);
		max-width: 56ch;
	}

	.story p {
		font-size: 1.05rem;
		line-height: 1.75;
		color: var(--text-secondary);
		margin-bottom: 1.1em;
		max-width: 64ch;
	}

	.value-grid {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: 18px;
	}
	.value-card {
		padding: 22px 24px;
		border: 1px solid var(--border);
		border-radius: var(--radius);
		background: var(--bg-card);
	}
	.value-card h3 {
		font-size: 1.1rem;
		font-weight: 650;
		margin-bottom: 8px;
	}
	.value-card p {
		color: var(--text-secondary);
		font-size: 0.96rem;
		line-height: 1.6;
	}

	.team-intro {
		color: var(--text-secondary);
		margin-bottom: 28px;
		max-width: 56ch;
	}
	.team-grid {
		display: grid;
		grid-template-columns: 1fr;
		gap: 18px;
	}
	.member {
		display: flex;
		gap: 20px;
		padding: 24px;
		border: 1px solid var(--border);
		border-radius: var(--radius);
		background: var(--bg-card);
	}
	.avatar {
		flex-shrink: 0;
		width: 56px;
		height: 56px;
		border-radius: 50%;
		display: grid;
		place-items: center;
		font-size: 1.4rem;
		font-weight: 700;
		color: #fff;
		background: linear-gradient(135deg, var(--accent), #b8431f);
	}
	.member-body h3 {
		font-size: 1.15rem;
		font-weight: 650;
	}
	.role {
		color: var(--accent);
		font-size: 0.88rem;
		font-weight: 500;
		margin: 2px 0 8px;
	}
	.bio {
		color: var(--text-secondary);
		font-size: 0.96rem;
		line-height: 1.6;
	}
	.member-links {
		display: flex;
		gap: 16px;
		margin-top: 12px;
	}
	.member-links a {
		font-size: 0.85rem;
		color: var(--text-secondary);
		border-bottom: 1px solid var(--border);
		padding-bottom: 1px;
	}
	.member-links a:hover {
		color: var(--accent);
		border-color: var(--accent);
	}

	.os-card {
		padding: 36px;
		border: 1px solid var(--border-accent);
		border-radius: var(--radius);
		background: var(--accent-soft);
	}
	.os-card p {
		color: var(--text-secondary);
		font-size: 1.02rem;
		line-height: 1.65;
		max-width: 60ch;
		margin-bottom: 22px;
	}
	.os-links {
		display: flex;
		flex-wrap: wrap;
		gap: 12px;
	}

	.btn {
		display: inline-block;
		padding: 10px 20px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--border-strong);
		background: var(--bg-raised);
		color: var(--text);
		font-weight: 600;
		font-size: 0.92rem;
		transition:
			border-color 0.15s ease,
			background 0.15s ease;
	}
	.btn:hover {
		border-color: var(--text-muted);
	}
	.btn.primary {
		background: var(--accent);
		border-color: var(--accent);
		color: #fff;
	}
	.btn.primary:hover {
		background: var(--accent-hover);
		border-color: var(--accent-hover);
	}
	.btn.lg {
		padding: 14px 30px;
		font-size: 1rem;
	}

	.cta {
		text-align: center;
		padding: 72px 0 88px;
	}
	.cta p {
		color: var(--text-secondary);
		margin-bottom: 28px;
		font-size: 1.05rem;
	}

	@media (max-width: 640px) {
		.hero {
			padding-top: 48px;
		}
		.hero h1 {
			font-size: 2.1rem;
		}
		.lede {
			font-size: 1.05rem;
		}
		.value-grid {
			grid-template-columns: 1fr;
		}
		.member {
			flex-direction: column;
			gap: 14px;
		}
		.os-card {
			padding: 24px;
		}
	}
</style>
