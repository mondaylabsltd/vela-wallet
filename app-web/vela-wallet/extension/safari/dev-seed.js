/**
 * DEVELOPMENT PACKAGE ONLY — point the wallet at the local fork harness
 * (`scripts/onchain/multisig-fork/up.sh`), then open the parallel space.
 *
 * The same keys the Playwright harness writes before a page runs
 * (`e2e/onchain/harness.ts` `useHarness` + `seedNetwork`), written here in the
 * extension's own origin because no test can reach Safari's storage from
 * outside. Every endpoint must be loopback http: this page exists for a
 * developer's machine and refuses to aim the wallet anywhere else.
 *
 *   dev-seed.html?genesis=<url>&relay=<url>&peerconnect=<ws url>
 *                &rpc=<chainId>=<url>[&rpc=…]&locale=<en|zh|…>
 *                [&index=<url>&page=<url>&next=create]
 *
 * The Safari route's e2e (spec 102 R8) creates a REAL wallet with the
 * simulator's own passkey instead of entering the parallel space: `index` is
 * the local passkey index (`app-web/trusted-signer/samples/mock-index.mjs`,
 * so no run writes to the real one), `page` the signing page on this
 * machine's loopback, which onboarding then creates and signs in on
 * (`src/lib/dev/harness-page.ts`), and `next=create` opens the create flow.
 */
const LOOPBACK = /^(?:https?|wss?):\/\/(?:127\.0\.0\.1|localhost)(?::\d+)?(?:\/|$)/;
const out = document.getElementById('out');
const say = (text) => {
	if (out) out.textContent = text;
};

function loopback(value) {
	return typeof value === 'string' && LOOPBACK.test(value) ? value : null;
}

function seedNetworks(networks) {
	return new Promise((resolve, reject) => {
		const open = indexedDB.open('vela', 1);
		open.onupgradeneeded = () => open.result.createObjectStore('kv');
		open.onerror = () => reject(open.error);
		open.onsuccess = () => {
			const tx = open.result.transaction('kv', 'readwrite');
			tx.objectStore('kv').put(JSON.stringify(networks), 'vela.networkConfig');
			tx.oncomplete = () => resolve();
			tx.onerror = () => reject(tx.error);
		};
	});
}

async function seed() {
	const params = new URLSearchParams(location.search);
	const genesis = loopback(params.get('genesis'));
	const relay = loopback(params.get('relay'));
	const peerconnect = loopback(params.get('peerconnect'));
	const index = loopback(params.get('index'));
	const page = loopback(params.get('page'));
	const create = params.get('next') === 'create';
	const networks = params
		.getAll('rpc')
		.map((entry) => {
			const at = entry.indexOf('=');
			const chainId = Number(entry.slice(0, at));
			const rpcURL = loopback(entry.slice(at + 1));
			return Number.isInteger(chainId) && chainId > 0 && rpcURL ? { chainId, rpcURL } : null;
		})
		.filter(Boolean);
	const locale = /^[a-z]{2}(?:-[A-Za-z]{2})?$/.test(params.get('locale') ?? '')
		? params.get('locale')
		: 'en';

	localStorage.setItem('vela.intro.seen', String(Date.now()));
	// The wallet's language everywhere (its request tabs included), not just
	// the page this opens next.
	localStorage.setItem('vela.language', locale);
	if (genesis) localStorage.setItem('vela.genesisServiceURL', genesis);
	if (page) localStorage.setItem('vela.dev.signingPage', page);
	else localStorage.removeItem('vela.dev.signingPage');
	if (relay || genesis || index) {
		localStorage.setItem(
			'vela.serviceEndpoints',
			JSON.stringify({
				...(relay ? { bundlerServiceURL: relay } : {}),
				...(genesis ? { genesisServiceURL: genesis } : {}),
				...(index ? { passkeyIndexURL: index } : {})
			})
		);
	}
	if (peerconnect) localStorage.setItem('vela.peerconnectRelay', peerconnect);
	if (networks.length) await seedNetworks(networks);
	say(`seeded: ${JSON.stringify({ genesis, relay, peerconnect, index, page, networks })}`);
	location.replace(create ? `${locale}/create.html` : `${locale}/parallel.html`);
}

seed().catch((error) => say(`seed failed: ${error?.message ?? error}`));
