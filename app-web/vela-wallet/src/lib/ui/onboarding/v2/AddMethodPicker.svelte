<script lang="ts">
	/**
	 * The four ways to mint a founding key — or to find one, when this picker
	 * is the sign-in sheet — expanded in place.
	 *
	 * The first three are the browser's to present: `navigator.credentials`
	 * shows its own picker covering this device, a nearby device and a security
	 * key. So those three dispatch the SAME ceremony and differ only in what the
	 * core records as the person's choice — which is what labels the key row
	 * afterwards.
	 *
	 * That is a web-only truth. The native shells run the ceremony themselves
	 * and must honour the choice, which is why it travels through the core
	 * rather than being decided here.
	 *
	 * The fourth is not a place a passkey is: the Clear Signer (spec 075) is a
	 * passkey route of our own — a page that shows what it is about to do and
	 * runs the ceremony there, on this device or on another one. The executor
	 * sends it to the page instead of the browser's sheet.
	 *
	 * `allowed` narrows the list once the set has a key: every key in a wallet
	 * belongs to the same relying party — the registry files a unit under one
	 * `rpId` — so a route that would mint for a different one cannot add to
	 * THIS set. Such a row stays, disabled, with `blocked`'s sentence under the
	 * list: a row that vanished could not say that the Clear Signer's page is a
	 * setting the person can change.
	 */
	import type { KeyMethod } from '$lib/onboarding/generated/KeyMethod';
	import type { AddBlocked } from '$lib/onboarding/generated/AddBlocked';
	import { methodCopy } from '$lib/onboarding/core/copy';
	import { methodGlyph } from '$lib/onboarding/passkey-icons';
	import PasskeyMethodIcon from '$lib/ui/onboarding/PasskeyMethodIcon.svelte';

	interface Props {
		open: boolean;
		/** The routes that may mint for this set; every route when absent. */
		allowed?: KeyMethod[];
		blocked?: AddBlocked | null;
		strings: (key: string, params?: Record<string, string | number>) => string;
		onPick: (method: KeyMethod) => void;
	}

	let { open, allowed, blocked = null, strings, onPick }: Props = $props();

	const METHODS: KeyMethod[] = ['platform', 'hybrid', 'security_key'];

	/**
	 * The core offers `clear_signer` to every shell; this one cannot open a
	 * Clear Signer page at all (owner, 2026-09-23), so it is never drawn here.
	 *
	 * Filtered rather than trusted-not-to-appear: `allowed` comes from the
	 * core, and a wallet whose only key was minted on a page could otherwise be
	 * created from a browser that can never sign with it again.
	 */
	const offered: KeyMethod[] = $derived(
		(allowed ?? METHODS).filter((m) => m !== 'clear_signer')
	);

	/**
	 * What this wallet's keys belong to, and — when the configured Clear Signer
	 * page is the thing that does not fit — which page it is and what a key
	 * from it would belong to instead.
	 *
	 * Two paragraphs, never one joined string: the first fact is always the
	 * reason, the second is only sometimes true and is the half a person can
	 * act on. Joining them would also put a space after a full stop that
	 * already ends a line in Chinese (device-found, 2026-09-23).
	 */
	const reason = $derived.by((): string[] => {
		if (!blocked) return [];
		const lines = [
			strings('onboarding.create.methodBlockedHint', { party: blocked.relying_party })
		];
		if (blocked.page) {
			lines.push(
				strings('onboarding.create.methodBlockedSigner', {
					page: blocked.page,
					pageParty: blocked.page_relying_party ?? '',
					party: blocked.relying_party
				})
			);
		}
		return lines;
	});
</script>

{#if open}
	<ul class="methods">
		{#each METHODS as method (method)}
			{@const copy = methodCopy(method)}
			{@const can = offered.includes(method)}
			<li>
				<button
					class="method"
					class:off={!can}
					type="button"
					disabled={!can}
					onclick={() => onPick(method)}
				>
					<PasskeyMethodIcon glyph={methodGlyph(method)} />
					<span class="text">
						<span class="name">{strings(copy.title)}</span>
						<span class="caption">{strings(copy.body)}</span>
					</span>
				</button>
			</li>
		{/each}
	</ul>
	{#each reason as line (line)}
		<p class="reason">{line}</p>
	{/each}
{/if}

<style>
	.methods {
		display: flex;
		flex-direction: column;
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.method {
		display: flex;
		gap: var(--space-lg);
		align-items: center;
		width: 100%;
		padding-block: var(--space-lg);
		border: 0;
		border-bottom: var(--border-hairline) solid var(--color-border-base);
		background: none;
		font-family: var(--font-ui);
		text-align: start;
		cursor: pointer;
	}

	.method:hover:not(:disabled) {
		background: var(--color-bg-sunken);
	}

	.off {
		opacity: var(--opacity-disabled);
		cursor: not-allowed;
	}

	.reason {
		margin: var(--space-sm) 0 0;
		color: var(--color-fg-muted);
		font-size: var(--text-sm);
	}

	.text {
		display: flex;
		flex-direction: column;
		gap: var(--space-xs);
	}

	.name {
		color: var(--color-fg-base);
		font-size: var(--text-base);
		font-weight: var(--weight-semibold);
	}

	.caption {
		color: var(--color-fg-muted);
		font-size: var(--text-sm);
	}
</style>
