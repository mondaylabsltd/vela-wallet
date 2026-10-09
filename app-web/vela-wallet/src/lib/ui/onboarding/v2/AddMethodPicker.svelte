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
	 * `ownPage` is the one thing that is not a place: "Use my own signing page"
	 * (advanced), which mints a wallet whose keys live on the person's own
	 * domain and sign only on that page (R2, R3). Only a shell that can OPEN a
	 * page offers it. The web cannot (owner, 2026-09-23; plan P2-11), so the
	 * live flows never pass it — the gallery draws it, as the design the phones
	 * build to.
	 */
	import type { KeyMethod } from '$lib/onboarding/generated/KeyMethod';
	import { methodCopy, OWN_PAGE_COPY, type KeyChooser } from '$lib/onboarding/core/copy';
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
		/** "Use my own signing page" — only from a shell that can open one. */
		ownPage?: { onPick: () => void };
	}

	let { open, allowed, strings, onPick, chooser = 'create', ownPage }: Props = $props();

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
	{#if ownPage}
		<!-- Set apart from the three: it is not a place, and it is not for most people. -->
		<button class="method own" type="button" onclick={() => ownPage?.onPick()}>
			<PasskeyMethodIcon glyph={{ kind: 'lucide', name: 'eye' }} />
			<span class="text">
				<span class="name">{strings(OWN_PAGE_COPY.title)}</span>
				<span class="caption">{strings(OWN_PAGE_COPY.body)}</span>
			</span>
		</button>
	{/if}
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

	/* The advanced entry: below the three, quieter, its own block. */
	.own {
		margin-top: var(--space-md);
		border-bottom: 0;
	}

	.own .name {
		color: var(--color-fg-muted);
		font-weight: var(--weight-medium);
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
