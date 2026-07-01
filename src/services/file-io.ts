/**
 * file-io (native) — pick a table/backup file and save a text file. The web
 * variant lives in `file-io.web.ts`; both export the same surface so callers
 * (`ContactsManager`, `BatchImportSheet`) never branch on platform.
 *
 * Every native module is imported lazily inside the functions, so merely importing
 * this module (e.g. from a unit test) pulls in nothing — and SheetJS/document-picker
 * stay off the startup path.
 */
export interface PickedFile {
  name: string;
  /** Present for CSV / TSV / TXT / JSON. */
  text?: string;
  /** Present for Excel workbooks (binary). */
  bytes?: Uint8Array;
}

const EXCEL_RE = /\.(xlsx|xlsm|xlsb|xls)$/i;

/** Prompt the user to pick a file; returns its contents, or null if cancelled. */
export async function pickTable(): Promise<PickedFile | null> {
  const DocumentPicker = await import('expo-document-picker');
  const res = await DocumentPicker.getDocumentAsync({
    type: [
      'text/csv',
      'text/plain',
      'text/tab-separated-values',
      'application/json',
      'application/vnd.ms-excel',
      'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
      '*/*',
    ],
    copyToCacheDirectory: true,
    multiple: false,
  });
  if (res.canceled || !res.assets?.length) return null;
  const asset = res.assets[0];
  const name = asset.name ?? 'import';
  const { File } = await import('expo-file-system');
  const file = new File(asset.uri);
  if (EXCEL_RE.test(name)) return { name, bytes: await file.bytes() };
  return { name, text: await file.text() };
}

/** Write `content` to a cache file and hand it to the OS share sheet. */
export async function saveTextFile(name: string, content: string, mime = 'text/plain'): Promise<void> {
  const { File, Paths } = await import('expo-file-system');
  const file = new File(Paths.cache, name);
  try {
    file.create({ overwrite: true });
  } catch {
    /* already exists — write below overwrites its contents */
  }
  file.write(content);
  const Sharing = await import('expo-sharing');
  if (await Sharing.isAvailableAsync()) {
    await Sharing.shareAsync(file.uri, { mimeType: mime, dialogTitle: name });
  }
}
