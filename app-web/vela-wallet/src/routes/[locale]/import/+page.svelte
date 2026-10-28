<script lang="ts">
	/**
	 * vis-22: no page of its own any more. "I already have a wallet" is
	 * Welcome's sheet, and nothing in the app links here; an old link or a
	 * typed address is taken there, with the sheet open. The placeholder is
	 * what shows for the moment before (and without) the script.
	 */
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import type { PageProps } from './$types';
	import PlaceholderPage from '$lib/ui/PlaceholderPage.svelte';
	import { SIGN_IN_PARAM, STAY_PARAM } from '$lib/intro/gate';

	let { data }: PageProps = $props();

	onMount(() => {
		const welcome = resolve('/[locale]', { locale: data.locale });
		// eslint-disable-next-line svelte/no-navigation-without-resolve -- Welcome, resolved above, with its query
		void goto(`${welcome}?${STAY_PARAM}&${SIGN_IN_PARAM}`, { replaceState: true });
	});
</script>

<svelte:head><title>{data.messages.alreadyHaveWallet}</title></svelte:head>

<PlaceholderPage
	title={data.messages.alreadyHaveWallet}
	backHref={resolve('/[locale]', { locale: data.locale })}
/>
