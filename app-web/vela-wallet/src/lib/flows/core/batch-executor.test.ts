/**
 * 087 (issue 333's twin): a picked table reaches the `batch_import` core as
 * its BYTES, undecoded — the core decodes text once, as it does a contacts
 * file, and refuses a legacy code page instead of garbling it; a workbook it
 * reads itself (`vela_core::xlsx`). `File.text()` used to decode here and
 * turned a GBK CSV's Chinese names into U+FFFD.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

const picked = vi.hoisted(() => ({ value: null as unknown }));
vi.mock('$lib/services/file-io', () => ({
	pickTable: vi.fn(async () => picked.value),
	saveTextFile: vi.fn(async () => undefined)
}));

import { executeBatchOperation } from './batch-executor';

beforeEach(() => {
	picked.value = null;
});

describe('picking a table', () => {
	it('hands a text table over as its bytes, never decoded here', async () => {
		const gbk = new Uint8Array([0xd5, 0xc5, 0xc8, 0xfd, 0x2c, 0x31]);
		picked.value = { name: 'payroll.csv', bytes: gbk };
		const result = await executeBatchOperation({ id: 1, operation: { type: 'pick_file' } });
		expect(result).toEqual({
			type: 'file_picked',
			name: 'payroll.csv',
			content: { type: 'bytes', bytes: [0xd5, 0xc5, 0xc8, 0xfd, 0x2c, 0x31] }
		});
	});

	it('hands a workbook over as its bytes too, and a cancel stays a cancel', async () => {
		const zip = new Uint8Array([0x50, 0x4b, 0x03, 0x04, 0x14, 0x00]);
		picked.value = { name: 'payroll.xlsx', bytes: zip };
		const workbook = await executeBatchOperation({ id: 2, operation: { type: 'pick_file' } });
		expect(workbook).toEqual({
			type: 'file_picked',
			name: 'payroll.xlsx',
			content: { type: 'bytes', bytes: [0x50, 0x4b, 0x03, 0x04, 0x14, 0x00] }
		});
		picked.value = null;
		expect(await executeBatchOperation({ id: 3, operation: { type: 'pick_file' } })).toEqual({
			type: 'file_pick_cancelled'
		});
	});
});
