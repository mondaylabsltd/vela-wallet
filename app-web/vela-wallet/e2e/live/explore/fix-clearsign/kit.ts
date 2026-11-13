/**
 * Signer-area exploration kit: the trusted signing page served locally, driven
 * the way the apps open it (sign.html?ch=url#i=…&cb=…&t=…), with a CDP virtual
 * authenticator holding a key we know, the velawallet:// navigation caught by
 * DevTools, and every expectation computed independently (foundry's `cast`,
 * vela-core's wasm, node:crypto) — never by the page's own JS.
 */
import { execFileSync } from 'node:child_process';
import { createHash, createPublicKey, generateKeyPairSync, randomBytes, verify } from 'node:crypto';
import { deflateRawSync } from 'node:zlib';
import { readFileSync, readdirSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';
import type { CDPSession, Page } from '@playwright/test';

export const D =
	'/private/tmp/claude-501/-Volumes-data-production2-vela-wallet/046bc8f0-055e-4828-9ec5-8b8fd835f088/scratchpad/explore/clearsign';
export const SHOTS = `${D}/shots`;
mkdirSync(SHOTS, { recursive: true });

export const PORT = 8653;
export const LAUNCH = 'f14e755aace5937d245c3debe4318fd7b6640d5c17032c472b976b72509f46a8';
export const SIGNER = `http://localhost:${PORT}/dist/b/${LAUNCH}/sign.html`;
export const WALLET_CB = 'velawallet://sign-result';

export const SAFE = '0x88cCA0f8B4E1F0dC0e7C4f9a2B3d5E6f7A8b6894';
export const RECIP = '0xaF5e8917831Ef08A64e18b2Cde9f8f5d32c7b3e1';
export const SPENDER = '0x000000000022D473030F116dDEE9F6B43aC78BA3'; // Permit2
export const ROUTER = '0x1111111254EEB25477B68fb85Ed929f73A960582';
export const FEE_TO = '0x5E11aE4d4C6b1C2fD1A7a8c4f6E1a9C2bB3e4D10';
export const MULTI_SEND = '0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526';
export const MODULE = '0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226';
export const ENTRY_POINT = '0x0000000071727De22E5E9d8BAf0edAc6f37da032';
export const USDC_ETH = '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48';
export const USDC_BASE = '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913';
export const USDC_GNO = '0x2a22f9c3b484c3629090FeED35F17Ff8F88f76F0'; // USDC.e on Gnosis

export function cast(...args: string[]): string {
	return execFileSync('cast', args, { encoding: 'utf8' }).trim();
}
export const calldata = (sig: string, ...args: string[]) => cast('calldata', sig, ...args);

/** MultiSend's packed transactions: op(1) ‖ to(20) ‖ value(32) ‖ len(32) ‖ data. */
export function packMultiSend(legs: { op?: number; to: string; value?: bigint; data?: string }[]): string {
	let out = '';
	for (const leg of legs) {
		const data = (leg.data ?? '0x').replace(/^0x/, '');
		out += (leg.op ?? 0).toString(16).padStart(2, '0');
		out += leg.to.replace(/^0x/, '').toLowerCase().padStart(40, '0');
		out += (leg.value ?? 0n).toString(16).padStart(64, '0');
		out += (data.length / 2).toString(16).padStart(64, '0');
		out += data;
	}
	return '0x' + out;
}

/** executeUserOp(to, value, data, op) — the Safe 4337 module's entry. */
export function executeUserOp(to: string, value: bigint, data: string, op: number): string {
	return calldata('executeUserOp(address,uint256,bytes,uint8)', to, value.toString(), data, String(op));
}

/** Calls + a fee leg, assembled as the wallet does (MultiSend delegatecall when >1 leg). */
export function assemble(legs: { op?: number; to: string; value?: bigint; data?: string }[]): string {
	if (legs.length === 1) {
		const l = legs[0];
		return executeUserOp(l.to, l.value ?? 0n, l.data ?? '0x', l.op ?? 0);
	}
	const packed = packMultiSend(legs);
	return executeUserOp(MULTI_SEND, 0n, calldata('multiSend(bytes)', packed), 1);
}

export interface UserOp {
	sender: string;
	nonce: string;
	initCode: string;
	callData: string;
	verificationGasLimit: string;
	callGasLimit: string;
	preVerificationGas: string;
	maxFeePerGas: string;
	maxPriorityFeePerGas: string;
	paymasterAndData: string;
}

export function userOp(callData: string, sender = SAFE): UserOp {
	return {
		sender,
		nonce: '0x7',
		initCode: '0x',
		callData,
		verificationGasLimit: '300000',
		callGasLimit: '200000',
		preVerificationGas: '110000',
		maxFeePerGas: '1500000007',
		maxPriorityFeePerGas: '1500000000',
		paymasterAndData: '0x'
	};
}

// --- vela-core, compiled: the reference for every digest -------------------
type Core = {
	initSync(o: { module: Buffer }): void;
	attestSafeOpHash(op: string, calls: string, chain: bigint): Uint8Array;
	attestSafeMessageHash(h: Uint8Array, chain: bigint, safe: string): Uint8Array;
	hashTypedData(json: string): Uint8Array;
};
let core: Core | undefined;
export async function loadCore(): Promise<Core> {
	if (core) return core;
	const root = '/Volumes/data/production2/vela-wallet';
	core = (await import(join(root, 'rust/pkg-web/vela_core.js'))) as Core;
	const dir = join(root, 'assets/wasm');
	const file = readdirSync(dir).find((n) => /^vela_core_bg\..*\.wasm$/.test(n)) as string;
	core.initSync({ module: readFileSync(join(dir, file)) });
	return core;
}

export async function safeOpHash(op: UserOp, chainId: number): Promise<string> {
	const c = await loadCore();
	const bytes = c.attestSafeOpHash(
		JSON.stringify({
			sender: op.sender,
			nonce: op.nonce,
			init_code_hex: op.initCode,
			call_data_hex: op.callData,
			verification_gas_limit: op.verificationGasLimit,
			call_gas_limit: op.callGasLimit,
			pre_verification_gas: op.preVerificationGas,
			max_fee_per_gas: op.maxFeePerGas,
			max_priority_fee_per_gas: op.maxPriorityFeePerGas,
			paymaster_and_data_hex: op.paymasterAndData
		}),
		'',
		BigInt(chainId)
	);
	return '0x' + Buffer.from(bytes).toString('hex');
}

export async function safeMessageHash(inner: string, chainId: number, safe = SAFE): Promise<string> {
	const c = await loadCore();
	const bytes = c.attestSafeMessageHash(Buffer.from(inner.replace(/^0x/, ''), 'hex'), BigInt(chainId), safe);
	return '0x' + Buffer.from(bytes).toString('hex');
}

/** EIP-191 via cast (foundry), independent of the page's keccak. */
export const hashMessage = (msg: string) => cast('hash-message', msg);

export async function typedHash(data: unknown): Promise<string> {
	const c = await loadCore();
	return '0x' + Buffer.from(c.hashTypedData(JSON.stringify(data))).toString('hex');
}

// --- request URLs ----------------------------------------------------------
const b64url = (b: Buffer | string) => Buffer.from(b).toString('base64url');

export interface UrlOpts {
	cb?: string | null;
	t?: string;
	z?: boolean;
	lang?: string;
	base?: string;
	rawI?: string;
}
export function signerUrl(payload: unknown, o: UrlOpts = {}): string {
	const json = JSON.stringify(payload);
	const i = o.rawI ?? (o.z ? b64url(deflateRawSync(Buffer.from(json))) : b64url(json));
	const q = `?ch=url${o.lang ? `&lang=${o.lang}` : ''}`;
	let frag = `#i=${i}`;
	if (o.cb !== null) frag += `&cb=${b64url(o.cb ?? WALLET_CB)}`;
	frag += `&t=${o.t ?? 'tok-' + randomBytes(4).toString('hex')}`;
	if (o.z) frag += '&z=1';
	return (o.base ?? SIGNER) + q + frag;
}

// --- the virtual authenticator, with a key we hold --------------------------
export interface Key {
	cdp: CDPSession;
	authenticatorId: string;
	credentialId: string; // base64url
	credentialHex: string;
	publicKeyPem: string;
	navs: string[];
}

export async function withKey(
	page: Page,
	opts: { rpId?: string; uv?: boolean; presence?: boolean } = {}
): Promise<Key> {
	const cdp = await page.context().newCDPSession(page);
	await cdp.send('WebAuthn.enable');
	const { authenticatorId } = await cdp.send('WebAuthn.addVirtualAuthenticator', {
		options: {
			protocol: 'ctap2',
			transport: 'internal',
			hasResidentKey: true,
			hasUserVerification: true,
			isUserVerified: opts.uv ?? true,
			automaticPresenceSimulation: opts.presence ?? true
		}
	});
	const { privateKey, publicKey } = generateKeyPairSync('ec', { namedCurve: 'P-256' });
	const raw = randomBytes(16);
	await cdp.send('WebAuthn.addCredential', {
		authenticatorId,
		credential: {
			credentialId: raw.toString('base64'),
			isResidentCredential: true,
			rpId: opts.rpId ?? 'localhost',
			privateKey: privateKey.export({ format: 'der', type: 'pkcs8' }).toString('base64'),
			userHandle: Buffer.from('vela-test').toString('base64'),
			signCount: 0
		}
	});
	const navs: string[] = [];
	await cdp.send('Page.enable');
	cdp.on('Page.frameRequestedNavigation', (e: { url: string }) => navs.push(e.url));
	return {
		cdp,
		authenticatorId,
		credentialId: raw.toString('base64url'),
		credentialHex: raw.toString('hex'),
		publicKeyPem: publicKey.export({ format: 'pem', type: 'spki' }).toString(),
		navs
	};
}

export async function waitAnswer(key: Key, ms = 10_000): Promise<URL | null> {
	const end = Date.now() + ms;
	while (Date.now() < end) {
		const hit = key.navs.find((u) => u.startsWith('velawallet:'));
		if (hit) return new URL(hit);
		await new Promise((r) => setTimeout(r, 150));
	}
	return null;
}

export interface Checked {
	digest: string;
	digestKind: string;
	challengeIsDigest: boolean;
	signatureValid: boolean;
	uv: boolean;
	rpIdHashOk: boolean;
	origin: string;
	credentialId: string;
	keys: string[];
}

/** Read the answer and verify it with node:crypto alone. */
export function checkAnswer(url: URL, key: Key, expectedDigest: string, rpId = 'localhost'): Checked {
	const result = JSON.parse(Buffer.from(url.searchParams.get('result') ?? '', 'base64url').toString());
	const hx = (v: string) => Buffer.from(String(v).replace(/^0x/, ''), 'hex');
	const authData = hx(result.authenticatorData);
	const cdj = hx(result.clientDataJSON);
	const client = JSON.parse(cdj.toString());
	const sig = hx(result.signature);
	const r = sig.subarray(0, 32);
	const s = sig.subarray(32, 64);
	const der = (n: Buffer) => {
		let b = n;
		while (b.length > 1 && b[0] === 0 && !(b[1] & 0x80)) b = b.subarray(1);
		if (b[0] & 0x80) b = Buffer.concat([Buffer.from([0]), b]);
		return Buffer.concat([Buffer.from([0x02, b.length]), b]);
	};
	const body = Buffer.concat([der(r), der(s)]);
	const derSig = Buffer.concat([Buffer.from([0x30, body.length]), body]);
	const signed = Buffer.concat([authData, createHash('sha256').update(cdj).digest()]);
	const ok = verify('sha256', signed, createPublicKey(key.publicKeyPem), derSig);
	const want = Buffer.from(expectedDigest.replace(/^0x/, ''), 'hex').toString('base64url');
	return {
		digest: result.digest,
		digestKind: result.digestKind,
		challengeIsDigest: client.challenge === want,
		signatureValid: ok,
		uv: (authData[32] & 0x04) !== 0,
		rpIdHashOk: authData.subarray(0, 32).equals(createHash('sha256').update(rpId).digest()),
		origin: client.origin,
		credentialId: result.credentialId,
		keys: Object.keys(result)
	};
}

/** Everything the page drew, as text, plus the state the page exposes. */
export async function sheetState(page: Page) {
	return page.evaluate(() => {
		const w = window as unknown as {
			__velaState?: { phase: string };
			__slider?: HTMLButtonElement | null;
		};
		const confirm = document.querySelector('.confirm') as HTMLButtonElement | null;
		return {
			phase: w.__velaState?.phase ?? null,
			text: (document.getElementById('sheet-slot') as HTMLElement).innerText,
			status: (document.getElementById('status') as HTMLElement).innerText,
			trust: (document.getElementById('trust') as HTMLElement | null)?.innerText ?? '',
			confirm: confirm ? { text: confirm.innerText, disabled: confirm.disabled } : null,
			warnings: [...document.querySelectorAll('.warning')].map((n) => (n as HTMLElement).innerText),
			hash: location.hash,
			lang: document.documentElement.lang,
			overflowX: document.documentElement.scrollWidth > window.innerWidth + 1
		};
	});
}

export const ctx = (chainId: number, extra: Record<string, unknown> = {}) => ({
	chainId,
	account: SAFE,
	signer: { name: 'Daily', letter: 'D' },
	...extra
});
