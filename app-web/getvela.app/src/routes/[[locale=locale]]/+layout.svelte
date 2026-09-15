<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { LayoutData } from './$types';

	/**
	 * The localized subtree's layout. It carries the locale — nothing else yet;
	 * the switcher and the fallback notice arrive in Phase 3 (T025, T044). Its
	 * job today is to exist so `+layout.ts` has somewhere to put `locale`, and so
	 * every localized page shares one place to wrap chrome around later.
	 */
	let { children, data }: { children: Snippet; data: LayoutData } = $props();
</script>

<svelte:head>
	<!-- `<html lang>` is set in hooks.server.ts (it lives outside the app root);
	     this is the app-level record of the same fact, for anything reading the DOM. -->
	<meta name="content-language" content={data.locale} />
</svelte:head>

{@render children()}
