<script lang="ts">
	/**
	 * A multisig member's approval handed to the member's trusted signing page
	 * (2026-10-29): the hand-off card the apps draw (spec 102 D4,
	 * `HandoffCard`), over the multisig's sheet.
	 *
	 * Deliberately not a preview: the page reads the SafeTx itself and is the
	 * authority. The card says that the approval is reviewed there, which key
	 * confirms it, what was checked about the page (and when) — and Open, which
	 * a page that failed its check never gets. Open is the tap a browser needs
	 * to open a window; after it, a wait, with the way back to the page.
	 * Closing the card gives up: nothing is approved.
	 */
	import HandoffCard from '$lib/signing/ui/HandoffCard.svelte';
	import type { HandoffMessages } from '$lib/signing/messages';
	import type { HandoffModel } from '$lib/signing/model';
	import { integrityLineModel } from '$lib/settings/venue';
	import { signerIntegrityTime } from '$lib/core/kernels';
	import Sheet from './Sheet.svelte';
	import { pageHandoff } from '../core/page-approval.svelte';

	interface Props {
		m: HandoffMessages;
		locale: string;
		wide: boolean;
		closeLabel: string;
	}

	let { m, locale, wide, closeLabel }: Props = $props();

	const handoff = $derived(pageHandoff.current);

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
			title: m.title,
			key: {
				label: key ? (m.keyLabels[key.label_key] ?? '') : '',
				value: key ? (key.name ?? m.places[key.place_key] ?? '') : h.member
			},
			page: { name: h.official ? m.official : h.host, host: h.host },
			integrity,
			open: { label: m.open, enabled: h.phase === 'ready' || h.phase === 'waiting' },
			waiting:
				h.phase === 'waiting'
					? { title: m.waiting, hint: m.waitingHint, reopen: m.reopen }
					: undefined
		};
	});
</script>

{#if handoff !== null && model !== null}
	<Sheet title={m.title} {closeLabel} {wide} onclose={() => handoff.cancel()}>
		<div data-testid="page-handoff" data-phase={handoff.phase}>
			<HandoffCard handoff={model} onopen={() => handoff.open()} />
		</div>
	</Sheet>
{/if}
