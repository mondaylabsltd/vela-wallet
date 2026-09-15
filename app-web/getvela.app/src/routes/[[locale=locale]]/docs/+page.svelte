<script lang="ts">
	import DocArticle from '$lib/components/DocArticle.svelte';
	import Seo from '$lib/components/Seo.svelte';
	import { getDoc } from '$lib/content/docs';
	import { DOCS_INDEX_SLUG } from '$lib/content/sidebar';
	import { seoConfig } from '$lib/seo';

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
	canonical="/docs"
	{jsonLd}
/>

<DocArticle {slug}>
	<Content />
</DocArticle>
