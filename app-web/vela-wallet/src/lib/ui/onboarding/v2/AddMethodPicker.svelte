<script lang="ts">
	/**
	 * The three places a founding key can live — or be found, when this picker
	 * is the sign-in sheet — expanded in place.
	 *
	 * The three are the browser's to present: `navigator.credentials` shows its
	 * own picker covering this device, a nearby device and a security key. So
	 * they dispatch the SAME ceremony and differ only in what the core records
	 * as the person's choice — which is what labels the key row afterwards, and
	 * which hints a later signature carries.
	 *
	 * That is a web-only truth. The native shells run the ceremony themselves
	 * and must honour the choice, which is why it travels through the core
	 * rather than being decided here.
	 *
	 * Three, and no fourth (spec 102). The Trusted Signer used to sit here as a
	 * fourth "place", and a person who tapped it was asked "this device / scan a
	 * code / USB key" again — because a signing page is not where a key lives,
	 * it is where a person REVIEWS AND SIGNS. That is the account's venue now,
	 * chosen in Settings, and the same three places exist on the page too.
	 *
	 * The apps add one entry below the three that is not a place: "Use a
	 * trusted signing page" (D6), for a wallet whose keys live on a page's own
	 * domain and sign only on that page (R2, R3). The web has no such entry
	 * (P2b-W3): it opens no signing page, so a self-hosted page's ceremonies
	 * could never run here, and the official page's run in the app and sign in
	 * Vela on the web anyway (P2-11).
	 */
	import type { KeyMethod } from '$lib/onboarding/generated/KeyMethod';
	import { methodCopy, type KeyChooser } from '$lib/onboarding/core/copy';
	import { methodGlyph } from '$lib/onboarding/passkey-icons';
	import PasskeyMethodIcon from '$lib/ui/onboarding/PasskeyMethodIcon.svelte';

	interface Props {
		open: boolean;
		/** The places the core offers — always the three (spec 102); every place when absent. */
		allowed?: KeyMethod[];
		strings: (key: string, params?: Record<string, string | number>) => string;
		onPick: (method: KeyMethod) => void;
		/**
		 * Creating a key, or finding one (087 F02): the sign-in sheet's phone
		 * row scans — it creates nothing.
		 */
		chooser?: KeyChooser;
	}

	let { open, allowed, strings, onPick, chooser = 'create' }: Props = $props();

	const METHODS: KeyMethod[] = ['platform', 'hybrid', 'security_key'];

	const offered: KeyMethod[] = $derived(allowed ?? METHODS);
</script>

{#if open}
	<ul class="methods">
		{#each METHODS as method (method)}
			{@const copy = methodCopy(method, chooser)}
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
{/if}

<style>
	.methods {
		display: flex;
		flex-direction: column;
		margin: 0;
		padding: 0;
		list-style: none;
	}

	/* A row in Settings' metrics (issue 475): at least a control tall, a
	   hairline under it, the caption a breath below the name. */
	.method {
		display: flex;
		gap: var(--space-lg);
		align-items: center;
		width: 100%;
		min-height: var(--size-control-lg);
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

	.text {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: var(--space-sm);
		min-width: 0;
	}

	.name {
		color: var(--color-fg-base);
		font-size: var(--text-base);
		font-weight: var(--weight-semibold);
	}

	/* ONE line (issue 475): three rows that each wrap to two read as six
	   lines of caption, and the rows stop lining up. Every caption in the
	   corpus fits a 390 px phone (`add-method-picker.svelte.test.ts` measures
	   all fifteen languages); the ellipsis is for a text size nobody tested. */
	.caption {
		overflow: hidden;
		color: var(--color-fg-muted);
		font-size: var(--text-sm);
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
