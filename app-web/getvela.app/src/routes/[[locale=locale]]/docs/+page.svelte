<script lang="ts">
	import DocArticle from '$lib/components/DocArticle.svelte';
	import Seo from '$lib/components/Seo.svelte';
	import TranslationNotice from '$lib/components/TranslationNotice.svelte';
	import { pathFor } from '$lib/i18n/locales';
	import { docLocales, getDoc } from '$lib/content/docs';
	import { DOCS_INDEX_SLUG } from '$lib/content/sidebar';
	import { seoConfig } from '$lib/seo';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const slug = DOCS_INDEX_SLUG;
	const doc = $derived(getDoc(data.locale, slug)!);
	const Content = $derived(doc.component);
	const alternates = $derived(docLocales(slug));

	const jsonLd = $derived({
		'@context': 'https://schema.org',
		'@type': 'TechArticle',
		headline: doc.meta.title,
		description: doc.meta.description,
		url: `${seoConfig.domain}${pathFor(data.locale, '/docs')}`,
		inLanguage: doc.renderedLocale,
		publisher: { '@type': 'Organization', name: seoConfig.siteName }
	});
</script>

<Seo
	title={doc.meta.title}
	description={doc.meta.description ?? 'Vela Wallet documentation.'}
	canonical={pathFor(data.locale, '/docs')}
	locale={data.locale}
	{alternates}
	englishPath="/docs"
	{jsonLd}
/>

<!-- The notice follows the DOC, not the locale. -->
{#if doc.fallback}
	<TranslationNotice locale={data.locale} />
{/if}

<DocArticle {slug} locale={data.locale}>
	<Content />
</DocArticle>
