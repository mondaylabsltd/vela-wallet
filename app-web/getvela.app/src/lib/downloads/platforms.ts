/**
 * Which file is whose (spec 065, Part A).
 *
 * Two pure functions and one table, shared by the Worker (`/download/<id>`,
 * `/api/downloads`) and the page. Nothing here fetches, so all of it is tested
 * without a browser or a network (FR-004).
 *
 * The table is the ONLY place that knows what a release file is called. The
 * names are matched by shape, never by version: the page must not be able to
 * name a version it did not read from the Release (A7).
 */

export const PLATFORM_IDS = [
	'macos-arm64',
	'macos-x64',
	'macos-universal',
	'windows-x64',
	'windows-arm64',
	'linux-deb-x64',
	'linux-deb-arm64',
	'linux-rpm-x64',
	'linux-rpm-arm64',
	'linux-flatpak-x64',
	'linux-flatpak-arm64',
	'extension'
] as const;

export type PlatformId = (typeof PLATFORM_IDS)[number];

export const isPlatformId = (value: string): value is PlatformId =>
	(PLATFORM_IDS as readonly string[]).includes(value);

/**
 * The shape of each platform's file name, as the packaging workflows write it
 * (checked against v0.9.3's fourteen assets, not recalled). Anchored at both
 * ends: `…-macos-arm64.dmg` must not also be somebody's `.dmg.sig` one day.
 *
 * No entry can match `.apk`, `.aab` or `.ipa`: phones are never offered (§0.3),
 * and the way to keep that true is for the table to have no row that could.
 */
const FILE_SHAPE: Record<PlatformId, RegExp> = {
	'macos-arm64': /^VelaWallet-[\d.]+-macos-arm64\.dmg$/,
	'macos-x64': /^VelaWallet-[\d.]+-macos-x86_64\.dmg$/,
	'macos-universal': /^VelaWallet-[\d.]+-macos-universal\.dmg$/,
	'windows-x64': /^VelaWallet-Setup-[\d.]+-x64\.exe$/,
	'windows-arm64': /^VelaWallet-Setup-[\d.]+-arm64\.exe$/,
	'linux-deb-x64': /^vela-wallet_[\d.]+_amd64\.deb$/,
	'linux-deb-arm64': /^vela-wallet_[\d.]+_arm64\.deb$/,
	'linux-rpm-x64': /^vela-wallet-[\d.]+-\d+\.[a-z0-9]+\.x86_64\.rpm$/,
	'linux-rpm-arm64': /^vela-wallet-[\d.]+-\d+\.[a-z0-9]+\.aarch64\.rpm$/,
	// Versioned from the release after 0.9.3; 0.9.3's own are not, and stay
	// downloadable for as long as it is the latest.
	'linux-flatpak-x64': /^app\.getvela\.VelaWallet-(?:[\d.]+-)?x86_64\.flatpak$/,
	'linux-flatpak-arm64': /^app\.getvela\.VelaWallet-(?:[\d.]+-)?aarch64\.flatpak$/,
	extension: /^vela-wallet-extension-[\d.]+\.zip$/
};

export interface ReleaseFile {
	name: string;
	size: number;
	/** GitHub's own download URL — the fallback when the mirror cannot serve. */
	url: string;
}

/**
 * Only what exists (A4): a platform with no file on the Release has no key
 * here, and the page says "coming shortly" for it rather than linking to a 404.
 */
export function matchFiles(
	files: readonly ReleaseFile[]
): Partial<Record<PlatformId, ReleaseFile>> {
	const out: Partial<Record<PlatformId, ReleaseFile>> = {};
	for (const id of PLATFORM_IDS) {
		const hit = files.find((f) => FILE_SHAPE[id].test(f.name));
		if (hit) out[id] = hit;
	}
	return out;
}

/** What the browser can tell us. Every field is optional because every one is. */
export interface ClientSignals {
	userAgent?: string;
	/** `navigator.userAgentData.platform` — Chromium only. */
	hintedPlatform?: string;
	/** `getHighEntropyValues(['architecture'])` — 'arm' | 'x86', Chromium only. */
	hintedArchitecture?: string;
	/** iPadOS says it is a Mac; a Mac has no touch points. */
	maxTouchPoints?: number;
}

/**
 * Which of the three columns is the visitor's — coarser than `detectPlatform`,
 * and answerable more often: a Linux browser that does not name its distribution
 * still gets its column highlighted, just no package guessed inside it.
 */
export function detectSystem(signals: ClientSignals): 'windows' | 'linux' | 'macos' | null {
	const ua = signals.userAgent ?? '';
	const hinted = (signals.hintedPlatform ?? '').toLowerCase();
	// The hint, when there is one, outranks the user-agent string: it is the
	// browser's structured answer, while the UA is a sentence kept for old sites.
	const family = hinted
		? hinted === 'macos'
			? 'macos'
			: hinted === 'windows'
				? 'windows'
				: hinted === 'linux' || hinted === 'chrome os'
					? 'linux'
					: null
		: /android|iphone|ipad|ipod/i.test(ua)
			? null
			: /Macintosh|Mac OS X/.test(ua)
				? 'macos'
				: /Windows NT/.test(ua)
					? 'windows'
					: /Linux|X11|CrOS/.test(ua)
						? 'linux'
						: null;
	// iPadOS says it is a Mac; a Mac has no touch points.
	if (family === 'macos' && (signals.maxTouchPoints ?? 0) > 1) return null;
	return family;
}

/**
 * The one button's platform, or `null` for "show the list, pick nothing".
 *
 * A convenience that is allowed to be wrong (A1) — so it guesses only where a
 * wrong guess still installs:
 *
 * - **macOS** without an architecture hint (Safari and Firefox both claim
 *   "Intel" on Apple silicon) gets the universal image, which runs on either.
 * - **Windows** without a hint gets x64, which Windows on ARM emulates.
 * - **Linux** gets a guess only when the browser names the distribution family
 *   (Firefox on Ubuntu and Fedora does). A .deb handed to a Fedora user is not
 *   "slightly wrong", it is useless — so otherwise: the list.
 * - **Phones and tablets** get nothing: their packages are not here (§0.3).
 */
export function detectPlatform(signals: ClientSignals): PlatformId | null {
	const ua = signals.userAgent ?? '';
	const arch = (signals.hintedArchitecture ?? '').toLowerCase();
	const family = detectSystem(signals);

	if (family === 'macos') {
		if (arch === 'arm') return 'macos-arm64';
		if (arch === 'x86') return 'macos-x64';
		return 'macos-universal';
	}
	if (family === 'windows') {
		return arch === 'arm' || /\bARM64\b/i.test(ua) ? 'windows-arm64' : 'windows-x64';
	}
	if (family === 'linux') {
		const arm = arch === 'arm' || /aarch64|arm64/i.test(ua);
		if (/Ubuntu|Debian|Mint/i.test(ua)) return arm ? 'linux-deb-arm64' : 'linux-deb-x64';
		if (/Fedora|Red Hat|SUSE/i.test(ua)) return arm ? 'linux-rpm-arm64' : 'linux-rpm-x64';
		return null;
	}
	return null;
}

/** The rows of "Other platforms", in the order a person scans them (A2). */
export const PLATFORM_GROUPS: ReadonlyArray<{
	group: 'macos' | 'windows' | 'linux';
	ids: readonly PlatformId[];
}> = [
	{ group: 'macos', ids: ['macos-arm64', 'macos-x64', 'macos-universal'] },
	{ group: 'windows', ids: ['windows-x64', 'windows-arm64'] },
	{
		group: 'linux',
		ids: [
			'linux-deb-x64',
			'linux-deb-arm64',
			'linux-rpm-x64',
			'linux-rpm-arm64',
			'linux-flatpak-x64',
			'linux-flatpak-arm64'
		]
	}
];

/** What `/api/downloads` answers and the page renders from. */
export interface DownloadsManifest {
	/** `0.9.3` — read off the Release's tag, nowhere else. */
	version: string;
	tag: string;
	files: Partial<Record<PlatformId, { name: string; size: number }>>;
}

/** `29212806` → `27.9 MB`. Binary megabytes, one decimal: what a file manager shows. */
export const megabytes = (bytes: number) => `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
