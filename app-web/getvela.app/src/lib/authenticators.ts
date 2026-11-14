/**
 * Authenticator recognition for the registry: turn the WebAuthn signals a
 * wallet stores on-chain into a human label.
 *
 * Every entry's `attestation` is 20 versioned bytes —
 *   version(1) ‖ AAGUID(16) ‖ authenticatorData flags(1) ‖ reserved(2)
 * — so the AAGUID (which identifies the authenticator model) and the flag bits
 * (UP/UV/BE/BS) are already on-chain. This module reads them out and resolves
 * the AAGUID to a provider name + icon.
 *
 * The AAGUID → name/icon lookup is served live by the AAGUID Explorer API
 * (https://aaguid-explorer.awesometools.dev) — Vela's own read-only, CORS-open
 * service, regenerated from the FIDO Metadata Service and the community
 * passkey-provider list. Resolving at runtime (rather than freezing a copy in
 * this bundle) keeps new authenticators recognized without a redeploy. Results
 * are cached per AAGUID for the page's lifetime, and every lookup degrades to a
 * generic label so the row still renders if the API is unreachable. Names are
 * cosmetic: `attestation` is the storer's unverified claim.
 */

const AAGUID_API = 'https://aaguid-explorer.awesometools.dev';

/** The registration-time WebAuthn signals decoded from a stored attestation. */
export interface AttestationSignals {
	/** Attestation format version (currently 1). */
	version: number;
	/** Canonical AAGUID uuid, or null when absent/all-zero (some authenticators
	 *  deliberately report a zero AAGUID for privacy). */
	aaguid: string | null;
	/** The raw flags byte, e.g. `0x5d`. */
	flagsHex: string;
	/** User Present (bit 0). */
	userPresent: boolean;
	/** User Verified (bit 2). */
	userVerified: boolean;
	/** Backup Eligible (bit 3) — the credential is syncable (a "synced passkey"). */
	backupEligible: boolean;
	/** Backup State (bit 4) — the credential is currently backed up / synced. */
	backedUp: boolean;
}

function hexToBytes(hex: string): Uint8Array {
	const clean = hex.startsWith('0x') ? hex.slice(2) : hex;
	const out = new Uint8Array(Math.floor(clean.length / 2));
	for (let i = 0; i < out.length; i++) {
		out[i] = parseInt(clean.slice(i * 2, i * 2 + 2), 16);
	}
	return out;
}

/**
 * Decode the 20-byte versioned attestation. Returns null for an empty or
 * malformed blob (a passkey that registered without stored signals, e.g. a
 * login-recovered key).
 */
export function parseAttestation(attestationHex: string | undefined): AttestationSignals | null {
	if (!attestationHex || attestationHex === '0x') return null;
	const bytes = hexToBytes(attestationHex);
	if (bytes.length !== 20) return null;
	const aaguidHex = [...bytes.slice(1, 17)].map((b) => b.toString(16).padStart(2, '0')).join('');
	const aaguid = /^0{32}$/.test(aaguidHex)
		? null
		: `${aaguidHex.slice(0, 8)}-${aaguidHex.slice(8, 12)}-${aaguidHex.slice(12, 16)}-${aaguidHex.slice(16, 20)}-${aaguidHex.slice(20)}`;
	const flags = bytes[17];
	return {
		version: bytes[0],
		aaguid,
		flagsHex: `0x${flags.toString(16).padStart(2, '0')}`,
		userPresent: (flags & 0x01) !== 0,
		userVerified: (flags & 0x04) !== 0,
		backupEligible: (flags & 0x08) !== 0,
		backedUp: (flags & 0x10) !== 0
	};
}

export interface SecurityChip {
	label: string;
	/** Coarse category, for colouring. */
	tone: 'verify' | 'sync' | 'bound';
	/** A one-line plain-language explanation. */
	title: string;
}

/**
 * Turn the raw authenticatorData flag bits into a few plain-language chips —
 * "User-verified", "Synced passkey", "Device-bound" — instead of `UP/UV/BE/BS`.
 */
export function securityChips(att: AttestationSignals): SecurityChip[] {
	const chips: SecurityChip[] = [];
	if (att.userVerified) {
		chips.push({
			label: 'User-verified',
			tone: 'verify',
			title: 'The device checked a biometric or PIN before signing (UV).'
		});
	}
	if (att.backupEligible && att.backedUp) {
		chips.push({
			label: 'Synced passkey',
			tone: 'sync',
			title: 'Backed up and syncable across the owner’s devices (BE + BS).'
		});
	} else if (att.backupEligible) {
		chips.push({
			label: 'Syncable',
			tone: 'sync',
			title: 'Eligible to sync, but not backed up yet (BE, no BS).'
		});
	} else {
		chips.push({
			label: 'Device-bound',
			tone: 'bound',
			title: 'Stays on this one device — cannot sync (no BE), e.g. a security key.'
		});
	}
	return chips;
}

export interface AuthenticatorLabel {
	/** Display name, e.g. "Apple Passwords", or a generic fallback. */
	name: string;
	/** Light/dark icon URLs on the AAGUID Explorer, or null when none / offline. */
	iconUrl: string | null;
	iconUrlDark: string | null;
	/** A vendor-family emoji, always set, used before/without an icon image. */
	glyph: string;
	/** true when the name came from a recognized AAGUID (vs a generic fallback). */
	known: boolean;
}

/** A vendor-family emoji for a resolved name — the pre-image placeholder. */
function glyphForName(name: string): string {
	const n = name.toLowerCase();
	if (n.includes('apple') || n.includes('icloud')) return '';
	if (n.includes('google')) return '🔵';
	if (n.includes('windows') || n.includes('microsoft')) return '🪟';
	if (n.includes('chrome') || n.includes('chromium') || n.includes('edge')) return '🌐';
	if (n.includes('samsung')) return '📱';
	if (n.includes('yubikey') || n.includes('security key') || n.includes('fido')) return '🔑';
	return '🔐';
}

/** A generic label from the browser hints when there is no AAGUID to resolve
 *  (or none is known yet) — synchronous, so a row renders before any fetch. */
export function fallbackLabel(
	attachment: string | undefined,
	transports: string | undefined
): AuthenticatorLabel {
	if (attachment === 'cross-platform' || (transports ?? '').includes('usb')) {
		return { name: 'Security key', iconUrl: null, iconUrlDark: null, glyph: '🔑', known: false };
	}
	if (attachment === 'platform') {
		return {
			name: 'Platform passkey',
			iconUrl: null,
			iconUrlDark: null,
			glyph: '🔑',
			known: false
		};
	}
	return { name: 'Passkey', iconUrl: null, iconUrlDark: null, glyph: '🔑', known: false };
}

// Per-AAGUID cache, shared across every row for the page's lifetime.
const cache = new Map<string, Promise<AuthenticatorLabel>>();

async function fetchLabel(
	aaguid: string,
	fallback: AuthenticatorLabel
): Promise<AuthenticatorLabel> {
	try {
		const res = await fetch(`${AAGUID_API}/api/v1/authenticators/${encodeURIComponent(aaguid)}`, {
			headers: { Accept: 'application/json' }
		});
		if (!res.ok) return fallback;
		const entry = (await res.json()) as {
			id?: string;
			name?: string;
			icon?: string;
			iconDark?: string;
		};
		// Never trust a response for a different AAGUID than we asked for.
		if (String(entry.id ?? '').toLowerCase() !== aaguid) return fallback;
		const name = entry.name?.trim() || fallback.name;
		return {
			name,
			iconUrl: entry.icon ? `${AAGUID_API}/data/${entry.icon}` : null,
			iconUrlDark: entry.iconDark ? `${AAGUID_API}/data/${entry.iconDark}` : null,
			glyph: glyphForName(name),
			known: true
		};
	} catch {
		return fallback;
	}
}

/**
 * Resolve an authenticator label. With an AAGUID, queries the AAGUID Explorer
 * (cached, best-effort); without one, returns a generic label immediately.
 */
export function resolveAuthenticator(
	aaguid: string | null,
	attachment: string | undefined,
	transports: string | undefined
): Promise<AuthenticatorLabel> {
	const fallback = fallbackLabel(attachment, transports);
	if (!aaguid) return Promise.resolve(fallback);
	const key = aaguid.toLowerCase();
	let pending = cache.get(key);
	if (!pending) {
		pending = fetchLabel(key, { ...fallback, name: 'Unrecognized authenticator', glyph: '🔐' });
		cache.set(key, pending);
	}
	return pending;
}
