/**
 * Files, in and out — the two capabilities the `batch_import` core cannot
 * have (spec 026 Phase 6).
 *
 * Ported in spirit from src/services/file-io.ts @ f9bcb278, whose native
 * implementation is a document picker and a share sheet. The web's are an
 * `<input type="file">` and a Blob download, and the difference stops at this
 * file: the core asks for a table and receives one, or hears that the person
 * cancelled.
 *
 * SheetJS (~1 MB) is reached through a DYNAMIC import and only when an
 * actual `.xlsx` is opened, so it never sits on the startup path of a page
 * that will never see a spreadsheet.
 */

/** What a picked table carries: text for CSV/TSV, bytes for a workbook. */
export interface PickedTable {
	name: string;
	text?: string;
	bytes?: Uint8Array;
}

const TABLE_ACCEPT = '.csv,.tsv,.txt,.xlsx,.xlsm,.xlsb,.xls';

function isExcelName(name: string): boolean {
	return /\.(xlsx|xlsm|xlsb|xls)$/i.test(name);
}

/**
 * Ask for a table. Resolves with the file, or `null` when the person
 * cancelled — which the core reads as "nothing happened", not as a failure.
 *
 * The input is created, clicked and thrown away. `cancel` is not universally
 * fired, so the promise also settles when the window regains focus with no
 * file chosen: a picker that never answers would leave the core's effect
 * unanswered forever, which is the one outcome the loop cannot tolerate.
 */
export function pickTable(): Promise<PickedTable | null> {
	if (typeof document === 'undefined') return Promise.resolve(null);
	return new Promise<PickedTable | null>((resolve) => {
		const input = document.createElement('input');
		input.type = 'file';
		input.accept = TABLE_ACCEPT;
		input.style.display = 'none';
		let settled = false;
		const done = (value: PickedTable | null) => {
			if (settled) return;
			settled = true;
			window.removeEventListener('focus', onFocus);
			input.remove();
			resolve(value);
		};
		const onFocus = () => {
			// The dialog closed. Give the change event a tick to arrive first.
			setTimeout(() => done(null), 500);
		};
		input.addEventListener('change', async () => {
			const file = input.files?.[0];
			if (!file) return done(null);
			try {
				if (isExcelName(file.name)) {
					done({ name: file.name, bytes: new Uint8Array(await file.arrayBuffer()) });
				} else {
					done({ name: file.name, text: await file.text() });
				}
			} catch {
				done(null);
			}
		});
		input.addEventListener('cancel', () => done(null));
		document.body.appendChild(input);
		window.addEventListener('focus', onFocus, { once: true });
		input.click();
	});
}

/** What a picked text file carries. */
export interface PickedTextFile {
	name: string;
	text: string;
}

/**
 * Ask for ONE text file of the given kinds (`accept` is the input's own
 * grammar — `.json,.csv`). Resolves with the file, or `null` when the person
 * cancelled. The same picker discipline as `pickTable` (spec 026): created,
 * clicked, thrown away, and settled on focus-return so the caller is never
 * left waiting on a dialog that closed without a word.
 *
 * The contacts book travels this way (spec 028 US5): the text goes to the
 * core's `import_file`, which owns the format — JSON-or-CSV sniffing, the
 * column heuristics — so what this returns is bytes, not rows.
 */
export function pickTextFile(accept: string): Promise<PickedTextFile | null> {
	if (typeof document === 'undefined') return Promise.resolve(null);
	return new Promise<PickedTextFile | null>((resolve) => {
		const input = document.createElement('input');
		input.type = 'file';
		input.accept = accept;
		input.style.display = 'none';
		let settled = false;
		const done = (value: PickedTextFile | null) => {
			if (settled) return;
			settled = true;
			window.removeEventListener('focus', onFocus);
			input.remove();
			resolve(value);
		};
		const onFocus = () => {
			setTimeout(() => done(null), 500);
		};
		input.addEventListener('change', async () => {
			const file = input.files?.[0];
			if (!file) return done(null);
			try {
				done({ name: file.name, text: await file.text() });
			} catch {
				done(null);
			}
		});
		input.addEventListener('cancel', () => done(null));
		document.body.appendChild(input);
		window.addEventListener('focus', onFocus, { once: true });
		input.click();
	});
}

/**
 * Hand a file to the person. A Blob and an anchor — the web's share sheet.
 * Silent about failure for the same reason the native one is: a dismissed
 * share is not an error, and the core's own result variant says so.
 */
export async function saveTextFile(name: string, contents: string, mime: string): Promise<void> {
	await saveBlob(name, new Blob([contents], { type: mime }));
}

/** The same hand-over for bytes already in a Blob — the receive share image. */
export async function saveBlob(name: string, blob: Blob): Promise<void> {
	if (typeof document === 'undefined') return;
	const url = URL.createObjectURL(blob);
	const anchor = document.createElement('a');
	anchor.href = url;
	anchor.download = name;
	anchor.style.display = 'none';
	document.body.appendChild(anchor);
	anchor.click();
	anchor.remove();
	// Give the download a tick to start before the object URL is revoked.
	await new Promise((resolve) => setTimeout(resolve, 0));
	URL.revokeObjectURL(url);
}

/**
 * A workbook's first sheet as a cell matrix, via lazily-loaded SheetJS.
 *
 * Ported from src/services/recipient-table.ts @ f9bcb278. Only this half is
 * ported: the web hands the matrix straight to the Rust `batch_import` core,
 * which owns every rule about what a column means — the 324-line TypeScript
 * interpreter beside it on Expo has no reader here.
 */
export async function readWorkbookMatrix(bytes: Uint8Array): Promise<string[][]> {
	const XLSX = await import('xlsx');
	const workbook = XLSX.read(bytes, { type: 'array' });
	const sheet = workbook.Sheets[workbook.SheetNames[0]];
	if (!sheet) return [];
	// header:1 ⇒ array-of-arrays; defval keeps column positions stable.
	return XLSX.utils.sheet_to_json<string[]>(sheet, { header: 1, defval: '', raw: false });
}
