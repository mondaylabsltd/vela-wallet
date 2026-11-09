<script lang="ts">
	/**
	 * A signature handed to the account's trusted signing page: the hand-off
	 * card the apps draw (spec 102 D4, `HandoffCard`), over the sheet that
	 * asked. Three hosts draw it, each its own `kind`: the signing host for a
	 * transaction, a batch, the wallet's own send or a message (the page,
	 * everywhere — owner, 2026-10-30), the multisig host for a member's
	 * approval (2026-10-29), and onboarding for a key ceremony — creating a
	 * key, signing in, a proof — under the ceremony's own title (spec 102 R8:
	 * every one of them in the Safari package).
	 *
	 * Deliberately not a preview: the page reads the request itself and is the
	 * authority. The card says that it is reviewed there, which key confirms
	 * it, what was checked about the page (and when) — and Open, which a page
	 * that failed its check never gets. Open is the tap a browser needs to open
	 * a window; after it, a wait, with the way back to the page. Closing the
	 * card gives up: nothing is signed.
	 */
	import HandoffCard from '$lib/signing/ui/HandoffCard.svelte';
	import type { HandoffMessages } from '$lib/signing/messages';
	import type { HandoffModel } from '$lib/signing/model';
	import { integrityLineModel } from '$lib/settings/venue';
	import { signerIntegrityTime } from '$lib/core/kernels';
	import Sheet from '$lib/multisig/ui/Sheet.svelte';
	import BottomSheet from '$lib/wallet/ui/BottomSheet.svelte';
	import { onMount } from 'svelte';
	import { pageHandoff, type HandoffKind } from '$lib/signing/page-handoff.svelte';

	interface Props {
		m: HandoffMessages;
		locale: string;
		wide: boolean;
		closeLabel: string;
		/** Which hand-offs this host draws. */
		kind: HandoffKind;
		/** Another kind it draws when no host of that kind is mounted. */
		standIn?: HandoffKind;
	}

	let { m, locale, wide, closeLabel, kind, standIn }: Props = $props();

	onMount(() => {
		const mine = kind;
		pageHandoff.drawers[mine] += 1;
		return () => {
			pageHandoff.drawers[mine] -= 1;
		};
	});

	const handoff = $derived.by(() => {
		const current = pageHandoff.current;
		if (current === null) return null;
		if (current.kind === kind) return current;
		return standIn !== undefined && current.kind === standIn && pageHandoff.drawers[standIn] === 0
			? current
			: null;
	});

	/** "checked 14:32", in the core's words for the time (`launch::checked_time`). */
	function checkedTime(at: number | bigint | null): string {
		if (at === null) return '';
		const now = Date.now();
		return signerIntegrityTime(
			Number(at),
			now,
			-new Date(now).getTimezoneOffset(),
			{ date: 'iso', time: 'h24' },
			locale
		);
	}

	/** A ceremony's own title (nothing to review there), else the signature card's. */
	const title = $derived(
		handoff?.titleKey ? (m.ceremonies?.[handoff.titleKey] ?? m.title) : m.title
	);

	const model = $derived.by((): HandoffModel | null => {
		const h = handoff;
		if (h === null) return null;
		const line = h.line ?? { key: 'componentsUi.signing.integrity.checking', version: '' };
		const integrity = integrityLineModel(
			line,
			h.line ? checkedTime(h.line.checked_at_ms) : '',
			m
		) ?? { text: '', tone: 'checking' as const };
		const key = h.keyLabel;
		return {
			title,
			key: {
				label: key ? (m.keyLabels[key.label_key] ?? '') : '',
				value: key ? (key.name ?? m.places[key.place_key] ?? '') : h.member
			},
			page: { name: h.official ? m.official : h.host, host: h.host },
			integrity,
			open: {
				label: m.open,
				enabled: h.phase === 'ready' || h.phase === 'waiting',
				// R22 (D4): the Open is the confirm that raised the card.
				...(pageHandoff.posture && h.kind !== 'ceremony'
					? {
							danger: pageHandoff.posture.danger,
							holdMs: pageHandoff.posture.holdMs,
							holdLabel: pageHandoff.posture.holdLabel,
							againLabel: pageHandoff.posture.againLabel
						}
					: {})
			},
			waiting:
				h.phase === 'waiting'
					? { title: m.waiting, hint: m.waitingHint, reopen: m.reopen }
					: undefined
		};
	});
</script>

{#snippet card(h: NonNullable<typeof handoff>, model: HandoffModel)}
	<div data-testid="page-handoff" data-kind={h.kind} data-phase={h.phase}>
		<HandoffCard handoff={model} onopen={() => h.open()} />
	</div>
{/snippet}

{#if handoff !== null && model !== null}
	{#if handoff.kind === 'signature' || handoff.kind === 'ceremony'}
		<!--
			Over the signing sheet, which is fixed above the page (the `signing`
			layer): the `prompt` layer, after it, is above it on every width. Only
			its ✕ closes it — closing gives the signature up, so a stray tap on the
			scrim must not.
		-->
		<BottomSheet
			{title}
			{closeLabel}
			variant="prompt"
			dismissible="explicit"
			onclose={() => handoff.cancel()}
		>
			{@render card(handoff, model)}
		</BottomSheet>
	{:else}
		<Sheet {title} {closeLabel} {wide} onclose={() => handoff.cancel()}>
			{@render card(handoff, model)}
		</Sheet>
	{/if}
{/if}
