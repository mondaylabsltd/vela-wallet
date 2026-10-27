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
	import type { IntentAction } from '$lib/core/generated/IntentAction';
	import { intentApprovalDetail } from '$lib/core/client';
	import { intents } from '$lib/intents/index.svelte';
	import { contactsBook } from '$lib/contacts/core/contacts-book.svelte';
	import { session } from '$lib/session/core/session.svelte';
	import PromptSheet from '$lib/ui/onboarding/v2/PromptSheet.svelte';
	import type { Status } from './clientmcp';
	import type { AgentMessages } from './messages';

	interface Props {
		messages: AgentMessages;
		status: Status | 'off';
	}

	let { messages, status }: Props = $props();

	const name = (type: string) => messages.actions[type] ?? type;
	const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

	/**
	 * What the request names, as the person knows it: a contact's or wallet's
	 * name — and, the core's, the network, URL or record it changes.
	 */
	function detail(action: IntentAction): string | null {
		switch (action.type) {
			case 'delete_contact': {
				const known = contactsBook.view?.contacts.find((c) => same(c.address, action.address));
				return known?.name ?? action.address;
			}
			case 'delete_group':
				return contactsBook.view?.groups.find((g) => g.id === action.id)?.name ?? action.id;
			case 'remove_wallet':
			case 'switch_account': {
				const row = session.view.accounts.find((r) => same(r.account.address, action.address));
				return row ? `${row.account.name} · ${action.address}` : action.address;
			}
			case 'clear_storage_item':
				return action.item;
			case 'set_preference':
				return `${action.preference.kind}: ${String(action.preference.value)}`;
			case 'set_balance_privacy':
				return action.hidden ? messages.hideBalance : messages.showBalance;
			default:
				return JSON.parse(intentApprovalDetail(JSON.stringify(action))) as string | null;
		}
	}

	const pending = $derived(intents.approvals[0]);

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

{#if status === 'online' || pending !== undefined}
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
	{@const what = detail(pending.action)}
	{#key pending.id}
		<PromptSheet
			copy={{
				title: messages.approveTitle,
				message: `${name(pending.action.type)}${what === null ? '' : ` — ${what}`}. ${messages.approveHint}`,
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
	.bar {
		position: fixed;
		top: calc(var(--space-md) + env(safe-area-inset-top, 0px));
		left: 50%;
		transform: translateX(-50%);
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

	.what {
		color: var(--color-fg-muted);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		min-width: 0;
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
