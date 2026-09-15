<script lang="ts">
	import DocArticle from '$lib/components/DocArticle.svelte';
	import Seo from '$lib/components/Seo.svelte';
	import { pathFor } from '$lib/i18n/locales';
	import { getDoc } from '$lib/content/docs';
	import { seoConfig } from '$lib/seo';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const doc = $derived(getDoc(data.slug)!);
	const Content = $derived(doc.component);
	const description = $derived(data.meta.description ?? `${data.meta.title} — Vela Wallet docs.`);

	const jsonLd = $derived({
		'@context': 'https://schema.org',
		'@type': 'TechArticle',
		headline: data.meta.title,
		description,
		url: `${seoConfig.domain}/docs/${data.slug}`,
		publisher: { '@type': 'Organization', name: seoConfig.siteName }
	});
</script>

<Seo
	title={data.meta.title}
	{description}
	canonical={pathFor(data.locale, `/docs/${data.slug}`)}
	locale={data.locale}
	englishPath={`/docs/${data.slug}`}
	{jsonLd}
/>

{#key data.slug}
	<DocArticle slug={data.slug} locale={data.locale}>
		<Content />
	</DocArticle>
{/key}
