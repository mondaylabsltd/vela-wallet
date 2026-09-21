<script lang="ts">
	/**
	 * The desktop host: one flow state, in the third column.
	 *
	 * Same screen bodies as the phone — only the chrome differs. The phone
	 * pushes a `FlowScreen` or raises a `BottomSheet`; the desktop puts the
	 * identical body inside `ThirdPanel`, which is why every body here takes
	 * its model and nothing else.
	 *
	 * `ds1` is the one exception in the whole feature: a scanner is a viewfinder
	 * and a narrow column is the wrong shape for one, so the desktop shows it as
	 * a centred modal (DS1L) and this component does not draw it.
	 */
	import ThirdPanel from '$lib/wallet/ui/ThirdPanel.svelte';
	import AddToken from './screens/AddToken.svelte';
	import Assets from './screens/Assets.svelte';
	import BatchImport from './screens/BatchImport.svelte';
	import ContactPick from './screens/ContactPick.svelte';
	import FeeTokenPick from './screens/FeeTokenPick.svelte';
	import History from './screens/History.svelte';
	import ReceiveList from './screens/ReceiveList.svelte';
	import ReceiveQr from './screens/ReceiveQr.svelte';
	import SendConfirm from './screens/SendConfirm.svelte';
	import SendForm from './screens/SendForm.svelte';
	import SendPick from './screens/SendPick.svelte';
	import SendReceipt from './screens/SendReceipt.svelte';
	import TxDetail from './screens/TxDetail.svelte';
	import type { DesktopFlowModel } from './model';

	/**
	 * The live send flow's handlers (spec 028 T453) — the SAME interface
	 * `FlowsMobile` takes, because the desktop drives the same session through
	 * the same events. Until this phase the third column showed the send
	 * screens with live data and dead controls: a Continue that did nothing
	 * on a form that knew the balance.
	 */
	interface SendActions {
		selectToken(index: number): void;
		amountChanged(value: string): void;
		recipientChanged(value: string): void;
		advance(): void;
		confirm(): void;
		addRecipient(): void;
		removeRecipient(index: number): void;
		pickFeeToken(index: number): void;
		/** 最大 — the core's `tap_max`: the whole balance, net of the fee it estimates. */
		max(): void;
		/**
		 * ⇄ — the core's `toggle_fiat_input`: type the amount in the token or in
		 * the display currency. Drawn since 021 and wired to nothing until now
		 * (issue 197), which made it look like a dead icon.
		 */
		toggleDenom(): void;
		done(): void;
		selectAll(): void;
		pickCta(): void;
		/**
		 * The three surfaces the phone raises as sheets and this column opens
		 * as panels. They belong to the session, not to the nav stack — which
		 * is why the desktop asks the session rather than pushing a step.
		 */
		openFeeSheet(): void;
		openBatch(): void;
		openScanner(): void;
		/**
		 * The fee row's ⟳ and the folded speed control (spec 068). All three
		 * are the session's: a refresh is a fresh measurement on the SAME
		 * quote session, and a tier pick re-prices this one send without
		 * touching the stored default.
		 */
		refreshFee?(): void;
		toggleSpeed?(): void;
		pickSpeed?(id: string): void;
		/**
		 * The recipient picker's two answers (spec 028 US5, wired by the contacts
		 * session): one person from the book, or a whole group as split-mode
		 * recipients. Optional, and the same shape `FlowsMobile` declares.
		 */
		pickContact?(index: number): void;
		pickGroup?(index: number): void;
		/**
		 * The split rows (spec 028 Phase 10): a row typed into, the book opened
		 * for one row — or for a NEW row when `index` is null — and the
		 * picker's class chips.
		 */
		recipientRowChanged?(index: number, patch: { address?: string; amount?: string }): void;
		pickContactFor?(index: number | null): void;
		filterClass?(id: string): void;
		continueDisabled: boolean;
		/** The core's `estimating_gas`: Continue is out checking, not refused. */
		continueBusy?: boolean;
		/** One amount into every split row that has none. */
		fillEmptyAmounts?(amount: string): void;
		confirmDisabled: boolean;
	}

	interface BatchActions {
		/** Spec 038 #E6: the rate typed in place, and back to the fetched one. */
		rate: (text: string) => void;
		resetRate: () => void;
		unit(id: string): void;
		paste(text: string): void;
		pickFile(): void;
		saveTemplate(): void;
		apply(): void;
		/** Add to the people already on the form, or replace them. */
		toggleMerge(): void;
	}

	interface Props {
		model: DesktopFlowModel;
		onback?: () => void;
		onclose?: () => void;
		onnavigate?: (to: string, index?: number) => void;
		/** The add-token panel's handlers, when the `manage_tokens` core is live (spec 028). */
		addToken?: {
			input(value: string): void;
			submit(): void;
			tab?(id: string): void;
			pick?(id: string): void;
		};
		send?: SendActions;
		batch?: BatchActions;
		/** The open transaction's delete (spec 028 Phase 8). Absent in the gallery. */
		ondeletetx?: () => void;
	}

	let { model, onback, onclose, onnavigate, addToken, send, batch, ondeletetx }: Props = $props();

	const body = $derived(model.body);
	const go = (to: string, index?: number) => onnavigate?.(to, index);
</script>

<ThirdPanel
	title={model.title}
	closeLabel={model.closeLabel}
	backLabel={model.backLabel}
	scrollKey={body.kind}
	{onback}
	{onclose}
>
	{#if body.kind === 'receive-list'}
		<ReceiveList model={body.model} chrome={false} onqr={(i) => go('receive-qr', i)} />
	{:else if body.kind === 'receive-qr'}
		<ReceiveQr model={body.model} />
	{:else if body.kind === 'history'}
		<History model={body.model} onselect={(g, r) => go('tx-detail', g * 100 + r)} />
	{:else if body.kind === 'tx-detail'}
		<TxDetail model={body.model} ondelete={ondeletetx} />
	{:else if body.kind === 'assets'}
		<Assets
			model={body.model}
			onselect={(i) => go('token-detail', i)}
			onadd={() => go('add-token')}
			onreceive={() => go('receive')}
		/>
	{:else if body.kind === 'add-token'}
		<AddToken
			model={body.model}
			oninput={addToken ? (value) => addToken.input(value) : undefined}
			onsubmit={addToken ? () => addToken.submit() : undefined}
			ontab={addToken?.tab ? (id) => addToken.tab?.(id) : undefined}
			onpick={addToken?.pick ? (id) => addToken.pick?.(id) : undefined}
		/>
	{:else if body.kind === 'send-pick'}
		<SendPick
			model={body.model}
			onfilter={send?.filterClass ? (id) => send.filterClass?.(id) : undefined}
			onselect={(i) => (send ? send.selectToken(i) : go('send-form', i))}
			onselectall={send ? () => send.selectAll() : undefined}
			oncta={send ? () => send.pickCta() : undefined}
		/>
	{:else if body.kind === 'send-form'}
		<SendForm
			model={body.model}
			onpickRecipient={() => go('contact-pick')}
			onscan={() => (send ? send.openScanner() : go('scan'))}
			onfee={() => (send ? send.openFeeSheet() : go('fee-token'))}
			onfeerefresh={send?.refreshFee ? () => send.refreshFee?.() : undefined}
			onspeed={send?.toggleSpeed ? () => send.toggleSpeed?.() : undefined}
			onspeedpick={send?.pickSpeed ? (id) => send.pickSpeed?.(id) : undefined}
			ondenom={send ? () => send.toggleDenom() : undefined}
			onmax={send ? () => send.max() : undefined}
			onrecipientAction={(id) => {
				if (id === 'import') {
					if (send) send.openBatch();
					else go('batch-import');
				} else if (id === 'add' && send) send.addRecipient();
				else if (id === 'contacts' && send?.pickContactFor) send.pickContactFor(null);
				else go(id === 'contacts' ? 'contact-pick' : 'add-recipient');
			}}
			oncontinue={() => (send ? send.advance() : go('send-confirm'))}
			onaddRecipient={send ? () => send.addRecipient() : undefined}
			onremoveRecipient={send ? (i) => send.removeRecipient(i) : undefined}
			onrecipientRow={send?.recipientRowChanged
				? (i, patch) => send.recipientRowChanged?.(i, patch)
				: undefined}
			onpickRecipientRow={send?.pickContactFor ? (i) => send.pickContactFor?.(i) : undefined}
			onamount={send ? (value) => send.amountChanged(value) : undefined}
			onrecipient={send ? (value) => send.recipientChanged(value) : undefined}
			ctaDisabled={send?.continueDisabled ?? false}
			ctaBusy={send?.continueBusy ?? false}
			onfillEmpty={send?.fillEmptyAmounts ? (amount) => send.fillEmptyAmounts?.(amount) : undefined}
		/>
	{:else if body.kind === 'send-confirm'}
		<SendConfirm
			model={body.model}
			onconfirm={() => (send ? send.confirm() : go('send-receipt'))}
		/>
	{:else if body.kind === 'contact-pick'}
		<ContactPick
			model={body.model}
			onscan={() => (send ? send.openScanner() : go('scan'))}
			onselect={send?.pickContact ? (i) => send.pickContact?.(i) : undefined}
			ongroup={send?.pickGroup ? (i) => send.pickGroup?.(i) : undefined}
		/>
	{:else if body.kind === 'fee-token'}
		<FeeTokenPick model={body.model} onselect={send ? (i) => send.pickFeeToken(i) : undefined} />
	{:else if body.kind === 'batch-import'}
		<BatchImport
			model={body.model}
			onunit={batch ? (id) => batch.unit(id) : undefined}
			onpaste={batch ? (text) => batch.paste(text) : undefined}
			onrate={batch ? (text) => batch.rate(text) : undefined}
			onresetrate={batch ? () => batch.resetRate() : undefined}
			onfile={batch ? () => batch.pickFile() : undefined}
			ontemplate={batch ? () => batch.saveTemplate() : undefined}
			onapply={batch ? () => batch.apply() : undefined}
			onmerge={batch ? () => batch.toggleMerge() : undefined}
		/>
	{:else}
		<SendReceipt
			model={body.model}
			layout="column"
			oncta={() => (send ? send.done() : go('done'))}
		/>
	{/if}
</ThirdPanel>
