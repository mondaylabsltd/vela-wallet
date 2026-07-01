/**
 * file-io (web) — DOM implementation of the file pick/save surface. Pick uses a
 * transient hidden <input type=file> (Playwright can drive it via setInputFiles);
 * save streams a Blob through a download anchor. Mirrors `file-io.ts` exactly.
 */
export interface PickedFile {
  name: string;
  text?: string;
  bytes?: Uint8Array;
}

const EXCEL_RE = /\.(xlsx|xlsm|xlsb|xls)$/i;

export async function pickTable(): Promise<PickedFile | null> {
  return new Promise<PickedFile | null>((resolve) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.csv,.tsv,.txt,.json,.xlsx,.xls,text/csv,text/plain,application/json';
    input.setAttribute('data-testid', 'file-picker-input');
    input.style.position = 'fixed';
    input.style.left = '-9999px';

    let settled = false;
    const done = (v: PickedFile | null) => {
      if (settled) return;
      settled = true;
      input.remove();
      resolve(v);
    };

    input.onchange = async () => {
      const f = input.files?.[0];
      if (!f) return done(null);
      if (EXCEL_RE.test(f.name)) done({ name: f.name, bytes: new Uint8Array(await f.arrayBuffer()) });
      else done({ name: f.name, text: await f.text() });
    };
    input.oncancel = () => done(null);

    document.body.appendChild(input);
    input.click();
  });
}

export async function saveTextFile(name: string, content: string, mime = 'text/plain'): Promise<void> {
  const blob = new Blob([content], { type: `${mime};charset=utf-8` });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.download = name;
  link.href = url;
  document.body.appendChild(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(url);
}
