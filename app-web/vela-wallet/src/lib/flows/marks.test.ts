/**
 * The core's mark vectors (`rust/crates/vela-core/tests/vectors/marks.json`),
 * replayed through THIS shell's Marks module: the endpoint read from storage
 * exactly as the person stored it, the core asked, its answer drawn. The core's
 * conformance test, the wasm verifier and the Kotlin and Swift harnesses replay
 * the same file, so a mark that differs between two platforms fails somewhere.
 */
import { readFileSync } from 'node:fs';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { initSync } from '../../../../../rust/pkg-web/vela_core.js';
import { WASM_URL } from '../../../../../rust/pkg-web/vela_core_wasm_url.js';
import type { MarkView } from '$lib/core/generated/MarkView';
import { nativeSymbol } from '$lib/services/networks';
import { chainColor } from '$lib/wallet/fixtures';
import { tokenGlyph } from '$lib/wallet/token-glyph';
import {
	chainLogoURL,
	chainMark,
	chainMarkView,
	tokenLogoURLs,
	tokenMarkFor,
	tokenMarkView
} from './marks';
import type { TokenMarkModel } from './model';

type Case =
	| {
			name: string;
			fn: 'token_mark';
			input: {
				ethereum_data_url: string;
				chain_id: number;
				symbol: string;
				token_address: string | null;
				named: string[];
			};
			expect: MarkView;
	  }
	| {
			name: string;
			fn: 'chain_mark';
			input: { ethereum_data_url: string; chain_id: number; native_symbol: string };
			expect: MarkView;
	  }
	| {
			name: string;
			fn: 'chain_logo_url';
			input: { ethereum_data_url: string; chain_id: number };
			expect: { value: string | null };
	  };

const VECTORS = JSON.parse(
	readFileSync('../../rust/crates/vela-core/tests/vectors/marks.json', 'utf8')
) as { suite: string; cases: Case[] };

beforeAll(() => {
	initSync({ module: readFileSync(`../../assets/wasm${WASM_URL}`) });
});

afterEach(() => vi.unstubAllGlobals());

/** The person's stored endpoint record, the way Settings writes it. */
function storeEndpoint(ethereumDataURL: string): void {
	const map = new Map([['vela.serviceEndpoints', JSON.stringify({ ethereumDataURL })]]);
	vi.stubGlobal('localStorage', {
		get length() {
			return map.size;
		},
		key: (i: number) => [...map.keys()][i] ?? null,
		getItem: (k: string) => map.get(k) ?? null,
		setItem: (k: string, v: string) => void map.set(k, v),
		removeItem: (k: string) => void map.delete(k),
		clear: () => map.clear()
	});
}

/** What the circle is told to draw for the core's answer. */
function drawn(view: MarkView, ticker: string, chainId: number): TokenMarkModel {
	return {
		ticker,
		badgeColor: chainColor(view.badge_chain_id ?? chainId),
		logoUrls: view.logo_urls.length > 0 ? view.logo_urls : undefined,
		badgeLogoUrl: view.badge_logo_url ?? undefined,
		badgeHidden: view.badge_chain_id === null
	};
}

describe('marks.json, through the web shell', () => {
	it('is the core\'s "marks" suite, and every case has an arm here', () => {
		expect(VECTORS.suite).toBe('marks');
		expect(VECTORS.cases.length).toBeGreaterThanOrEqual(36);
		const arms = new Set(['token_mark', 'chain_mark', 'chain_logo_url']);
		expect(VECTORS.cases.filter((c) => !arms.has(c.fn)).map((c) => c.name)).toEqual([]);
	});

	for (const c of VECTORS.cases) {
		it(c.name, () => {
			storeEndpoint(c.input.ethereum_data_url);
			if (c.fn === 'token_mark') {
				const { chain_id, symbol, token_address, named } = c.input;
				expect(tokenMarkView(chain_id, symbol, token_address, named)).toEqual(c.expect);
				expect(tokenMarkFor(chain_id, symbol, token_address, named)).toEqual(
					drawn(c.expect, symbol, chain_id)
				);
				expect(tokenLogoURLs(chain_id, symbol, token_address, named)).toEqual(c.expect.logo_urls);
				// The circle draws its own letters (fixtures ask the core nothing);
				// they are the core's.
				expect(tokenGlyph(symbol)).toBe(c.expect.glyph);
			} else if (c.fn === 'chain_mark') {
				const { chain_id, native_symbol } = c.input;
				expect(chainMarkView(chain_id, native_symbol)).toEqual(c.expect);
				// The screens name the coin from the network list (opBNB is not
				// in it, so its letters there are the list's default); the logo
				// and the absent badge are the network's either way.
				expect(chainMark(chain_id)).toEqual(drawn(c.expect, nativeSymbol(chain_id), chain_id));
				expect(tokenGlyph(native_symbol)).toBe(c.expect.glyph);
			} else {
				expect(chainLogoURL(c.input.chain_id)).toBe(c.expect.value ?? undefined);
			}
		});
	}
});

describe('the endpoint the marks read', () => {
	it('with nothing stored, is the built-in host', () => {
		// No storage at all — a fresh profile, or a context with none.
		expect(chainLogoURL(56)).toBe('https://ethereum-data.getvela.app/chainlogos/eip155-56.png');
	});

	it('follows a re-pointed endpoint at call time, not at import', () => {
		storeEndpoint('https://mirror.example');
		expect(chainLogoURL(1)).toBe('https://mirror.example/chainlogos/eip155-1.png');
		storeEndpoint('https://other.example/');
		expect(tokenMarkFor(8453, 'ETH', null).badgeLogoUrl).toBe(
			'https://other.example/chainlogos/eip155-8453.png'
		);
	});
});

describe('the kind rule', () => {
	it('a network wears its own logo, never its coin’s: Base is Base, ETH on Base is Ethereum', () => {
		const network = chainMark(8453);
		expect(network.logoUrls).toEqual([
			'https://ethereum-data.getvela.app/chainlogos/eip155-8453.png'
		]);
		expect(network.badgeHidden).toBe(true);
		const coin = tokenMarkFor(8453, 'ETH', null);
		expect(coin.logoUrls).toEqual(['https://ethereum-data.getvela.app/chainlogos/eip155-1.png']);
		expect(coin.badgeHidden).toBe(false);
	});

	it('chain 0 asks for nothing', () => {
		expect(chainLogoURL(0)).toBeUndefined();
		expect(chainMark(0).logoUrls).toBeUndefined();
	});
});
