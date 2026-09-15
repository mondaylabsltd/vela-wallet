<script lang="ts">
	import DocArticle from '$lib/components/DocArticle.svelte';
	import Seo from '$lib/components/Seo.svelte';
	import TranslationNotice from '$lib/components/TranslationNotice.svelte';
	import { pathFor } from '$lib/i18n/locales';
	import { docLocales, getDoc } from '$lib/content/docs';
	import { seoConfig } from '$lib/seo';
	import type { PageData } from './$types';

	let { data }: { data: PageData } = $props();

	const doc = $derived(getDoc(data.locale, data.slug)!);
	const Content = $derived(doc.component);
	const description = $derived(data.meta.description ?? `${data.meta.title} — Vela Wallet docs.`);

	const alternates = $derived(docLocales(data.slug));

	const jsonLd = $derived({
		'@context': 'https://schema.org',
		'@type': 'TechArticle',
		headline: data.meta.title,
		description,
		url: `${seoConfig.domain}${pathFor(data.locale, `/docs/${data.slug}`)}`,
		inLanguage: doc.renderedLocale,
		publisher: { '@type': 'Organization', name: seoConfig.siteName }
	});
</script>

<Seo
	title={data.meta.title}
	{description}
	canonical={pathFor(data.locale, `/docs/${data.slug}`)}
	locale={data.locale}
	{alternates}
	englishPath={`/docs/${data.slug}`}
	{jsonLd}
/>

<!-- The notice follows the DOC, not the locale: once `zh/faq.md` exists, the
     Chinese FAQ stops apologising while the untranslated ones still do. -->
{#if data.fallback}
	<TranslationNotice locale={data.locale} />
{/if}

{#key data.slug}
	<DocArticle slug={data.slug} locale={data.locale}>
		<Content />
	</DocArticle>
{/key}
