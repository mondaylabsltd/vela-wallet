/**
 * Where an approved request stands (spec 079): has its signature been made,
 * is the passkey prompt up. The sheet's ✕ opens only once the signature
 * exists and no prompt is up — closed earlier, a cancelled prompt would leave
 * the page with no answer at all.
 */
import { describe, expect, it } from 'vitest';
import { approvalProgress, IDLE_APPROVAL, type ApprovalInput } from './approval-progress';

const run = (...inputs: ApprovalInput[]) => inputs.reduce(approvalProgress, IDLE_APPROVAL);
const view = (requestId: string | null, inFlight: boolean): ApprovalInput => ({
	type: 'view',
	requestId,
	inFlight
});
const ceremony = (event: 'started' | 'signed' | 'failed'): ApprovalInput => ({
	type: 'ceremony',
	event
});

describe('approvalProgress', () => {
	it('an approval starts unsigned; the prompt going up and signing marks it signed', () => {
		expect(run(view('r1', false), view('r1', true))).toEqual({
			requestId: 'r1',
			signed: false,
			ceremonyUp: false
		});
		expect(run(view('r1', true), ceremony('started'))).toMatchObject({
			signed: false,
			ceremonyUp: true
		});
		expect(run(view('r1', true), ceremony('started'), ceremony('signed'))).toMatchObject({
			signed: true,
			ceremonyUp: false
		});
	});

	it('a cancelled or failed prompt signs nothing', () => {
		expect(run(view('r1', true), ceremony('started'), ceremony('failed'))).toMatchObject({
			signed: false,
			ceremonyUp: false
		});
	});

	it('a ceremony outside any approval (a backup, a sign-in) is not this sheet’s signature', () => {
		expect(run(view('r1', false), ceremony('started'), ceremony('signed'))).toMatchObject({
			requestId: null,
			signed: false
		});
	});

	it('the next approval starts clean — after an answer, a failure, or for another request', () => {
		const signed = [view('r1', true), ceremony('started'), ceremony('signed')];
		expect(run(...signed, view(null, false))).toMatchObject({ requestId: null, signed: false });
		expect(run(...signed, view('r1', false), view('r1', true))).toMatchObject({
			requestId: 'r1',
			signed: false
		});
		expect(run(...signed, view('r2', true))).toMatchObject({ requestId: 'r2', signed: false });
	});

	it('the same approval keeps its signature through the submission', () => {
		const state = run(view('r1', true), ceremony('started'), ceremony('signed'), view('r1', true));
		expect(state).toMatchObject({ requestId: 'r1', signed: true, ceremonyUp: false });
	});

	it('a second prompt in the same approval (a re-sign after sponsorship) is up again', () => {
		const state = run(
			view('r1', true),
			ceremony('started'),
			ceremony('signed'),
			ceremony('started')
		);
		expect(state).toMatchObject({ signed: true, ceremonyUp: true });
	});
});
