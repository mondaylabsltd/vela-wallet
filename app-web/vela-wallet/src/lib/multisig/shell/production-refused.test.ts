/**
 * r18-8 (§28, §32), end to end: a test build pointed at the live multisig
 * service, the person's Share again. The real guard (`guardFetch`, the core's
 * `productionWriteVerdict`) refuses the record's POST before it leaves; the
 * shell answers it `refused` (never the network's silence); the core asks
 * once — no 3x retry, no outage, nothing again by itself — and fails it as
 * `production_write_refused`, said in the test build's words.
 *
 * Nothing here can reach a production service: the guard's inner `fetch`
 * answers on this machine, and a write to a production host never gets to it.
 */
import '$lib/i18n/wasm-init.server';
import { generateKeyPairSync } from 'node:crypto';
import { describe, expect, it } from 'vitest';
import { productionWriteVerdict } from '../../../../../../rust/pkg-web/vela_core.js';
import { MultisigAccountsCore } from '$lib/core/client';
import { SERVICE_DEFAULTS } from '$lib/core/generated/serviceDefaults';
import { guardFetch, type Decide, type WriteVerdict } from '$lib/services/production-guard';
import type { MultisigAccountsEvent } from '../generated/MultisigAccountsEvent';
import type { MultisigAccountsView } from '../generated/MultisigAccountsView';
import { memoryStorage, offlineDeps, settled } from './accounts.testing';
import { failureWords } from './failure';
import { MultisigMachine } from './machine.svelte';
import { worldMultisig } from './members.testing';

const OWNER = '0x00000000000000000000000000000000000000aa';
const SENTENCE = "This test build won't write to the live service; point it at a local one.";

/** A P-256 key, uncompressed (`04…`): the owner's passkey, as the machine keeps it. */
function publicKeyHex(): string {
	const { publicKey } = generateKeyPairSync('ec', { namedCurve: 'prime256v1' });
	const der = publicKey.export({ format: 'der', type: 'spki' });
	return Buffer.from(der.subarray(der.length - 65)).toString('hex');
}

const hex = (text: string) => Buffer.from(text, 'utf8').toString('hex');

describe('a test build’s Share again to the live service (r18-8, end to end)', () => {
	it('is refused before it leaves, asked once, and said in the test build’s words', async () => {
		const made = worldMultisig([OWNER], 1);
		const storage = memoryStorage({
			'vela.multisigAccounts': JSON.stringify([
				{
					address: made.address,
					genesis: made.genesis,
					owners: [OWNER],
					threshold: 1,
					recorded: false,
					chainIds: [100],
					addedMs: 1
				}
			])
		});
		/** Every write the guard was asked about, and everything that got past it. */
		const judged: string[] = [];
		const reached: string[] = [];
		const decide: Decide = async (ask) => {
			judged.push(`${ask.method} ${new URL(ask.url).host}${new URL(ask.url).pathname}`);
			return JSON.parse(productionWriteVerdict(JSON.stringify(ask))) as WriteVerdict;
		};
		const inner = (async (input: RequestInfo | URL, init?: RequestInit) => {
			reached.push(`${init?.method ?? 'GET'} ${String(input)}`);
			return new Response('{"error":"not found"}', { status: 404 });
		}) as typeof fetch;
		const fetch = guardFetch(
			inner,
			() => ({ testBuild: true, allowProductionWrites: false }),
			decide
		);
		const machine = new MultisigMachine<MultisigAccountsView, MultisigAccountsEvent>(
			new MultisigAccountsCore(),
			offlineDeps(storage, {
				fetch,
				passkey: async () => ({
					type: 'asserted',
					credential_id: 'cred-1',
					authenticator_data: '00'.repeat(37),
					client_data_json: hex('{"type":"webauthn.get","challenge":"x"}'),
					signature: '30'.repeat(8)
				})
			}),
			'accounts'
		);
		machine.dispatch({
			type: 'start',
			service_url: SERVICE_DEFAULTS.genesisServiceUrl,
			accounts: [
				{
					address: OWNER,
					name: 'Me',
					keys: [{ credential_id: 'cred-1', public_key_hex: publicKeyHex() }],
					route: 'vela'
				}
			],
			networks: [100]
		});
		const key = made.address.toLowerCase();
		const ended = new Promise<MultisigAccountsView>((resolve) => {
			const stop = machine.listen((view) => {
				const phase = view.share_again[key];
				if (phase && phase !== 'working') {
					stop();
					resolve(view);
				}
			});
		});
		await new Promise<void>((resolve) => {
			if (machine.view.loaded) return resolve();
			const stop = machine.listen((view) => {
				if (view.loaded) {
					stop();
					resolve();
				}
			});
		});
		machine.dispatch({ type: 'share_again', address: made.address, owner: OWNER });
		const view = await ended;
		expect(view.share_again[key]).toBe('failed');
		const said = failureWords(view.share_again_failure[key], {
			notSent: 'not sent',
			reason: {
				passkeyClosed: 'closed',
				passkeyTimedOut: 'timed out',
				passkeyMissing: 'missing',
				passkeyFailed: 'failed',
				chainUnreachable: 'cannot reach {{network}}',
				unknown: 'something went wrong'
			}
		});
		expect(said).toBe(SENTENCE);
		expect(said).not.toMatch(/didn.t answer|can.t be reached|offline/i);

		// The background asks nothing again by itself: its own rounds run, and
		// the record's write is still the one the person asked for.
		for (let i = 0; i < 3; i += 1) {
			machine.dispatch({ type: 'tick' });
			await settled();
		}
		const posts = judged.filter((j) => j.startsWith('POST '));
		expect(posts).toHaveLength(1);
		expect(posts[0]).toMatch(/^POST safe-genesis\.getvela\.app\//);
		// Nothing written reached past the guard.
		expect(reached.filter((r) => !r.startsWith('GET '))).toEqual([]);
		machine.dispose();
	});
});
