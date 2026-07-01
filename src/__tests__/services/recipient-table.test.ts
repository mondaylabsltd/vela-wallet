import { parseRecipientTableText, parseRecipientTable } from '@/services/recipient-table';

const A = '0x' + 'aa'.repeat(20);
const B = '0x' + 'bb'.repeat(20);
const C = '0x' + 'cc'.repeat(20);

describe('parseRecipientTableText — delimiters', () => {
  test('comma-delimited address,amount', () => {
    const { rows, errors } = parseRecipientTableText(`${A},5000\n${B},3000`);
    expect(errors).toHaveLength(0);
    expect(rows).toEqual([
      { line: 1, name: undefined, address: A, rawAmount: '5000' },
      { line: 2, name: undefined, address: B, rawAmount: '3000' },
    ]);
  });

  test('tab-delimited (TSV)', () => {
    const { rows } = parseRecipientTableText(`${A}\t5000\n${B}\t3000`);
    expect(rows.map((r) => [r.address, r.rawAmount])).toEqual([[A, '5000'], [B, '3000']]);
  });

  test('semicolon-delimited', () => {
    const { rows } = parseRecipientTableText(`${A};5000\n${B};3000`);
    expect(rows.map((r) => r.rawAmount)).toEqual(['5000', '3000']);
  });
});

describe('parseRecipientTableText — header + column order', () => {
  test('drops a header row that has no address', () => {
    const { rows } = parseRecipientTableText(`name,address,amount\nAlice,${A},5000`);
    expect(rows).toEqual([{ line: 1, name: 'Alice', address: A, rawAmount: '5000' }]);
  });

  test('无 header 时首行数据不丢失', () => {
    const { rows } = parseRecipientTableText(`${A},5000`);
    expect(rows).toHaveLength(1);
    expect(rows[0].address).toBe(A);
  });

  test('infers column order: amount,address and name,address,amount both parse', () => {
    const a = parseRecipientTableText(`5000,${A}`); // amount first
    expect(a.rows[0]).toMatchObject({ address: A, rawAmount: '5000' });
    const b = parseRecipientTableText(`Alice,${A},5000`); // name,address,amount
    expect(b.rows[0]).toMatchObject({ name: 'Alice', address: A, rawAmount: '5000' });
  });
});

describe('parseRecipientTableText — cleaning + robustness', () => {
  test('strips a BOM and CRLF line endings', () => {
    const { rows } = parseRecipientTableText(`﻿${A},5000\r\n${B},3000\r\n`);
    expect(rows.map((r) => r.address)).toEqual([A, B]);
  });

  test('strips currency symbol + thousands separators inside a single cell', () => {
    // semicolon-delimited so the thousands comma stays inside one cell
    const { rows } = parseRecipientTableText(`${A};¥5,000.50`);
    expect(rows[0].rawAmount).toBe('5000.50');
  });

  test('mixed-case (checksummed) addresses are preserved as written', () => {
    const checksummed = '0x' + 'Ab'.repeat(20); // 0x lowercase, hex mixed-case (EIP-55 style)
    const { rows } = parseRecipientTableText(`${checksummed},5000`);
    expect(rows[0].address).toBe(checksummed);
  });

  test('skips fully blank lines between rows', () => {
    const { rows } = parseRecipientTableText(`${A},5000\n\n\n${B},3000`);
    expect(rows).toHaveLength(2);
  });
});

describe('parseRecipientTableText — errors', () => {
  test('a row with no valid address is reported (line numbering excludes header)', () => {
    const { rows, errors } = parseRecipientTableText(`address,amount\n${A},5000\n0xdeadbeef,3000`);
    expect(rows).toHaveLength(1);
    expect(errors).toEqual([{ line: 2, raw: '0xdeadbeef , 3000', reason: 'no-address' }]);
  });

  test('a row with an address but no positive amount is reported', () => {
    const { rows, errors } = parseRecipientTableText(`${A},abc\n${B},0`);
    expect(rows).toHaveLength(0);
    expect(errors.map((e) => e.reason)).toEqual(['no-amount', 'no-amount']);
  });

  test('a second address-less row is an error, not a swallowed "header"', () => {
    const { rows, errors } = parseRecipientTableText(`${A},5000\njust some text`);
    expect(rows).toHaveLength(1);
    expect(errors).toEqual([{ line: 2, raw: 'just some text', reason: 'no-address' }]);
  });

  test('empty input ⇒ empty result', () => {
    expect(parseRecipientTableText('')).toEqual({ rows: [], errors: [] });
    expect(parseRecipientTableText('   \n  \n')).toEqual({ rows: [], errors: [] });
  });
});

describe('parseRecipientTable — dispatch', () => {
  test('string input routes to the text parser', async () => {
    const { rows } = await parseRecipientTable(`${A},5000`);
    expect(rows[0].address).toBe(A);
  });

  test('an .xlsx filename routes through lazily-loaded SheetJS', async () => {
    const mockSheet = {};
    const read = jest.fn(() => ({ SheetNames: ['Sheet1'], Sheets: { Sheet1: mockSheet } }));
    const sheet_to_json = jest.fn(() => [
      ['name', 'address', 'amount'], // header — dropped
      ['Alice', A, 5000],            // numeric cell, as Excel delivers it
      ['Bob', B, '3000'],
      ['Carol', C, ''],             // missing amount → error
    ]);
    jest.doMock('xlsx', () => ({ read, utils: { sheet_to_json } }), { virtual: true });

    const { rows, errors } = await parseRecipientTable(new Uint8Array([1, 2, 3]), 'payroll.xlsx');
    expect(read).toHaveBeenCalled();
    expect(rows.map((r) => [r.name, r.address, r.rawAmount])).toEqual([
      ['Alice', A, '5000'],
      ['Bob', B, '3000'],
    ]);
    expect(errors).toEqual([{ line: 3, raw: 'Carol , ' + C + ' , ', reason: 'no-amount' }]);
  });
});
