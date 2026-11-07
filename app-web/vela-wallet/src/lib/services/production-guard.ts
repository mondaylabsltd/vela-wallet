/**
 * §28 (docs/multisig-machines.md): a test build never writes to production.
 *
 * The rule is the core's (`service_defaults::write_verdict`, wasm
 * `productionWriteVerdict`): in a TEST build, a write — any method but
 * GET/HEAD/OPTIONS; a JSON-RPC POST asking only reads is a read — to a
 * production host (`*.getvela.app` and the defaults' hosts) is refused
 * before it leaves the page, logged, and said honestly ("This test build
 * won't write to the live service; point it at a local one."). Reads stay
 * allowed.
 *
 * The web's test builds (`BuildFacts`):
 *   - the dev server (`vite dev`: `import.meta.env.DEV`);
 *   - the extension's DEVELOPMENT package (`extension/dist`), which carries
 *     `vela-test-build.json` (`extension/build.mjs`; its release and store
 *     packages are derived without it).
 * The hosted wallet and the release packages are none of them, and install
 * nothing: `installProductionWriteGuard` returns before touching `fetch`.
 *
 * The opt-in, off by default: `VELA_ALLOW_PRODUCTION_WRITES=1` when the dev
 * server starts or the package is built, or the dev setting
 * `localStorage['vela.dev.allowProductionWrites'] = '1'`.
 */
import {
	PRODUCTION_WRITE_REFUSED_ENGLISH,
	PRODUCTION_WRITE_REFUSED_KEY
} from '$lib/core/generated/serviceDefaults';

/** The dev setting that opts a test build in. */
export const OPT_IN_KEY = 'vela.dev.allowProductionWrites';
/** The development package's marker (`extension/build.mjs`). */
export const TEST_BUILD_MARKER = 'vela-test-build.json';

export interface GuardPolicy {
	testBuild: boolean;
	allowProductionWrites: boolean;
}

export type WriteVerdict =
	{ verdict: 'allowed' } | { verdict: 'refused'; host: string; key: string; message: string };

/** The core's answer for one request (`productionWriteVerdict`). */
export type Decide = (ask: {
	testBuild: boolean;
	allowProductionWrites: boolean;
	method: string;
	url: string;
	body?: string;
}) => Promise<WriteVerdict>;

/** A test build's write to a production service, refused before it left. */
export class ProductionWriteRefused extends TypeError {
	readonly host: string;
	readonly key = PRODUCTION_WRITE_REFUSED_KEY;
	constructor(host: string) {
		super(`${PRODUCTION_WRITE_REFUSED_ENGLISH} (${host})`);
		this.name = 'ProductionWriteRefused';
		this.host = host;
	}
}

/** Methods the core never counts as a write: no need to ask it. */
const READ_METHODS = new Set(['GET', 'HEAD', 'OPTIONS']);

async function bodyText(input: RequestInfo | URL, init?: RequestInit): Promise<string | undefined> {
	const body = init?.body;
	if (typeof body === 'string') return body;
	if (body instanceof URLSearchParams) return body.toString();
	if (body == null && input instanceof Request) {
		try {
			return await input.clone().text();
		} catch {
			return undefined;
		}
	}
	return undefined;
}

/**
 * `inner`, guarded: each request a test build would send is judged by
 * `decide` (the core) first. A refusal rejects with
 * {@link ProductionWriteRefused} and is logged; nothing reaches `inner`.
 */
export function guardFetch(
	inner: typeof fetch,
	policy: () => Promise<GuardPolicy> | GuardPolicy,
	decide: Decide
): typeof fetch {
	return async (input: RequestInfo | URL, init?: RequestInit) => {
		const method = (
			init?.method ?? (input instanceof Request ? input.method : 'GET')
		).toUpperCase();
		if (READ_METHODS.has(method)) return inner(input, init);
		const { testBuild, allowProductionWrites } = await policy();
		if (!testBuild || allowProductionWrites) return inner(input, init);
		const url = input instanceof Request ? input.url : String(input);
		const verdict = await decide({
			testBuild,
			allowProductionWrites,
			method,
			url,
			body: await bodyText(input, init)
		});
		if (verdict.verdict === 'refused') {
			console.warn(`[vela-wallet] guard: refused ${method} ${url} — ${verdict.message}`);
			throw new ProductionWriteRefused(verdict.host);
		}
		return inner(input, init);
	};
}

/** `1` or `true` opts in (the core's `opt_in`). */
export function optedIn(value: string | null | undefined): boolean {
	const v = (value ?? '').trim().toLowerCase();
	return v === '1' || v === 'true';
}

function devSetting(): boolean {
	try {
		return optedIn(localStorage.getItem(OPT_IN_KEY));
	} catch {
		return false;
	}
}

/** The extension's development package, read once from its marker. */
let packageMarker: Promise<{ testBuild: boolean; allowProductionWrites: boolean } | null> | null =
	null;
function devPackage(inner: typeof fetch): Promise<{
	testBuild: boolean;
	allowProductionWrites: boolean;
} | null> {
	packageMarker ??= inner(new URL(`/${TEST_BUILD_MARKER}`, location.origin))
		.then((r) => (r.ok ? (r.json() as Promise<Partial<GuardPolicy> | null>) : null))
		.then((m) =>
			m && m.testBuild === true
				? { testBuild: true, allowProductionWrites: m.allowProductionWrites === true }
				: null
		)
		.catch(() => null);
	return packageMarker;
}

/** This page's policy: the dev server, or the development package. */
export function thisBuild(inner: typeof fetch): () => Promise<GuardPolicy> {
	return async () => {
		if (import.meta.env.DEV) {
			return {
				testBuild: true,
				allowProductionWrites: optedIn(__VELA_ALLOW_PRODUCTION_WRITES__) || devSetting()
			};
		}
		if (__VELA_EXTENSION__) {
			const marker = await devPackage(inner);
			if (marker) {
				return {
					testBuild: true,
					allowProductionWrites: marker.allowProductionWrites || devSetting()
				};
			}
		}
		return { testBuild: false, allowProductionWrites: false };
	};
}

/** The core's door (loads the module once — a write is about to need it anyway). */
const coreDecide: Decide = async (ask) => {
	const { loadCore, productionWriteVerdict } = await import('$lib/core/client');
	await loadCore();
	return JSON.parse(productionWriteVerdict(JSON.stringify(ask))) as WriteVerdict;
};

let installed = false;

/**
 * Put the guard in front of `fetch` in a build that may be a test build —
 * the dev server or the extension (whose package says which it is). The
 * hosted wallet's build skips it at compile time.
 */
export function installProductionWriteGuard(): void {
	if (installed || typeof window === 'undefined') return;
	if (!import.meta.env.DEV && !__VELA_EXTENSION__) return;
	installed = true;
	const inner = window.fetch.bind(window);
	window.fetch = guardFetch(inner, thisBuild(inner), coreDecide);
}
