/**
 * DEVELOPMENT PACKAGE ONLY — what this Safari actually gives the extension,
 * measured where the wallet runs (an extension page), printed for a person or
 * a UI test to read. Every Safari claim in `extension/README.md` § Safari
 * comes from this page on a simulator or a device.
 */
const out = document.getElementById('out');
const facts = {};
const show = () => (out.textContent = JSON.stringify(facts, null, 1));

async function step(name, fn) {
	try {
		facts[name] = await fn();
	} catch (error) {
		facts[name] = `throws ${error?.name ?? ''}: ${error?.message ?? error}`;
	}
	show();
}

const api = globalThis.chrome?.runtime ? globalThis.chrome : globalThis.browser;

await step('origin', () => location.origin);
await step('chromeNamespace', () =>
	typeof globalThis.chrome === 'undefined'
		? 'absent'
		: globalThis.chrome === globalThis.browser
			? 'same as browser'
			: 'present'
);
await step('browserNamespace', () => typeof globalThis.browser);
await step('runtimeId', () => api.runtime.id);
await step('platform', async () => JSON.stringify(await api.runtime.getPlatformInfo()));
await step('storageSession', async () => {
	if (!api.storage?.session) return 'absent';
	await api.storage.session.set({ 'vela.probe': 1 });
	const got = await api.storage.session.get('vela.probe');
	return got['vela.probe'] === 1 ? 'works' : 'no read-back';
});
await step('chromePromises', async () => {
	const r = globalThis.chrome?.storage?.local?.get?.('vela.probe.none');
	return r && typeof r.then === 'function' ? 'promise' : String(typeof r);
});
await step('sidePanel', () => typeof api.sidePanel);
await step('windowsApi', () => typeof api.windows?.create);
await step('getContexts', () => typeof api.runtime.getContexts);
await step('wasm', async () => {
	// The smallest valid module: "\0asm" v1.
	const module = await WebAssembly.compile(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));
	return module instanceof WebAssembly.Module ? 'compiles' : 'no module';
});
await step('localStorage', () => {
	// Survives a Safari relaunch? Each launch appends the origin it ran at.
	const seen = JSON.parse(localStorage.getItem('vela.probe.origins') ?? '[]');
	if (seen.at(-1) !== location.origin) seen.push(location.origin);
	localStorage.setItem('vela.probe.origins', JSON.stringify(seen));
	return `origins seen: ${seen.length} ${JSON.stringify(seen)}`;
});
await step('storageLocalOrigins', async () => {
	const all = await api.storage.local.get('vela.probe.origins');
	const seen = Array.isArray(all['vela.probe.origins']) ? all['vela.probe.origins'] : [];
	if (seen.at(-1) !== location.origin) seen.push(location.origin);
	await api.storage.local.set({ 'vela.probe.origins': seen });
	return `origins seen: ${seen.length}`;
});
await step('indexedDB', () => typeof indexedDB);
await step('publicKeyCredential', () => typeof globalThis.PublicKeyCredential);
// A UI test reads the title: Safari exposes it where it does not expose <pre>.
document.title = 'probe:done';

/** WebAuthn needs a tap: the passkey prompt is a user-activation API. */
document.getElementById('passkey').addEventListener('click', async () => {
	facts.passkey = 'asking…';
	show();
	try {
		await navigator.credentials.get({
			publicKey: {
				challenge: crypto.getRandomValues(new Uint8Array(32)),
				rpId: 'getvela.app',
				userVerification: 'required',
				timeout: 15000
			}
		});
		facts.passkey = 'resolved (an assertion came back)';
	} catch (error) {
		facts.passkey = `${error?.name}: ${error?.message}`;
		document.title = `probe:passkey:${error?.name}`;
	}
	show();
});
