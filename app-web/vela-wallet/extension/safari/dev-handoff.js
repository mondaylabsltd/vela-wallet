/**
 * DEVELOPMENT PACKAGE ONLY — the trusted signing page's hand-off from a real
 * `safari-web-extension://` page, over the very transport the wallet's
 * `handOff` uses in the Safari package (`src/lib/extension/safari-signer.ts`:
 * the page in a tab, its messages through Vela's content script on it), with
 * `handOff`'s own sequence — wait for `ready`, post the intent, hear `result`
 * or `error` — against a page on this machine's loopback
 * (`app-web/trusted-signer/samples/serve-dist.mjs`).
 *
 * The wallet cannot be pointed at a loopback page for a `getvela.app` account
 * (a page reaches only its own domain's keys — vela-core `signing_venue` R1),
 * and a simulator cannot map `sign.getvela.app` to this machine. So this page
 * drives the transport and the page — everything Safari decides — and the
 * wallet's `handOff` over the same transport is unit-tested
 * (`safari-signer.test.ts`).
 *
 *   dev-handoff.html?page=http://127.0.0.1:<port>/b/<hash>/sign&account=0x…
 */
import { safariSignerDeps } from '../../src/lib/extension/safari-signer';

const LOOPBACK = /^http:\/\/(?:127\.0\.0\.1|localhost)(?::\d+)?\//;
const out = document.getElementById('out');
const log = [];
const say = (line) => {
	log.push(line);
	out.textContent = log.join('\n');
};
const api = globalThis.chrome?.runtime ? globalThis.chrome : globalThis.browser;
const params = new URLSearchParams(location.search);
const page = params.get('page') ?? '';
const account = params.get('account') ?? '0xD400866e00B055B20752a826CD5C89b811de130b';
// `eth_sign`: what the page refuses whoever asks — its refusal must travel back.
const method = params.get('method') === 'eth_sign' ? 'eth_sign' : 'personal_sign';
say(`wallet: ${location.origin}`);
say(`page: ${page}`);

const deps = safariSignerDeps({ open: () => null, listen: () => () => {} }, api);

document.getElementById('open').addEventListener('click', () => {
	if (!LOOPBACK.test(page)) {
		say('refusing: the page must be on this machine’s loopback');
		return;
	}
	const target = new URL(page);
	target.searchParams.set('ch', 'post');
	target.searchParams.set('lang', params.get('lang') ?? 'en');
	const signer = deps.open(target.toString());
	document.title = 'handoff:opened';
	let handed = false;
	deps.listen((event) => {
		if (event.source !== signer || event.origin !== target.origin) return;
		const data = event.data ?? {};
		say(`heard from ${event.origin}: ${JSON.stringify(data).slice(0, 160)}`);
		if (data.vela === 'ready' && !handed) {
			handed = true;
			document.title = 'handoff:ready';
			signer.postMessage({
				vela: 'intent',
				v: 1,
				id: 'safari-1',
				intent: {
					method,
					params:
						method === 'eth_sign'
							? [account, `0x${'ab'.repeat(32)}`]
							: ['0x48656c6c6f2c2056656c61', account],
					origin: 'http://127.0.0.1:9893'
				},
				context: { account, chainId: 100 }
			});
			say('intent posted');
		} else if (data.vela === 'error') {
			document.title = `handoff:error:${data.code}`;
			signer.close();
		} else if (data.vela === 'result') {
			document.title = 'handoff:result';
			signer.close();
		}
	});
});
