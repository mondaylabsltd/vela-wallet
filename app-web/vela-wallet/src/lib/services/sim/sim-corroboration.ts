// Ported from src/services/sim-corroboration.ts @ f9bcb278 — RN seams rewritten to the web modules; logic verbatim.
/**
 * "Is every simulated asset movement already declared by the decoded hero?" —
 * the corroboration verdict behind the signing sheet's quiet green ✓.
 *
 * ---------------------------------------------------------------------------
 * OWNERSHIP: this rule is the SHELL's, on purpose. Written down here so the
 * next reader does not have to adjudicate it again.
 * ---------------------------------------------------------------------------
 *
 * 1. **No core holds a simulation, and none may.** `clear_signing`'s canon
 *    resolves CALLDATA into a descriptor and deliberately leaves simulated
 *    state to the `tx-simulation` service (the engines are RPC-side, and their
 *    answers are network facts, not portable rules). A machine that holds no
 *    simulated changes cannot adjudicate them, so there is no core to move
 *    this into — it is `no_core_owns_it` by design, not by neglect.
 * 2. **Both operands are already shell values.** `AssetChange[]` is what
 *    `tx-simulation` returned; `heroFlows` is what the decoded HERO is
 *    currently painting on this very screen. The question is "does the picture
 *    on screen account for everything the sim saw?", which is a statement
 *    about the rendered surface — the shell's job by construction.
 * 3. **There is no second implementation to drift against.** This is
 *    platform-neutral TypeScript and web and native run this same file, so it
 *    needs no parity gate (contrast `approval-guard-parity.test.ts`, which
 *    exists precisely because a Rust twin ships alongside a TS one).
 *
 * WHY IT IS A MODULE AND NOT A LOCAL HELPER: jest is `testEnvironment: 'node'`
 * and matches `*.test.ts` only — nothing in this repo renders a component — so
 * a verdict that decides whether an UNDECLARED OUTFLOW is itemised or folded
 * away behind a green checkmark would otherwise have no test coverage at all.
 * It was also stated TWICE inside `BalanceChangePreview.tsx` (once for the
 * preview, once for the 技术细节 summary line), and two copies of a
 * safety predicate is exactly how one of them quietly relaxes.
 *
 * NOT MONEY MATH: no scaling, no decimals, no unit conversion happens here —
 * only the SIGN of a delta and token identity are read, so there is no factor
 * that could go missing. Amounts are formatted elsewhere, and an
 * `unverified` change (decimals unconfirmed) can never corroborate anything.
 */

import type { AssetChange } from './tx-simulation';

/**
 * One asset movement the decoded hero already shows.
 * `token` is a LOWERCASED ERC-20 address; `undefined` means the native coin.
 */
export interface HeroFlow {
	token?: string;
	dir: 'out' | 'in';
}

/**
 * True only when the simulation is pure corroboration of the decoded hero:
 * EVERY simulated change maps to a same-token, same-direction hero flow and
 * none is `unverified`.
 *
 * Deliberately identity + direction PER CHANGE rather than a count budget: a
 * budget lets an undeclared outflow, a swapped output token, or an
 * unverified-decimals caution hide behind the ✓. Any unmatched movement makes
 * this false, and the caller then itemises the full list.
 *
 * The three guards are each load-bearing, and all three fail CLOSED (false =
 * "show the user everything"):
 *   - no hero flows at all (approvals, permits, batches) never corroborate;
 *   - an empty `changes` is not corroboration — `[].every()` is vacuously
 *     true, which would turn "the sim saw nothing" into "the sim agreed";
 *   - a single `unverified` change poisons the whole verdict, because a
 *     movement whose decimals could not be confirmed is not a movement we can
 *     claim to have matched.
 */
export function simCorroboratedByHero(
	changes: readonly AssetChange[],
	heroFlows: readonly HeroFlow[]
): boolean {
	if (heroFlows.length === 0 || changes.length === 0) return false;
	if (changes.some((c) => c.unverified)) return false;
	return changes.every((c) => {
		// Only an unverified change has no figure (PR 3), and those were
		// turned away above; a change with none corroborates nothing.
		const delta = c.delta;
		if (delta === undefined) return false;
		return heroFlows.some(
			(h) =>
				h.token === (c.token?.toLowerCase() ?? undefined) &&
				(h.dir === 'out' ? delta < 0n : delta > 0n)
		);
	});
}
