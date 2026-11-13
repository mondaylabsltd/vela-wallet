<script lang="ts">
	/**
	 * The AI agent, over every page (spec 103): what it is doing, the request
	 * it waits on the person for, and the way to stop it.
	 *
	 * - The bar says the agent is connected, and what it did last.
	 * - An agent's manage-level request is asked here, in the wallet — not in
	 *   the agent's chat — with the same prompt the wallet's own questions use.
	 *   Dismissing it is a no.
	 * - Stop refuses everything of the agent's still waiting, and switches the
	 *   agent off (the person's own setting, through the same intent Settings
	 *   uses); turning it back on is in Settings.
	 */
	import { untrack } from 'svelte';
	import { page } from '$app/state';
	import type { IntentAction } from '$lib/core/generated/IntentAction';
	import { intentApprovalDetailIn } from '$lib/core/client';
	import { worldJson } from '$lib/intents/world';
	import { networkSpace } from '$lib/network-space/network-space.svelte';
	import { intents, world } from '$lib/intents/index.svelte';
	import { contactsBook } from '$lib/contacts/core/contacts-book.svelte';
	import { networkAdmin } from '$lib/settings/core/network-admin.svelte';
	import { feed } from '$lib/wallet/core/feed.svelte';
	import PromptSheet from '$lib/ui/onboarding/v2/PromptSheet.svelte';
	import type { Status } from './clientmcp';
	import { approvalDetail, approvalMessage } from './detail';
	import type { AgentMessages } from './messages';

	interface Props {
		messages: AgentMessages;
		status: Status | 'off' | 'elsewhere';
		/** Take the agent back into this tab from the one that has it. */
		onusehere?: () => void;
	}

	let { messages, status, onusehere }: Props = $props();

	const name = (type: string) => messages.actions[type] ?? type;

	/**
	 * What the request names, as the person knows it: a contact's, wallet's
	 * or network's name, a setting as Settings says it — and, the core's, the
	 * wallets a switch moves between.
	 */
	function detail(action: IntentAction): string | null {
		const book = contactsBook.view;
		const net = networkAdmin.view;
		const checking = net.wizard.chain_info;
		return approvalDetail(action, {
			messages,
			locale: page.params.locale ?? 'en',
			contacts: book?.contacts ?? [],
			groups: book?.groups ?? [],
			networks: net.networks,
			checking: checking ? { chain_id: checking.chain_id, name: checking.name } : null,
			records: feed.view.transactions,
			space: networkSpace.words,
			core: (request) =>
				JSON.parse(intentApprovalDetailIn(JSON.stringify(request), worldJson(world()))) as
					string | null
		});
	}

	const pending = $derived(intents.approvals[0]);

	/**
	 * What the request changes, read once as it comes up. Reading the wallet
	 * (`world()`) brings the transfers up to date — a write, which a template
	 * may not make (Svelte refuses it, and the sheet would not draw at all).
	 */
	let what = $state<string | null>(null);
	$effect(() => {
		const request = pending;
		what = request === undefined ? null : untrack(() => detail(request.action));
	});

	/** How long a request is up before its Allow takes a tap. */
	const ARM_MS = 700;
	/**
	 * Each request is a prompt of its own (`#key`), and its Allow is armed only
	 * once it has been up a moment: a tap already on its way to Allow does not
	 * land on the next request.
	 */
	let armed = $state(false);
	$effect(() => {
		const id = pending?.id;
		armed = false;
		if (id === undefined) return;
		const timer = setTimeout(() => (armed = true), ARM_MS);
		return () => clearTimeout(timer);
	});
	const latest = $derived(intents.activity[0]);

	/**
	 * agent-19: on a wide screen the bar sits at the foot of the sidebar rail,
	 * which keeps that room free while it is up (`html[data-agent-bar]`) —
	 * centred at the top, it covered the pages' own header controls.
	 */
	const barShown = $derived(status === 'elsewhere' || status === 'online' || pending !== undefined);
	$effect(() => {
		if (!barShown) return;
		const root = document.documentElement;
		root.dataset.agentBar = '';
		return () => delete root.dataset.agentBar;
	});

	function stop(): void {
		intents.stop();
		void intents.invoke('person', {
			type: 'configure_agent',
			enabled: false,
			bridge_url: null,
			client_token: null
		});
	}
</script>

{#if status === 'elsewhere' && pending === undefined}
	<!-- Another tab of this wallet answers the agent: said here on every
	     size, with the way to take it back. -->
	<div class="bar" role="status" data-testid="agent-bar">
		<span class="dot elsewhere" aria-hidden="true"></span>
		<span class="note" data-testid="agent-elsewhere">{messages.elsewhere}</span>
		<button type="button" class="here" onclick={() => onusehere?.()}>{messages.useHere}</button>
	</div>
{:else if status === 'online' || pending !== undefined}
	<div class="bar" role="status" data-testid="agent-bar">
		<span class="dot" class:busy={latest?.stage != null} aria-hidden="true"></span>
		<span class="who">{messages.title}</span>
		{#if latest !== undefined}
			<span class="what">{name(latest.action)}</span>
		{:else}
			<span class="what">{messages.connected}</span>
		{/if}
		<button type="button" class="stop" onclick={stop}>{messages.stop}</button>
	</div>
{/if}

{#if pending !== undefined}
	{#key pending.id}
		<PromptSheet
			copy={{
				title: messages.approveTitle,
				message: approvalMessage(name(pending.action.type), what, messages),
				confirm: { confirmLabel: messages.allow, cancelLabel: messages.deny }
			}}
			dismissLabel={messages.deny}
			confirmDisabled={!armed}
			refuseFirst
			onAnswer={(approved) => intents.answer(pending.id, approved)}
		/>
	{/key}
{/if}

<style>
	/* A phone's header carries the account's name at its left: the bar keeps to
	   the right corner, compact, so the name stays in sight and in reach. The
	   wide layout puts it at the foot of the sidebar rail, which keeps that
	   room for it (agent-19: centred at the top, it covered the pages' search
	   fields and buttons), with what the agent did last. */
	.bar {
		position: fixed;
		top: calc(var(--space-md) + env(safe-area-inset-top, 0px));
		right: var(--space-lg);
		z-index: 70;
		display: flex;
		align-items: center;
		gap: var(--space-md);
		max-width: calc(100vw - 2 * var(--space-xl));
		padding: var(--space-sm) var(--space-sm) var(--space-sm) var(--space-lg);
		border-radius: var(--radius-full);
		border: var(--border-hairline) solid var(--color-border-base);
		background: var(--color-bg-raised);
		box-shadow: var(--shadow-md);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		color: var(--color-fg-base);
	}

	.dot {
		width: var(--space-md);
		height: var(--space-md);
		border-radius: var(--radius-full);
		background: var(--color-success-base);
		flex-shrink: 0;
	}

	.dot.busy {
		background: var(--color-info-base);
		animation: pulse 1s ease-in-out infinite;
	}

	@keyframes pulse {
		50% {
			opacity: 0.35;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.dot.busy {
			animation: none;
		}
	}

	.who {
		font-weight: var(--weight-semibold);
		white-space: nowrap;
	}

	.dot.elsewhere {
		background: var(--color-fg-subtle);
	}

	/* Another tab has the agent: the one line this tab says, on every size. */
	.note {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		color: var(--color-fg-muted);
	}

	.here {
		flex-shrink: 0;
		min-height: var(--size-control-sm);
		padding-inline: var(--space-lg);
		border: var(--border-hairline) solid var(--color-border-base);
		border-radius: var(--radius-full);
		background: var(--color-bg-base);
		color: var(--color-fg-base);
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		cursor: pointer;
	}

	.what {
		display: none;
		color: var(--color-fg-muted);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		min-width: 0;
	}

	@media (min-width: 1280px) {
		/* The rail's own width (`Sidebar.svelte`), less its padding. */
		.bar {
			top: auto;
			right: auto;
			left: var(--space-xl);
			bottom: var(--space-xl);
			box-sizing: border-box;
			width: calc(var(--space-4xl) * 7 - var(--space-xl));
			max-width: none;
		}

		.what {
			display: inline;
			flex: 1;
		}
	}

	.stop {
		flex-shrink: 0;
		min-height: var(--size-control-sm);
		padding-inline: var(--space-lg);
		border: none;
		border-radius: var(--radius-full);
		background: var(--color-error-soft);
		color: var(--color-error-base);
		font-family: var(--font-ui);
		font-size: calc(var(--text-sm) * var(--text-scale, 1));
		cursor: pointer;
	}
</style>
