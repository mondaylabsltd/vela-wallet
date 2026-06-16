<script lang="ts">
	import { seoConfig } from '$lib/seo';

	type JsonLd = Record<string, unknown>;

	let {
		title,
		description,
		canonical,
		image,
		type = 'website',
		published,
		modified,
		author,
		jsonLd,
		noindex = false
	}: {
		/** Page title — also used for the browser tab and og:title. */
		title: string;
		description: string;
		/** Absolute path, e.g. "/blog/my-post". Resolved against the site domain. */
		canonical: string;
		/** Full image URL. Defaults to a generated OG image (articles) or the site preview. */
		image?: string;
		type?: 'website' | 'article';
		published?: string;
		modified?: string;
		author?: string;
		jsonLd?: JsonLd | JsonLd[];
		noindex?: boolean;
	} = $props();

	const domain = seoConfig.domain;
	const fullTitle = $derived(
		title.includes(seoConfig.siteName) ? title : `${title} — ${seoConfig.siteName}`
	);
	const url = $derived(domain + canonical);
	const ogImage = $derived(
		image ??
			(type === 'article'
				? `${domain}/api/og?type=article&title=${encodeURIComponent(title)}${
						author ? `&author=${encodeURIComponent(author)}` : ''
					}${published ? `&date=${encodeURIComponent(published)}` : ''}`
				: `${domain}/getvela-app-preview.png`)
	);

	// Serialize JSON-LD, escaping `<` so the value can never break out of the
	// surrounding <script> tag.
	const ldHtml = $derived(
		jsonLd
			? `<script type="application/ld+json">${JSON.stringify(jsonLd).replace(
					/</g,
					'\\u003c'
				)}<\/script>`
			: ''
	);
</script>

<svelte:head>
	<title>{fullTitle}</title>
	<meta name="description" content={description} />
	<link rel="canonical" href={url} />
	{#if noindex}
		<meta name="robots" content="noindex, nofollow" />
	{/if}

	<meta property="og:type" content={type} />
	<meta property="og:site_name" content={seoConfig.siteName} />
	<meta property="og:title" content={fullTitle} />
	<meta property="og:description" content={description} />
	<meta property="og:url" content={url} />
	<meta property="og:image" content={ogImage} />
	{#if type === 'article' && published}
		<meta property="article:published_time" content={published} />
	{/if}
	{#if type === 'article' && modified}
		<meta property="article:modified_time" content={modified} />
	{/if}
	{#if type === 'article' && author}
		<meta property="article:author" content={author} />
	{/if}

	<meta name="twitter:card" content="summary_large_image" />
	<meta name="twitter:title" content={fullTitle} />
	<meta name="twitter:description" content={description} />
	<meta name="twitter:image" content={ogImage} />
	<meta name="twitter:site" content="@realvelawallet" />

	{#if ldHtml}
		<!-- eslint-disable-next-line svelte/no-at-html-tags -->
		{@html ldHtml}
	{/if}
</svelte:head>
