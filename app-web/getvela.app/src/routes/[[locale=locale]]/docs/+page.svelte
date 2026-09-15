<script lang="ts">
	import DocArticle from '$lib/components/DocArticle.svelte';
	import Seo from '$lib/components/Seo.svelte';
	import TranslationNotice from '$lib/components/TranslationNotice.svelte';
	import { pathFor } from '$lib/i18n/locales';
	import { getDoc } from '$lib/content/docs';
	import { DOCS_INDEX_SLUG } from '$lib/content/sidebar';
	import { seoConfig } from '$lib/seo';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const slug = DOCS_INDEX_SLUG;
	const doc = getDoc(slug)!;
	const Content = doc.component;

	const jsonLd = {
		'@context': 'https://schema.org',
		'@type': 'TechArticle',
		headline: doc.meta.title,
		description: doc.meta.description,
		url: `${seoConfig.domain}/docs`,
		publisher: { '@type': 'Organization', name: seoConfig.siteName }
	};
</script>

<Seo
	title={doc.meta.title}
	description={doc.meta.description ?? 'Vela Wallet documentation.'}
	canonical={pathFor(data.locale, '/docs')}
	locale={data.locale}
	englishPath="/docs"
	{jsonLd}
/>

<!-- No docs page is translated yet — the per-locale content pipeline is T041 —
     so a localized docs URL is English, and the reader is told so in their own
     language. When translations land this becomes conditional on the page's
     real state rather than on the locale. -->
{#if data.locale !== 'en'}
	<TranslationNotice locale={data.locale} />
{/if}

<DocArticle {slug} locale={data.locale}>
	<Content />
</DocArticle>
