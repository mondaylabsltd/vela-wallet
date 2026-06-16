<script lang="ts">
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
			avatar: '/shelchin-avatar.jpg',
			links: [
				{ label: 'GitHub', href: 'https://github.com/atshelchin' },
				{ label: 'X', href: 'https://x.com/atshelchin' }
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
		<p class="eyebrow">About</p>
		<h1>Who builds Vela.</h1>
		<p class="lede">
			Vela is built in the open — the wallet, the smart contracts, and this very site. No faceless
			company behind it: just real code you can read, and a real person you can reach.
		</p>
	</section>

	<section class="team">
		<div class="team-grid">
			{#each team as member (member.name)}
				<div class="member">
					{#if member.avatar}
						<img class="avatar" src={member.avatar} alt={member.name} width="72" height="72" loading="lazy" />
					{:else}
						<div class="avatar" aria-hidden="true">{member.initials}</div>
					{/if}
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
		width: 72px;
		height: 72px;
		border-radius: 50%;
		object-fit: cover;
		display: grid;
		place-items: center;
		font-size: 1.6rem;
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
	}
</style>
