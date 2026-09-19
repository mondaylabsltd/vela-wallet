import { describe, expect, it } from 'vitest';
import {
	PLATFORM_IDS,
	detectPlatform,
	detectSystem,
	matchFiles,
	type ReleaseFile
} from './platforms';

/** v0.9.3's assets, copied from `gh release view v0.9.3` on 2026-09-19. */
const V093 = [
	'app.getvela.VelaWallet-aarch64.flatpak',
	'app.getvela.VelaWallet-x86_64.flatpak',
	'SHA256SUMS',
	'SHA256SUMS-macos',
	'vela-wallet-0.9.3-1.fc44.aarch64.rpm',
	'vela-wallet-0.9.3-1.fc44.x86_64.rpm',
	'vela-wallet-extension-0.9.3.zip',
	'vela-wallet_0.9.3_amd64.deb',
	'vela-wallet_0.9.3_arm64.deb',
	'VelaWallet-0.9.3-macos-arm64.dmg',
	'VelaWallet-0.9.3-macos-universal.dmg',
	'VelaWallet-0.9.3-macos-x86_64.dmg',
	'VelaWallet-Setup-0.9.3-arm64.exe',
	'VelaWallet-Setup-0.9.3-x64.exe'
];

const files = (names: string[]): ReleaseFile[] =>
	names.map((name) => ({ name, size: 1, url: `https://example.invalid/${name}` }));

describe('matchFiles', () => {
	it('finds every platform on a real release, each a different file', () => {
		const matched = matchFiles(files(V093));
		expect(Object.keys(matched).sort()).toEqual([...PLATFORM_IDS].sort());
		expect(new Set(Object.values(matched).map((f) => f!.name)).size).toBe(PLATFORM_IDS.length);
		expect(matched['macos-x64']!.name).toBe('VelaWallet-0.9.3-macos-x86_64.dmg');
		expect(matched['linux-rpm-arm64']!.name).toBe('vela-wallet-0.9.3-1.fc44.aarch64.rpm');
	});

	it('offers only what exists: a release still waiting for its macOS images has no macOS rows', () => {
		const matched = matchFiles(files(V093.filter((n) => !n.endsWith('.dmg'))));
		expect(matched['macos-arm64']).toBeUndefined();
		expect(matched['macos-universal']).toBeUndefined();
		expect(matched['windows-x64']).toBeDefined();
	});

	it('never offers a phone package, whatever it is called', () => {
		const matched = matchFiles(
			files(['vela-wallet-0.9.3.apk', 'vela-wallet-0.9.3.aab', 'VelaWallet-0.9.3.ipa'])
		);
		expect(matched).toEqual({});
	});

	it('does not take a signature or checksum beside a file for the file', () => {
		const matched = matchFiles(
			files(['VelaWallet-0.9.3-macos-arm64.dmg.sig', 'VelaWallet-Setup-0.9.3-x64.exe.sha256'])
		);
		expect(matched).toEqual({});
	});

	it('finds the Flatpak under its versioned name (after 0.9.3) as well as its old one', () => {
		const matched = matchFiles(
			files([
				'app.getvela.VelaWallet-26.9.0-x86_64.flatpak',
				'app.getvela.VelaWallet-26.9.0-aarch64.flatpak'
			])
		);
		expect(matched['linux-flatpak-x64']!.name).toBe('app.getvela.VelaWallet-26.9.0-x86_64.flatpak');
		expect(matched['linux-flatpak-arm64']!.name).toBe(
			'app.getvela.VelaWallet-26.9.0-aarch64.flatpak'
		);
	});

	it('follows the next Fedora without being told', () => {
		const matched = matchFiles(files(['vela-wallet-1.0.0-2.fc45.x86_64.rpm']));
		expect(matched['linux-rpm-x64']).toBeDefined();
	});
});

const MAC_UA =
	'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15';
const WIN_UA =
	'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36';

describe('detectPlatform — one case per row of A2', () => {
	it('macOS, Apple silicon: only a Chromium architecture hint can say so', () => {
		expect(
			detectPlatform({ userAgent: MAC_UA, hintedPlatform: 'macOS', hintedArchitecture: 'arm' })
		).toBe('macos-arm64');
	});

	it('macOS, Intel', () => {
		expect(
			detectPlatform({ userAgent: MAC_UA, hintedPlatform: 'macOS', hintedArchitecture: 'x86' })
		).toBe('macos-x64');
	});

	it('macOS, not sure: Safari says "Intel" on every Mac, so it gets the image that runs on both', () => {
		expect(detectPlatform({ userAgent: MAC_UA, maxTouchPoints: 0 })).toBe('macos-universal');
	});

	it('Windows, most PCs', () => {
		expect(detectPlatform({ userAgent: WIN_UA })).toBe('windows-x64');
	});

	it('Windows on ARM', () => {
		expect(
			detectPlatform({ userAgent: WIN_UA, hintedPlatform: 'Windows', hintedArchitecture: 'arm' })
		).toBe('windows-arm64');
	});

	it('Linux .deb, both architectures', () => {
		expect(
			detectPlatform({
				userAgent: 'Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:130.0) Gecko/20100101 Firefox/130.0'
			})
		).toBe('linux-deb-x64');
		expect(
			detectPlatform({
				userAgent: 'Mozilla/5.0 (X11; Ubuntu; Linux aarch64; rv:130.0) Gecko/20100101 Firefox/130.0'
			})
		).toBe('linux-deb-arm64');
	});

	it('Linux .rpm, both architectures', () => {
		expect(
			detectPlatform({
				userAgent: 'Mozilla/5.0 (X11; Fedora; Linux x86_64; rv:130.0) Gecko/20100101 Firefox/130.0'
			})
		).toBe('linux-rpm-x64');
		expect(
			detectPlatform({
				userAgent: 'Mozilla/5.0 (X11; Fedora; Linux aarch64; rv:130.0) Gecko/20100101 Firefox/130.0'
			})
		).toBe('linux-rpm-arm64');
	});

	it('Linux that does not name its family: the list, because a wrong package format is useless', () => {
		expect(
			detectPlatform({
				userAgent:
					'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36',
				hintedPlatform: 'Linux',
				hintedArchitecture: 'x86'
			})
		).toBeNull();
	});

	it('the structured hint outranks the user-agent sentence when they disagree', () => {
		expect(detectPlatform({ userAgent: MAC_UA, hintedPlatform: 'Windows' })).toBe('windows-x64');
		expect(detectPlatform({ userAgent: WIN_UA, hintedPlatform: 'Android' })).toBeNull();
	});

	it('…but that Linux still gets its COLUMN: the system is known even when the package is not', () => {
		const chromeOnLinux = {
			userAgent:
				'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36'
		};
		expect(detectPlatform(chromeOnLinux)).toBeNull();
		expect(detectSystem(chromeOnLinux)).toBe('linux');
		expect(detectSystem({ userAgent: MAC_UA, maxTouchPoints: 5 })).toBeNull();
	});

	it('never guesses Flatpak or the extension: those are chosen, not detected', () => {
		const guessable = new Set([MAC_UA, WIN_UA].map((userAgent) => detectPlatform({ userAgent })));
		expect(guessable.has('extension')).toBe(false);
		expect([...guessable].some((id) => id?.includes('flatpak'))).toBe(false);
	});
});

describe('detectPlatform — what gets nothing', () => {
	it('unknown → show the list, pick nothing', () => {
		expect(detectPlatform({})).toBeNull();
		expect(detectPlatform({ userAgent: 'curl/8.7.1' })).toBeNull();
	});

	it('phones: their packages are not here', () => {
		expect(
			detectPlatform({
				userAgent:
					'Mozilla/5.0 (Linux; Android 15; Pixel 9) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Mobile Safari/537.36',
				hintedPlatform: 'Android'
			})
		).toBeNull();
		expect(
			detectPlatform({
				userAgent:
					'Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1'
			})
		).toBeNull();
	});

	it('an iPad calling itself a Mac is caught by its touch points', () => {
		expect(detectPlatform({ userAgent: MAC_UA, maxTouchPoints: 5 })).toBeNull();
	});
});
