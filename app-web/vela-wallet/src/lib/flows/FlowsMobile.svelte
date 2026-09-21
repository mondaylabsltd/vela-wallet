<script lang="ts">
	/**
	 * The phone host: one flow state, rendered.
	 *
	 * Takes a `FlowScreenModel` and draws its base screen plus, where the state
	 * has one, the sheet over it. Every screen in the four journeys goes
	 * through here, so the gallery and the real app render the same thing by
	 * construction rather than by discipline — there is no second code path for
	 * either to drift down.
	 */
	import type { Snippet } from 'svelte';
	import BottomSheet from '$lib/wallet/ui/BottomSheet.svelte';
	import FlowScreen from './ui/FlowScreen.svelte';
	import ScanSurface from './ui/ScanSurface.svelte';
	import ShareCard from './ui/ShareCard.svelte';
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
	import TokenDetail from './screens/TokenDetail.svelte';
	import TxDetail from './screens/TxDetail.svelte';
	import type { FlowScreenModel } from './model';

	/**
	 * The live send flow's handlers (spec 026). When present, a tap becomes an
	 * EVENT for the `send` core and the core's own stage decides which screen
	 * shows next; when absent, the fixture navigation below runs and the
	 * gallery renders the drawn journey exactly as 021 drew it.
	 */
	interface SendActions {
		selectToken(index: number): void;
		amountChanged(value: string): void;
		recipientChanged(value: string): void;
		advance(): void;
		confirm(): void;
		/** "+ add recipient" — the core turns one recipient into many. */
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
		/**
		 * The picker's two sweep affordances (spec 028 T440): the master tick,
		 * and the CTA that either opens the multi-select or confirms it. Which
		 * of the two the CTA is right now is the route's to say — it knows
		 * whether the checkboxes are showing.
		 */
		selectAll(): void;
		pickCta(): void;
		/**
		 * The recipient picker's two answers (spec 028 US5): one person from
		 * the book, or a whole group as split-mode recipients. Indices into the
		 * live `ContactPickModel`'s lists.
		 */
		/**
		 * The fee row's ⟳ and the folded speed control (spec 068). All three
		 * are the session's: a refresh is a fresh measurement on the SAME
		 * quote session, and a tier pick re-prices this one send without
		 * touching the stored default.
		 */
		refreshFee?(): void;
		toggleSpeed?(): void;
		pickSpeed?(id: string): void;
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
		/** The core's gates — `can_continue` / `can_confirm`. */
		continueDisabled: boolean;
		/** The core's `estimating_gas`: Continue is out checking, not refused. */
		continueBusy?: boolean;
		/** One amount into every split row that has none. */
		fillEmptyAmounts?(amount: string): void;
		confirmDisabled: boolean;
	}

	/** The add-token sheet's handlers, when the `manage_tokens` core is live. */
	interface AddTokenActions {
		input(value: string): void;
		submit(): void;
		/** The ERC-20 / native toggle, and a chain picked on the native tab (Phase 10). */
		tab?(id: string): void;
		pick?(id: string): void;
	}

	/**
	 * The scanner's live half (spec 028 T422). Absent — in the gallery — the
	 * surface draws the inert frame it has always drawn; present, `feed` is the
	 * `<video>` the camera writes into and `notice` is why there is nothing in
	 * it.
	 */
	interface ScanActions {
		feed: Snippet;
		notice?: string;
		tool(id: 'gallery' | 'torch' | 'flip'): void;
	}

	/** The batch importer's handlers, when its own core is live. */
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
		model: FlowScreenModel;
		/** Absent in the gallery, where the screens are pictures. */
		onback?: () => void;
		onnavigate?: (to: string, index?: number) => void;
		send?: SendActions;
		batch?: BatchActions;
		scan?: ScanActions;
		addToken?: AddTokenActions;
		/**
		 * The person dismissed the sheet (spec 028 T442). A sheet is a pushed
		 * step on the route's stack, and a route that does not hear the
		 * dismissal keeps the step — so the next `go()` to the same sheet is a
		 * no-op and nothing opens. Absent in the gallery.
		 */
		onsheetclose?: () => void;
		/** The open transaction's delete (spec 028 Phase 8). Absent in the gallery. */
		ondeletetx?: () => void;
		/**
		 * The header pill (spec 028 Phase 10): the network filter, which on the
		 * phone is a sheet the route raises. Absent in the gallery.
		 */
		onchains?: () => void;
	}

	let {
		model,
		onback,
		onnavigate,
		send,
		batch,
		scan,
		addToken,
		onsheetclose,
		ondeletetx,
		onchains
	}: Props = $props();

	const base = $derived(model.base);
	const sheet = $derived(model.sheet);

	let sheetClosed = $state(false);
	const showSheet = $derived(sheet !== undefined && !sheetClosed);

	// A new state means a new sheet; without this the dismissal of the last
	// one would suppress the next.
	$effect(() => {
		void model.state;
		sheetClosed = false;
	});

	const go = (to: string, index?: number) => onnavigate?.(to, index);
</script>

<div class="host" style:--text-scale={model.textScale === 1 ? undefined : model.textScale}>
	{#if base.kind === 'scan'}
		<ScanSurface
			model={base.model}
			feed={scan?.feed}
			notice={scan?.notice}
			ontool={scan ? (id) => scan.tool(id) : undefined}
			onclose={onback}
		/>
	{:else if base.kind === 'share-card'}
		<!-- Not a screen: the saved image, shown on its own so the gallery and
		     the save path render the very same artwork. -->
		<div class="card-stage"><ShareCard model={base.model} /></div>
	{:else if base.kind === 'receive-list'}
		<FlowScreen header={base.model.header} {onback}>
			<ReceiveList model={base.model} onqr={(i) => go('receive-qr', i)} />
		</FlowScreen>
	{:else if base.kind === 'history'}
		<FlowScreen header={base.model.header} {onback} onpill={onchains}>
			<History model={base.model} onselect={(g, r) => go('tx-detail', g * 100 + r)} />
		</FlowScreen>
	{:else if base.kind === 'assets'}
		<FlowScreen
			header={base.model.header}
			{onback}
			onaction={() => go('add-token')}
			onpill={onchains}
		>
			<Assets
				model={base.model}
				onselect={(i) => go('token-detail', i)}
				onadd={() => go('add-token')}
				onreceive={() => go('receive')}
			/>
		</FlowScreen>
	{:else if base.kind === 'send-pick'}
		<FlowScreen header={base.model.header} {onback} onpill={onchains}>
			<SendPick
				model={base.model}
				onfilter={send?.filterClass ? (id) => send.filterClass?.(id) : undefined}
				onselect={(i) => (send ? send.selectToken(i) : go('send-form', i))}
				onselectall={send ? () => send.selectAll() : undefined}
				oncta={() => (send ? send.pickCta() : go('send-multi'))}
			/>
		</FlowScreen>
	{:else if base.kind === 'send-form'}
		<FlowScreen header={base.model.header} {onback}>
			<SendForm
				model={base.model}
				onpickRecipient={() => go('contact-pick')}
				onscan={() => go('scan')}
				onfee={() => go('fee-token')}
				onfeerefresh={send?.refreshFee ? () => send.refreshFee?.() : undefined}
				onspeed={send?.toggleSpeed ? () => send.toggleSpeed?.() : undefined}
				onspeedpick={send?.pickSpeed ? (id) => send.pickSpeed?.(id) : undefined}
				ondenom={send ? () => send.toggleDenom() : undefined}
				onmax={send ? () => send.max() : undefined}
				onrecipientAction={(id) => {
					// The split form's three pills (Phase 10): a blank row, the book
					// into a new row, the importer sheet. Live, the first two are the
					// session's; the gallery keeps the drawn navigation.
					if (id === 'add' && send) send.addRecipient();
					else if (id === 'contacts' && send?.pickContactFor) send.pickContactFor(null);
					else
						go(
							id === 'import'
								? 'batch-import'
								: id === 'contacts'
									? 'contact-pick'
									: 'add-recipient'
						);
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
				onfillEmpty={send?.fillEmptyAmounts
					? (amount) => send.fillEmptyAmounts?.(amount)
					: undefined}
			/>
		</FlowScreen>
	{:else if base.kind === 'send-confirm'}
		<FlowScreen header={base.model.header} {onback}>
			<SendConfirm
				model={base.model}
				onconfirm={() => (send ? send.confirm() : go('send-receipt'))}
			/>
		</FlowScreen>
	{:else}
		<FlowScreen header={base.model.header} {onback}>
			<SendReceipt model={base.model} oncta={() => (send ? send.done() : go('done'))} />
		</FlowScreen>
	{/if}

	{#if showSheet && sheet !== undefined}
		{#if sheet.kind === 'receive-qr'}
			<BottomSheet
				title={sheet.model.title}
				closeLabel={sheet.model.closeLabel}
				height="tall"
				hideTitle
				onclose={() => (sheetClosed = true)}
			>
				<ReceiveQr model={sheet.model} />
			</BottomSheet>
		{:else if sheet.kind === 'tx-detail'}
			<BottomSheet
				title={sheet.model.title}
				closeLabel={sheet.model.closeLabel}
				height="tall"
				hideTitle
				onclose={() => (sheetClosed = true)}
			>
				<TxDetail model={sheet.model} ondelete={ondeletetx} />
			</BottomSheet>
		{:else if sheet.kind === 'token-detail'}
			<BottomSheet
				title={sheet.model.symbol}
				closeLabel={sheet.model.closeLabel}
				height="tall"
				hideTitle
				onclose={() => (sheetClosed = true)}
			>
				<TokenDetail
					model={sheet.model}
					onselect={(i) => {
						const target = sheet.model.rowTargets?.[i];
						if (target !== undefined) go('tx-detail', target);
					}}
					onsend={() => go('send-token')}
					onreceive={() => go('receive-token')}
				/>
			</BottomSheet>
		{:else if sheet.kind === 'add-token'}
			<BottomSheet
				title={sheet.model.title}
				closeLabel={sheet.model.closeLabel}
				height="tall"
				onclose={() => {
					sheetClosed = true;
					onsheetclose?.();
				}}
			>
				<AddToken
					model={sheet.model}
					oninput={addToken ? (value) => addToken.input(value) : undefined}
					onsubmit={addToken ? () => addToken.submit() : undefined}
					ontab={addToken?.tab ? (id) => addToken.tab?.(id) : undefined}
					onpick={addToken?.pick ? (id) => addToken.pick?.(id) : undefined}
				/>
			</BottomSheet>
		{:else if sheet.kind === 'contact-pick'}
			<BottomSheet
				title={sheet.model.title}
				closeLabel={sheet.model.closeLabel}
				height="tall"
				onclose={() => (sheetClosed = true)}
			>
				<ContactPick
					model={sheet.model}
					onscan={() => go('scan')}
					onselect={send?.pickContact ? (i) => send.pickContact?.(i) : undefined}
					ongroup={send?.pickGroup ? (i) => send.pickGroup?.(i) : undefined}
				/>
			</BottomSheet>
		{:else if sheet.kind === 'fee-token'}
			<BottomSheet
				title={sheet.model.title}
				closeLabel={sheet.model.closeLabel}
				onclose={() => (sheetClosed = true)}
			>
				<FeeTokenPick
					model={sheet.model}
					onselect={send ? (i) => send.pickFeeToken(i) : undefined}
				/>
			</BottomSheet>
		{:else}
			<BottomSheet
				title={sheet.model.title}
				closeLabel={sheet.model.closeLabel}
				height="tall"
				onclose={() => (sheetClosed = true)}
			>
				<BatchImport
					model={sheet.model}
					onunit={batch ? (id) => batch.unit(id) : undefined}
					onpaste={batch ? (text) => batch.paste(text) : undefined}
					onrate={batch ? (text) => batch.rate(text) : undefined}
					onresetrate={batch ? () => batch.resetRate() : undefined}
					onfile={batch ? () => batch.pickFile() : undefined}
					ontemplate={batch ? () => batch.saveTemplate() : undefined}
					onapply={batch ? () => batch.apply() : undefined}
					onmerge={batch ? () => batch.toggleMerge() : undefined}
				/>
			</BottomSheet>
		{/if}
	{/if}
</div>

<style>
	.host {
		position: relative;
		height: 100%;
		background: var(--color-bg-base);
		overflow: hidden;
	}

	.card-stage {
		display: flex;
		align-items: center;
		justify-content: center;
		height: 100%;
		/* The card is a fixed 480 wide and the phone frame is not, so it
		   scales down to fit rather than being cropped by it. */
		container-type: inline-size;
		overflow: auto;
	}
</style>
