<script lang="ts">
	/**
	 * The share card (spec 038 #meta): what a link to this page becomes on
	 * X, Discord, iMessage, WeChat. Without these a shared link was an
	 * anonymous text card — no title, no image, no site name — and the image
	 * some clients fell back to was the template favicon.
	 *
	 * One text-free image serves every locale; the words are the page's own.
	 * `description` is the meta description clipped to what previews show
	 * (about 110 characters) at a word boundary — the corpus's sentence is
	 * written for a search result, and a card truncates it mid-word otherwise.
	 */
	import { SITE_ORIGIN } from '$lib/site';
	import { SUPPORTED_LOCALES } from '$lib/i18n/locales';

	interface Props {
		locale: string;
		title: string;
		description: string;
		/** Absolute canonical URL of this page. */
		url: string;
	}

	let { locale, title, description, url }: Props = $props();

	const SOCIAL_MAX = 110;
	const short = $derived(clip(description, SOCIAL_MAX));
	const image = $derived(`${SITE_ORIGIN}/og-image.png`);
	const ogLocale = $derived(locale.replace('-', '_'));
	const alternates = $derived(
		SUPPORTED_LOCALES.filter((l) => l !== locale).map((l) => l.replace('-', '_'))
	);

	/** Clip at a word boundary (a space, or any character for CJK) with an ellipsis. */
	export function clip(text: string, max: number): string {
		if (text.length <= max) return text;
		const cut = text.slice(0, max);
		const space = cut.lastIndexOf(' ');
		return `${(space > max * 0.6 ? cut.slice(0, space) : cut).trimEnd()}…`;
	}
</script>

<svelte:head>
	<meta property="og:site_name" content="Vela Wallet" />
	<meta property="og:type" content="website" />
	<meta property="og:title" content={title} />
	<meta property="og:description" content={short} />
	<meta property="og:url" content={url} />
	<meta property="og:image" content={image} />
	<meta property="og:image:width" content="1200" />
	<meta property="og:image:height" content="630" />
	<meta property="og:locale" content={ogLocale} />
	{#each alternates as alt (alt)}
		<meta property="og:locale:alternate" content={alt} />
	{/each}
	<meta name="twitter:card" content="summary_large_image" />
	<meta name="twitter:title" content={title} />
	<meta name="twitter:description" content={short} />
	<meta name="twitter:image" content={image} />
</svelte:head>
