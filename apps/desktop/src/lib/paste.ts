import { api } from './api';

const MAX_FILES = 20;

function stamp(): string {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}.${p(d.getMinutes())}.${p(d.getSeconds())}`;
}

function extFor(mime: string): string {
  const sub = mime.split('/')[1]?.split(';')[0] ?? '';
  return sub === 'jpeg' ? 'jpg' : sub.replace(/[^a-z0-9]/gi, '') || 'bin';
}

async function stageBlob(blob: Blob, name: string): Promise<string> {
  return api.stageClipboard(name, new Uint8Array(await blob.arrayBuffer()));
}

/** Files, images or text from a `paste` event (Ctrl+V). Returns staged file paths. */
export async function pathsFromPasteEvent(dt: DataTransfer): Promise<string[]> {
  const out: string[] = [];
  const files = Array.from(dt.files).slice(0, MAX_FILES);
  for (const f of files) {
    const name = f.name || `Pasted ${f.type.startsWith('image/') ? 'image' : 'file'} ${stamp()}.${extFor(f.type)}`;
    out.push(await stageBlob(f, name));
  }
  if (out.length === 0) {
    const text = dt.getData('text/plain');
    if (text.trim()) out.push(await stageBlob(new Blob([text], { type: 'text/plain' }), `Pasted text ${stamp()}.txt`));
  }
  return out;
}

/** Clipboard read for the Paste button (the browser may ask for permission). */
export async function pathsFromClipboardApi(): Promise<string[]> {
  const out: string[] = [];
  if (navigator.clipboard?.read) {
    const items = await navigator.clipboard.read();
    for (const item of items.slice(0, MAX_FILES)) {
      const img = item.types.find((t) => t.startsWith('image/'));
      if (img) {
        out.push(await stageBlob(await item.getType(img), `Pasted image ${stamp()}.${extFor(img)}`));
      } else if (item.types.includes('text/plain')) {
        const blob = await item.getType('text/plain');
        if ((await blob.text()).trim()) out.push(await stageBlob(blob, `Pasted text ${stamp()}.txt`));
      }
    }
  } else if (navigator.clipboard?.readText) {
    const text = await navigator.clipboard.readText();
    if (text.trim()) out.push(await stageBlob(new Blob([text], { type: 'text/plain' }), `Pasted text ${stamp()}.txt`));
  }
  return out;
}
