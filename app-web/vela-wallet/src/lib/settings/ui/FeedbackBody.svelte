<script lang="ts">
	/**
	 * ST15 — the in-app report. The disclosure showing exactly what will be
	 * sent is open by default and the consent note sits directly above the send
	 * button, because the promise ("never keys, seed phrase, or balances") is
	 * only worth anything next to the thing it is a promise about.
	 *
	 * Spec 081 FR-016 gave the button somewhere to go. Three things changed and
	 * each one was a lie being retired:
	 *
	 * - **`onsend` was never passed.** `SettingsHome` rendered this component
	 *   with `panel` alone, so the primary button on the screen did nothing at
	 *   all — the person typed, pressed 发送, and watched the sheet sit there.
	 * - **The preview lines were fiction** (a phone running iOS 26.0 in
	 *   Chinese, on somebody else's build). They now come from the same builder
	 *   the payload is assembled from, so the consent sentence is literal.
	 * - **There was no steps box.** `+ 添加重现步骤` was a button with no
	 *   textarea behind it, and the GitHub form marks steps required.
	 *
	 * The two outcomes are drawn here rather than in a second sheet: a report
	 * that filed says which issue it became and offers to open it; a report the
	 * endpoint refused (503 when the token is unprovisioned, 429, 5xx, no
	 * network) offers the prefilled form, which is the ONLY remaining road and
	 * must never be replaced by an apology.
	 */
	import type { FeedbackModel, FeedbackResult } from '../model';
	import Button from '$lib/ui/Button.svelte';
	import { UTILITY_ICONS } from '$lib/wallet/icons';
	import Icon from '$lib/wallet/ui/Icon.svelte';
	import { fill } from '$lib/wallet/messages';
	import Disclosure from './Disclosure.svelte';

	interface Props {
		panel: FeedbackModel;
		/** Absent in the gallery, where this sheet is a picture of itself. */
		onsend?: (report: { what: string; steps: string }) => void;
		/** The route is waiting on the endpoint. */
		sending?: boolean;
		/** The last send's outcome, or `undefined` before the first. */
		result?: FeedbackResult;
	}

	let { panel, onsend, sending = false, result }: Props = $props();

	let what = $state('');
	let steps = $state('');
	let stepsOpen = $state(false);

	/** Nothing typed is nothing to file — the endpoint refuses it with a 400. */
	const ready = $derived(what.trim() !== '');
</script>

<div class="feedback">
	{#if result?.filed === true}
		<!-- Filed. The number is the whole point: a person who reported
		     something is owed a way back to it. -->
		<p class="ok"><Icon icon={UTILITY_ICONS.check} size="md" /><span>{panel.success.title}</span></p>
		<p class="line">
			{fill(result.deduped === true ? panel.success.bodyDeduped : panel.success.bodyNew, {
				number: result.number ?? 0
			})}
		</p>
		{#if result.url !== undefined}
			<a class="github" href={result.url} target="_blank" rel="noreferrer noopener"
				>{panel.success.view}</a
			>
		{/if}
	{:else}
		<textarea
			data-field
			bind:value={what}
			placeholder={panel.placeholder}
			aria-label={panel.placeholder}
			rows="4"
		></textarea>

		{#if stepsOpen}
			<textarea
				data-field
				bind:value={steps}
				placeholder={panel.stepsPlaceholder}
				aria-label={panel.stepsPlaceholder}
				rows="3"
			></textarea>
		{:else}
			<button type="button" class="steps" onclick={() => (stepsOpen = true)}>{panel.addSteps}</button
			>
		{/if}

		<Disclosure label={panel.previewToggle}>
			{#each panel.previewLines as line (line)}
				<p class="line">{line}</p>
			{/each}
		</Disclosure>

		<p class="consent">
			<Icon icon={UTILITY_ICONS.info} size="md" />
			<span>{panel.consent}</span>
		</p>

		{#if result !== undefined && result.filed === false && result.fallbackUrl !== undefined}
			<!-- The endpoint could not file it. This is not an error message: it
			     is the other road, and the person's typing is already in that
			     URL. The Send button stays below it, because "try again" is a
			     real answer to a 429 or a dropped connection. -->
			<p class="consent warn">
				<Icon icon={UTILITY_ICONS['triangle-alert']} size="md" />
				<span>{panel.fallback.title} — {panel.fallback.body}</span>
			</p>
			<Button variant="primary" shape="rounded" href={result.fallbackUrl} external
				>{panel.fallback.open}</Button
			>
		{/if}

		<!-- Dimmed only where the button DOES something: the gallery board draws
		     ST15 as the mock draws it, at full emphasis, and a disabled CTA in a
		     design board is a state the mock never had. -->
		<Button
			variant={result?.filed === false ? 'secondary' : 'primary'}
			shape="rounded"
			loading={sending}
			disabled={onsend !== undefined && !ready}
			onclick={() => onsend?.({ what, steps })}>{sending ? panel.sending : panel.send}</Button
		>

		<a
			class="github"
			href="https://github.com/mondaylabsltd/vela-wallet/issues/new"
			target="_blank"
			rel="noreferrer noopener">{panel.githubLink}</a
		>
	{/if}
</div>

<style>
	.feedback {
		display: flex;
		flex-direction: column;
		gap: var(--space-xl);
		padding-block: var(--space-md) var(--space-xl);
	}

	textarea {
		width: 100%;
		padding: var(--space-lg);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-lg);
		background: var(--color-bg-sunken);
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-fg-base);
		resize: vertical;
		outline: none;
	}

	textarea::placeholder {
		color: var(--color-fg-subtle);
	}

	.steps {
		align-self: flex-start;
		padding: 0;
		border: none;
		background: none;
		font-family: var(--font-ui);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-info-base);
		cursor: pointer;
	}

	.line {
		margin: 0;
	}

	.ok {
		display: flex;
		align-items: center;
		gap: var(--space-md);
		margin: 0;
		font-size: calc(var(--text-lg) * var(--text-scale, 1));
		font-weight: var(--weight-semibold);
		color: var(--color-success-base);
	}

	.consent {
		display: flex;
		align-items: flex-start;
		gap: var(--space-md);
		margin: 0;
		padding: var(--space-lg);
		border-radius: var(--radius-lg);
		background: var(--color-info-soft);
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		line-height: var(--leading-normal);
		color: var(--color-info-base);
	}

	/* The fallback is a route, not a failure — amber, not red, because
	   nothing has been lost and there is a button that still works. */
	.consent.warn {
		background: var(--color-warning-soft);
		color: var(--color-warning-base);
	}

	.github {
		align-self: center;
		font-size: calc(var(--text-base) * var(--text-scale, 1));
		color: var(--color-info-base);
	}
</style>
