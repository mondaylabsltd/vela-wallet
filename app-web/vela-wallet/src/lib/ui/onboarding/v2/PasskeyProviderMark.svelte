<script lang="ts">
	/**
	 * The mark of the vault holding a passkey — Apple Passwords, 1Password,
	 * Windows Hello — resolved from the key's AAGUID by the core.
	 *
	 * Renders nothing when the catalog has no entry, which is a normal answer:
	 * hardware keys live in the FIDO metadata service, and an authenticator may
	 * report no AAGUID at all. The row keeps the shape glyph it always drew.
	 *
	 * An image element, not inline markup: these marks carry embedded CSS with
	 * generic class names (`.cls-1`) and `clipPath` ids, and several of them
	 * inlined into one document would fight over both — the same collision that
	 * once turned this app's identicons square. (Written without the literal
	 * tag names: svelte2tsx scans this block for them and would call the
	 * script unclosed.)
	 */
	import { browser } from '$app/environment';
	import { isDarkTheme } from '$lib/theme.svelte';
	import {
		passkeyFallbackIconDataUri,
		passkeyProviderIconDataUri
	} from '$lib/onboarding/core/wasm-client';
	import type { CreateKeyRow } from '$lib/onboarding/generated/CreateKeyRow';
	import { directoryEntry } from '$lib/onboarding/core/passkey-directory.svelte';
	import { isHandheld, keyKindGlyph } from '$lib/onboarding/passkey-icons';
	import PasskeyMethodIcon from '$lib/ui/onboarding/PasskeyMethodIcon.svelte';

	interface Props {
		/** The row this mark stands for; everything comes off it. */
		key: CreateKeyRow;
		/** The mark's accessible label. */
		label: string;
		/**
		 * Draw the glyph for where the key lives when there is no artwork at
		 * all — a platform authenticator the catalog cannot name. A list wants
		 * a filled slot: an empty one leaves the names in that row starting a
		 * few pixels further left than every other row's.
		 */
		glyphFallback?: boolean;
	}

	let { key, label, glyphFallback = false }: Props = $props();

	/**
	 * The fallback artwork's three slots, read off the live cascade rather than
	 * hard-coded: it ships in one theme, and one vendor's greys are not this
	 * app's greys in either. Resolved once — these tokens do not change while a
	 * key list is on screen.
	 */
	const token = (name: string) =>
		browser ? getComputedStyle(document.documentElement).getPropertyValue(name).trim() : '';
	const strong = $derived(token('--color-fg-muted'));
	const soft = $derived(token('--color-border-strong'));
	const hole = $derived(token('--color-bg-base'));

	const dark = $derived(isDarkTheme());

	/**
	 * Three sources, in order: the provider's own mark from the compiled
	 * catalog; the directory service's mark for a model no catalog carries
	 * (hardware keys); and the security-key artwork when neither can name it but
	 * the authenticator at least said what KIND it is. A platform authenticator
	 * nobody can name gets none of them, and the row keeps its shape glyph.
	 */
	const uri = $derived.by(() => {
		// Nothing to draw before hydration: the artwork comes from the wasm core,
		// which only exists in the browser, and these screens only render there.
		if (!browser) return undefined;
		// Spec 075: a key behind a Trusted Signer page wears no vault's mark. The
		// AAGUID it reported is the authenticator on the PAGE's side — the one
		// thing this wallet cannot reach — so a vendor's logo here would draw the
		// far side of the page while the caption names the page. The glyph below
		// (the eye) is the honest slot, and the two read one field, as issue 207
		// requires.
		if (key.kind === 'trusted_signer') return undefined;
		const provider = key.aaguid ? passkeyProviderIconDataUri(key.aaguid, dark) : undefined;
		if (provider) return provider;
		const listed = directoryEntry(key.aaguid, dark);
		if (listed?.iconUrl) return listed.iconUrl;
		return passkeyFallbackIconDataUri(
			key.authenticator_attachment,
			key.transports,
			// The REPORT, not the tap: a person who tapped "security key" and
			// then used Touch ID does not own a fob (issue 207). The core only
			// consults this flag when the authenticator reported nothing at all.
			key.kind === 'security_key',
			strong,
			soft,
			hole
		);
	});

	/**
	 * The last resort: the shape of the thing the key lives in — a laptop, a
	 * phone, a USB key — drawn by the same component the method picker uses, so
	 * the row and the picker speak one visual language.
	 *
	 * Keyed off `kind`, like the caption beside it, which is what stops the slot
	 * contradicting the words (issue 207: a hardware fob next to "Passkey").
	 * `isHandheld` reads `navigator`, so it is only asked in the browser — this
	 * component renders nothing before hydration anyway.
	 */
	const glyph = $derived(keyKindGlyph(key.kind, browser ? isHandheld() : false));
</script>

{#if uri}
	<img class="mark" src={uri} alt={label} />
{:else if glyphFallback}
	<span class="glyph"><PasskeyMethodIcon {glyph} /></span>
{/if}

<style>
	.mark {
		flex: 0 0 var(--icon-xl);
		width: var(--icon-xl);
		height: var(--icon-xl);
		border-radius: var(--radius-sm);
		object-fit: contain;
	}

	/*
	 * The last resort, moved here from the key list so one component owns the
	 * whole question of what a row's leading slot shows.
	 *
	 * It used to be three bare CSS boxes distinguished only by proportion — a
	 * wide one, a tall one, a squat one — which is a shape nobody can name.
	 * Issue 207 replaced them with the picker's own glyphs, drawn by the same
	 * component, so a row shows a laptop, a phone or a USB key. The wrapper
	 * keeps the slot the size of a provider mark, so rows with and without
	 * artwork still line up.
	 */
	.glyph {
		display: grid;
		flex: 0 0 var(--icon-xl);
		place-items: center;
		width: var(--icon-xl);
		height: var(--icon-xl);
	}
</style>
