/**
 * Three dApp records as the signing path now writes them (spec 093) — a swap
 * on Uniswap, a Permit2 permit and a Sign-In with Ethereum — for the tests
 * that feed them through the REAL `activity_feed` core and for the e2e that
 * draws them. Each carries what `buildSigningRecord` stores: the core's
 * summary verbatim (`dappSummary`), its cut of the request (`signedRequest`),
 * the sheet's balance changes (`balanceChanges`) and no signature anywhere.
 *
 * Fixtures only: nothing on a live page imports this module.
 */
import type { FeedDapp } from '$lib/core/generated/FeedDapp';
import type { LocalTransaction } from '$lib/services/transactions-model';

export const UNISWAP_ROUTER = '0x3fc91a3afd70395cd496c647d5a6cc9d4b2b7fad';
export const PERMIT2 = '0x000000000022d473030f116ddee9f6b43ac78ba3';
export const USDC_MAINNET = '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48';
export const UNISWAP_ORIGIN = 'https://app.uniswap.org';

const MAX_UINT160 = '1461501637330902918203684832716283019655932542975';

/** UTF-8 text as the `0x…` hex a page sends to `personal_sign`. */
function hexOf(text: string): string {
	return (
		'0x' +
		Array.from(new TextEncoder().encode(text), (b) => b.toString(16).padStart(2, '0')).join('')
	);
}

/**
 * The swap, the permit and the sign-in, newest first, signed by `account` on
 * Ethereum — one minute apart, ending `nowSec`.
 */
export function dappActivityRecords(account: string, nowSec: number): LocalTransaction[] {
	const swapAt = nowSec - 120;
	const permitAt = nowSec - 60;
	const signInAt = nowSec;
	const typedData = {
		types: {
			EIP712Domain: [
				{ name: 'name', type: 'string' },
				{ name: 'chainId', type: 'uint256' },
				{ name: 'verifyingContract', type: 'address' }
			],
			PermitSingle: [
				{ name: 'details', type: 'PermitDetails' },
				{ name: 'spender', type: 'address' },
				{ name: 'sigDeadline', type: 'uint256' }
			],
			PermitDetails: [
				{ name: 'token', type: 'address' },
				{ name: 'amount', type: 'uint160' },
				{ name: 'expiration', type: 'uint48' },
				{ name: 'nonce', type: 'uint48' }
			]
		},
		primaryType: 'PermitSingle',
		domain: { name: 'Permit2', chainId: 1, verifyingContract: PERMIT2 },
		message: {
			details: { token: USDC_MAINNET, amount: MAX_UINT160, expiration: '0', nonce: '0' },
			spender: UNISWAP_ROUTER,
			sigDeadline: String(permitAt + 1800)
		}
	};
	const siwe = [
		'app.uniswap.org wants you to sign in with your Ethereum account:',
		account,
		'',
		'Sign in to Uniswap',
		'',
		'URI: https://app.uniswap.org',
		'Version: 1',
		'Chain ID: 1',
		'Nonce: 9fK2xQ7a',
		'Issued At: 2026-10-02T10:00:00.000Z'
	].join('\n');

	const swap: LocalTransaction = {
		id: `dapp-${swapAt * 1000}-tx`,
		userOpHash: '0x' + 'e1'.repeat(32),
		txHash: '0x' + 'f1'.repeat(32),
		from: account,
		to: UNISWAP_ROUTER,
		value: '0x0',
		symbol: 'ETH',
		decimals: 18,
		chainId: 1,
		timestamp: swapAt,
		status: 'confirmed',
		type: 'dapp_tx',
		dappOrigin: 'Uniswap Interface',
		dappUrl: UNISWAP_ORIGIN,
		intent: 'Swap',
		signedRequest: {
			method: 'eth_sendTransaction',
			params: [{ to: UNISWAP_ROUTER, value: '0x0', data: '0x3593564c' + '00'.repeat(96) }]
		},
		requestTruncated: false,
		dappSummary: { action: 'call', calls: 1, contract: UNISWAP_ROUTER },
		balanceChanges: [
			{
				type: 'erc20_trusted',
				token: USDC_MAINNET,
				delta: '-100000000',
				symbol: 'USDC',
				decimals: 6,
				in_trusted_set: true
			},
			{ type: 'native', delta: '30000000000000000' }
		]
	};
	const permit: LocalTransaction = {
		id: `dapp-${permitAt * 1000}-typed`,
		userOpHash: '',
		txHash: '',
		from: account,
		to: '',
		value: '0',
		symbol: '',
		decimals: 0,
		chainId: 1,
		timestamp: permitAt,
		status: 'confirmed',
		type: 'sign_typed_data',
		dappOrigin: 'Uniswap Interface',
		dappUrl: UNISWAP_ORIGIN,
		signedRequest: {
			method: 'eth_signTypedData_v4',
			params: [account, JSON.stringify(typedData)]
		},
		requestTruncated: false,
		dappSummary: {
			action: 'permit',
			calls: 0,
			contract: PERMIT2,
			spender: UNISWAP_ROUTER,
			token: USDC_MAINNET,
			symbol: 'USDC',
			decimals: 6,
			unlimited: true,
			primary_type: 'PermitSingle'
		}
	};
	const signIn: LocalTransaction = {
		id: `dapp-${signInAt * 1000}-msg`,
		userOpHash: '',
		txHash: '',
		from: account,
		to: '',
		value: '0',
		symbol: '',
		decimals: 0,
		chainId: 1,
		timestamp: signInAt,
		status: 'confirmed',
		type: 'sign_message',
		dappOrigin: 'Uniswap Interface',
		dappUrl: UNISWAP_ORIGIN,
		signedRequest: { method: 'personal_sign', params: [hexOf(siwe), account] },
		requestTruncated: false,
		dappSummary: { action: 'sign_in', calls: 0, signin_domain: 'app.uniswap.org' }
	};
	return [signIn, permit, swap];
}

/** A complete `FeedDapp` — every field the core sends for a dApp row — over `partial`. */
export function feedDapp(partial: Partial<FeedDapp> = {}): FeedDapp {
	return {
		site: null,
		intent: null,
		intent_term: null,
		changes: [],
		received: null,
		estimated: false,
		contract_call: false,
		action: 'call',
		place: null,
		allowance: null,
		off_chain: false,
		facts: [],
		technical: [],
		...partial
	};
}
