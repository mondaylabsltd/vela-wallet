/**
 * recipient-table — parse a pasted or uploaded payee table into recipient rows,
 * for the payroll batch importer. A row carries one recipient address and one
 * amount (fiat by default; the importer applies the fiat→token conversion). A
 * name column is optional and, if present, becomes the contact label.
 *
 * The text path (CSV / TSV / TXT / pasted table) is pure and synchronous. The
 * Excel path lazily `import('xlsx')` so SheetJS (~1MB) never sits on the app's
 * startup path — it loads only when a user actually opens an .xlsx file.
 *
 * Column order is inferred, not fixed: the address is the cell that looks like an
 * address, the amount is the first positive-number cell that isn't the address,
 * and any remaining text cell is the name. So `address,amount`, `amount,address`,
 * and `name,address,amount` all parse.
 */
import { isAddress } from '@/models/types';

/** One successfully-read payee row. `line` is the 1-based source row (header excluded). */
export interface ParsedRow {
  line: number;
  name?: string;
  /** The address exactly as written; the caller lowercases/validates downstream. */
  address: string;
  /** The amount as a clean numeric string ("5000", "173.88"); caller converts fiat→token. */
  rawAmount: string;
}

export interface ParseError {
  line: number;
  raw: string;
  reason: 'no-address' | 'no-amount';
}

export interface ParseResult {
  rows: ParsedRow[];
  errors: ParseError[];
}

const DELIMITERS = [',', '\t', ';'] as const;

/**
 * Pick the delimiter for the table. We prefer whichever one splits the first line
 * into a cell that actually looks like an address (so `addr;¥5,000.50` chooses `;`
 * over the thousands-comma), and otherwise the one that yields the most columns.
 */
function sniffDelimiter(firstLine: string): string {
  let best = ',';
  let bestScore = -1;
  for (const d of DELIMITERS) {
    const cells = splitCsvLine(firstLine, d);
    const score = (cells.some((c) => isAddress(c.trim())) ? 1000 : 0) + (cells.length - 1);
    if (score > bestScore) {
      best = d;
      bestScore = score;
    }
  }
  return best;
}

/** Split one CSV line, honouring simple double-quoted cells with "" escapes.
 *  Exported so the contacts CSV importer shares the exact same cell semantics. */
export function splitCsvLine(line: string, delim: string): string[] {
  const out: string[] = [];
  let cur = '';
  let inQuotes = false;
  for (let i = 0; i < line.length; i++) {
    const ch = line[i];
    if (inQuotes) {
      if (ch === '"') {
        if (line[i + 1] === '"') { cur += '"'; i++; } else inQuotes = false;
      } else cur += ch;
    } else if (ch === '"') {
      inQuotes = true;
    } else if (ch === delim) {
      out.push(cur);
      cur = '';
    } else {
      cur += ch;
    }
  }
  out.push(cur);
  return out;
}

/**
 * Reduce a cell to a positive decimal string, or '' if it isn't one. Strips
 * currency symbols, spaces, and thousands separators (`¥5,000.00` → `5000.00`).
 * NOTE: on comma-delimited text a value split by a thousands comma is already two
 * cells and can't be recovered — the CSV template documents "no thousands commas".
 * Excel cells arrive intact, so this is a text-CSV-only caveat.
 */
function cleanAmount(cell: string): string {
  const stripped = cell.replace(/[^0-9.]/g, '');
  if (!stripped) return '';
  if ((stripped.match(/\./g) || []).length > 1) return ''; // "1.2.3" — ambiguous
  const n = parseFloat(stripped);
  return Number.isFinite(n) && n > 0 ? stripped : '';
}

/** Interpret an already-split cell matrix. The first non-blank row is dropped as
 *  a header only when it carries no address (a real data row always has one);
 *  every later address-less row is reported as an error, not silently skipped. */
function interpretRows(matrix: string[][]): ParseResult {
  const rows: ParsedRow[] = [];
  const errors: ParseError[] = [];
  let dataLine = 0;
  let seenAnyRow = false;

  matrix.forEach((cellsRaw) => {
    const cells = cellsRaw.map((c) => String(c ?? '').trim());
    if (cells.every((c) => c.length === 0)) return; // blank line

    const addrIdx = cells.findIndex((c) => isAddress(c));

    // Only the very first non-blank row may be a header: if it has no address,
    // drop it silently and move on.
    if (!seenAnyRow) {
      seenAnyRow = true;
      if (addrIdx === -1) return;
    }

    dataLine += 1;
    const raw = cells.join(' , ');
    if (addrIdx === -1) {
      errors.push({ line: dataLine, raw, reason: 'no-address' });
      return;
    }

    let amount = '';
    let amtIdx = -1;
    for (let i = 0; i < cells.length; i++) {
      if (i === addrIdx) continue;
      const cleaned = cleanAmount(cells[i]);
      if (cleaned) { amount = cleaned; amtIdx = i; break; }
    }
    if (!amount) {
      errors.push({ line: dataLine, raw, reason: 'no-amount' });
      return;
    }

    const name = cells.find((c, i) => i !== addrIdx && i !== amtIdx && c.length > 0) || undefined;
    rows.push({ line: dataLine, name, address: cells[addrIdx], rawAmount: amount });
  });

  return { rows, errors };
}

/** Parse delimited text (CSV / TSV / TXT / pasted). Pure + synchronous. */
export function parseRecipientTableText(text: string): ParseResult {
  const clean = text.replace(/^﻿/, ''); // strip BOM
  const lines = clean.split(/\r\n|\r|\n/).filter((l) => l.trim().length > 0);
  if (lines.length === 0) return { rows: [], errors: [] };
  const delim = sniffDelimiter(lines[0]);
  return interpretRows(lines.map((l) => splitCsvLine(l, delim)));
}

/** Read an .xlsx/.xls workbook's first sheet into a cell matrix via lazy SheetJS. */
async function parseWorkbook(bytes: Uint8Array): Promise<string[][]> {
  const XLSX = await import('xlsx');
  const wb = XLSX.read(bytes, { type: 'array' });
  const sheet = wb.Sheets[wb.SheetNames[0]];
  if (!sheet) return [];
  // header:1 ⇒ array-of-arrays; defval keeps column positions stable.
  return XLSX.utils.sheet_to_json<string[]>(sheet, { header: 1, defval: '', raw: false });
}

function isExcelName(name?: string): boolean {
  return !!name && /\.(xlsx|xlsm|xlsb|xls)$/i.test(name);
}

/**
 * Parse a table from either text or an Excel workbook. Pass a string for
 * CSV/TSV/TXT/pasted content, or bytes (+ a `.xlsx` filename) for Excel.
 */
export async function parseRecipientTable(
  input: string | Uint8Array,
  filename?: string,
): Promise<ParseResult> {
  if (typeof input === 'string' && !isExcelName(filename)) {
    return parseRecipientTableText(input);
  }
  const bytes = typeof input === 'string' ? new TextEncoder().encode(input) : input;
  return interpretRows(await parseWorkbook(bytes));
}
