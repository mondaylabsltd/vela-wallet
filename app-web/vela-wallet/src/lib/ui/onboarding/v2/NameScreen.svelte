<script lang="ts">
	/**
	 * Name the wallet, and accept the three gates.
	 *
	 * Three checkboxes, matching the core's `ACK_COUNT`, and every one of them a
	 * FACT ABOUT WHERE SOMETHING ENDS UP: the public key and the name go into
	 * the on-chain contract, the private key stays in the device or on a
	 * security key, and the legal assent. Together they are the whole custody
	 * story, and none is pre-ticked — a box that arrives ticked records nothing.
	 *
	 * The recovery assurance that used to sit between them is gone. It described
	 * a BENEFIT, and mixing one of those into a list of consequences teaches
	 * people to skim the list.
	 *
	 * The field has no label and no helper line either. The heading above it
	 * already says "name your wallet", so a label restated it — and what the
	 * helper said (the name is stored on-chain) is now `ack0`, where a person
	 * has to look at it rather than past it.
	 *
	 * Legal assent goes LAST because it is the only line about the company
	 * rather than about the wallet.
	 */
	import AckRow from '../AckRow.svelte';
	import NameField from '../NameField.svelte';
	import Button from '$lib/ui/Button.svelte';

	interface Props {
		name: string;
		nameEditable: boolean;
		nameTooLong: boolean;
		/** The core's `name_bytes` / `name_limit`: the budget, shown before it is hit (ob-12). */
		nameBytes?: number;
		nameLimit?: number;
		/** Names of the wallets already on this device (ob-12). */
		takenNames?: string[];
		acks: boolean[];
		canSubmit: boolean;
		busy: boolean;
		submitLabel: string;
		statusText?: string;
		showStartOver: boolean;
		strings: (key: string, params?: Record<string, string | number>) => string;
		privacyUrl: string;
		termsUrl: string;
		onName: (value: string) => void;
		onToggleAck: (index: number) => void;
		onSubmit: () => void;
		onStartOver: () => void;
	}

	let {
		name,
		nameEditable,
		nameTooLong,
		nameBytes = 0,
		nameLimit = 0,
		takenNames = [],
		acks,
		canSubmit,
		busy,
		submitLabel,
		statusText,
		showStartOver,
		strings,
		privacyUrl,
		termsUrl,
		onName,
		onToggleAck,
		onSubmit,
		onStartOver
	}: Props = $props();

	const uid = $props.id();

	/** A wallet on this device already goes by this name (ob-12): said, not refused. */
	const duplicate = $derived.by(() => {
		const wanted = name.trim().toLowerCase();
		return wanted !== '' && takenNames.some((taken) => taken.trim().toLowerCase() === wanted);
	});

	/** The counter, from the first letter — so the limit is never a surprise. */
	const hint = $derived(
		duplicate
			? strings('onboarding.create.nameDuplicate', { name: name.trim() })
			: nameLimit > 0 && name.trim() !== ''
				? strings('onboarding.create.nameCounter', { used: nameBytes, limit: nameLimit })
				: undefined
	);
</script>

<!--
	The two policy links are absolute external URLs (getvela.app/privacy and
	/terms), not app routes, so `resolve()` has nothing to resolve. Disabled for
	the file rather than per line because the rule reports at the `href`
	attribute, which prettier wraps onto its own line.
-->
<!-- eslint-disable svelte/no-navigation-without-resolve -->
<section class="screen">
	<h1 class="title" id="{uid}-title">{strings('onboarding.create.nameTitle')}</h1>

	<NameField
		labelledby="{uid}-title"
		placeholder={strings('onboarding.create.accountNamePlaceholder')}
		errorText={nameTooLong ? strings('onboarding.create.nameTooLong') : undefined}
		{hint}
		value={name}
		oninput={nameEditable ? onName : undefined}
	/>

	<div class="spacer"></div>

	<div class="gates">
		<AckRow checked={acks[0] ?? false} onToggle={() => onToggleAck(0)}>
			{strings('onboarding.create.ack0')}
		</AckRow>

		<AckRow checked={acks[1] ?? false} onToggle={() => onToggleAck(1)}>
			{strings('onboarding.create.ack1')}
		</AckRow>

		<AckRow checked={acks[2] ?? false} onToggle={() => onToggleAck(2)}>
			{strings('onboarding.create.ack2')}<a
				href={privacyUrl}
				target="_blank"
				rel="noreferrer"
				onclick={(event) => event.stopPropagation()}
				>{strings('onboarding.create.ack2PrivacyPolicy')}</a
			>{strings('onboarding.create.ack2And')}<a
				href={termsUrl}
				target="_blank"
				rel="noreferrer"
				onclick={(event) => event.stopPropagation()}>{strings('onboarding.create.ack2Terms')}</a
			>{strings('onboarding.create.ack2Period')}
		</AckRow>
	</div>

	{#if statusText}
		<p class="status" aria-live="polite">{statusText}</p>
	{/if}

	<Button variant="primary" shape="rounded" disabled={!canSubmit} loading={busy} onclick={onSubmit}
		>{submitLabel}</Button
	>

	{#if showStartOver}
		<button class="startover" type="button" onclick={onStartOver}>
			{strings('onboarding.create.startOverBtn')}
		</button>
	{/if}
</section>

<style>
	.screen {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: var(--space-3xl);
	}

	.title {
		margin: 0;
		color: var(--color-fg-base);
		font-size: calc(var(--text-3xl) * var(--text-scale, 1));
		font-weight: var(--weight-bold);
		line-height: var(--leading-tight);
		letter-spacing: -0.015em;
	}

	.spacer {
		flex: 1;
		min-height: var(--space-md);
	}

	.gates {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
	}

	.status {
		margin: 0;
		color: var(--color-info-base);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
	}

	/* Links in the text's own ink, told apart by their underline: the
	   accent is only for the action that moves value (vis-6). */
	.gates :global(a) {
		color: var(--color-fg-base);
		text-decoration: underline;
	}

	.gates :global(a:hover) {
		color: var(--color-fg-muted);
	}

	.startover {
		align-self: center;
		padding: 0;
		border: 0;
		background: none;
		color: var(--color-fg-muted);
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		text-decoration: underline;
		cursor: pointer;
	}
</style>
