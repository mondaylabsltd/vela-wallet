/**
 * Why a proposal was not made, as the screens have always said it: the
 * proposals machine's `ActionState` reads back as one of these
 * (`shell/proposal-failure.ts`), so every sheet keeps its words.
 */

/**
 * The mailbox's list of waiting transactions (or the chain's members) could
 * not be read, so nothing was proposed: there is no nonce to take that is
 * known to be free.
 */
export class MailboxUnreadableError extends Error {
	constructor() {
		super("the multisig's waiting transactions could not be read");
		this.name = 'MailboxUnreadableError';
	}
}

/**
 * r13-3: the reads a proposal is built from did not come back — the
 * multisig's network (`reasonKey` `home.multisig.proposal.unread.chain`), or
 * the network AND the multisig service (`.both`) — failed, or still out at
 * the core's bound: nothing was proposed. Said with the core's key, never as
 * the waiting transactions the service holds.
 */
export class ProposalUnreadError extends Error {
	constructor(readonly reasonKey: string) {
		super(`the proposal's reads did not come back (${reasonKey})`);
		this.name = 'ProposalUnreadError';
	}
}

/**
 * No member is on this device, and none can sign through an outside wallet:
 * every member is a Vela account, which approves in Vela on its own device.
 * Nothing was proposed.
 */
export class NoOutsideSignerError extends Error {
	constructor() {
		super('no member can sign through an outside wallet');
		this.name = 'NoOutsideSignerError';
	}
}

/** The mailbox refused a submission (the core's reason, for the log). */
export class MailboxRefusedError extends Error {
	constructor(readonly refusal: unknown) {
		super(`the mailbox refused it: ${JSON.stringify(refusal)}`);
		this.name = 'MailboxRefusedError';
	}
}
