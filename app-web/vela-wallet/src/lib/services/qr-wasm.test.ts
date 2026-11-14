/**
 * The JS half of vela-qr's ABI: the result format `rust/vela-qr/wasm` writes,
 * and the text the platform's decoders make of segments the crate leaves to
 * them. Plus the shipped module itself, read from disk: it loads, has the ABI,
 * and finds nothing where there is nothing.
 */
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { instantiateQr, parseQrResult, readQr } from './qr-wasm';

/** Build a result the way the crate does. */
function result(opts: {
	part?: [number, number];
	segments: { candidates: ({ text: string } | { label: string })[]; raw: number[] }[];
	format?: number;
}): Uint8Array {
	const out: number[] = [opts.format ?? 1];
	const u32 = (v: number) =>
		out.push(v & 0xff, (v >> 8) & 0xff, (v >> 16) & 0xff, (v >>> 24) & 0xff);
	const utf8 = (s: string) => [...new TextEncoder().encode(s)];
	if (opts.part) out.push(1, ...opts.part);
	else out.push(0);
	u32(opts.segments.length);
	for (const seg of opts.segments) {
		out.push(seg.candidates.length);
		for (const c of seg.candidates) {
			if ('text' in c) {
				const b = utf8(c.text);
				out.push(0);
				u32(b.length);
				out.push(...b);
			} else {
				const b = utf8(c.label);
				out.push(1, b.length, ...b);
			}
		}
		u32(seg.raw.length);
		out.push(...seg.raw);
	}
	return new Uint8Array(out);
}

const ascii = (s: string) => [...new TextEncoder().encode(s)];

describe('the result format', () => {
	it('takes text the crate decoded as it is', () => {
		const r = result({
			segments: [{ candidates: [{ text: 'ethereum:0xabc' }], raw: ascii('ethereum:0xabc') }]
		});
		expect(parseQrResult(r)).toEqual({ kind: 'text', text: 'ethereum:0xabc' });
	});

	it('decodes a GBK segment with the platform’s table', () => {
		// "你好" in GBK, undeclared: the crate ranks gb18030 first, Latin-1 last.
		const gbk = [0xc4, 0xe3, 0xba, 0xc3];
		const r = result({
			segments: [{ candidates: [{ label: 'gb18030' }, { text: 'Äãºà' }], raw: gbk }]
		});
		expect(parseQrResult(r)).toEqual({ kind: 'text', text: '你好' });
	});

	it('falls through a charset the bytes are not valid in', () => {
		// 0x80 alone is not Shift_JIS: the next candidate stands.
		const r = result({
			segments: [{ candidates: [{ label: 'shift_jis' }, { text: '\u0080' }], raw: [0x80] }]
		});
		expect(parseQrResult(r)).toEqual({ kind: 'text', text: '\u0080' });
	});

	it('joins segments in order', () => {
		const r = result({
			segments: [
				{ candidates: [{ text: 'Total: ' }], raw: ascii('Total: ') },
				{ candidates: [{ label: 'shift_jis' }], raw: [0x93, 0xfa, 0x96, 0x7b] }
			]
		});
		expect(parseQrResult(r)).toEqual({ kind: 'text', text: 'Total: 日本' });
	});

	it('marks one part of several, never as the message', () => {
		const r = result({
			part: [0, 3],
			segments: [{ candidates: [{ text: 'ethereum:0x' }], raw: ascii('ethereum:0x') }]
		});
		expect(parseQrResult(r)).toEqual({ kind: 'part', index: 0, count: 3, text: 'ethereum:0x' });
	});

	it('says unreadable for data that is not text', () => {
		expect(parseQrResult(result({ segments: [{ candidates: [], raw: [0, 1, 2] }] }))).toEqual({
			kind: 'unreadable'
		});
		expect(parseQrResult(result({ format: 2, segments: [] }))).toEqual({ kind: 'unreadable' });
	});
});

describe('the shipped module', () => {
	it('loads, and finds nothing in a blank image or a wrong-sized one', async () => {
		const decoder = await instantiateQr(readFileSync('../../rust/pkg-web-qr/vela_qr.wasm'));
		const blank = new Uint8Array(200 * 200).fill(255);
		expect(readQr(decoder, blank, 200, 200, 'luma')).toBeNull();
		expect(readQr(decoder, blank, 201, 200, 'luma')).toBeNull();
		expect(
			readQr(decoder, new Uint8ClampedArray(200 * 200 * 4).fill(255), 200, 200, 'rgba')
		).toBeNull();
	});
});
