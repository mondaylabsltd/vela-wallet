<script lang="ts">
	import DocArticle from '$lib/components/DocArticle.svelte';
	import Seo from '$lib/components/Seo.svelte';
	import TranslationNotice from '$lib/components/TranslationNotice.svelte';
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

<!-- No docs page is translated yet — the per-locale content pipeline is T041 —
     so a localized docs URL is English, and the reader is told so in their own
     language. When translations land this becomes conditional on the page's
     real state rather than on the locale. -->
{#if data.locale !== 'en'}
	<TranslationNotice locale={data.locale} />
{/if}

{#key data.slug}
	<DocArticle slug={data.slug} locale={data.locale}>
		<Content />
	</DocArticle>
{/key}
