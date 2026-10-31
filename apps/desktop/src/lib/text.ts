/** Text and links (sent as plain-text items). The core accepts 1 byte to 1 MiB. */
export const MAX_TEXT_BYTES = 1024 * 1024;

export function textBytes(s: string): number {
  return new TextEncoder().encode(s).length;
}

/** The text as a URL when (and only when) it is a single http(s) link. */
export function asHttpUrl(text: string): string | null {
  const t = text.trim();
  if (!t || t.length > 4096 || /\s/.test(t) || !/^https?:\/\//i.test(t)) return null;
  try {
    const u = new URL(t);
    return (u.protocol === 'http:' || u.protocol === 'https:') && u.hostname ? t : null;
  } catch {
    return null;
  }
}

/** History titles of text transfers are stored as "Text: <short preview>". */
export function textPreviewOf(title: string): string | null {
  return title.startsWith('Text: ') ? title.slice(6) : null;
}

export type TextLabel = 'Text' | 'Link';

/** "Link" for a single http(s) URL, otherwise "Text". */
export function textLabelOf(text: string): TextLabel {
  return /^https?:\/\/\S+$/i.test(text.trim()) ? 'Link' : 'Text';
}

/**
 * Whether a transfer carries only a note or link, and how to name it. Text travels as an internal
 * "text-N.txt" item, so neither that name nor file actions may ever reach the UI.
 * `preview` is empty while it is still arriving (the content is only known once delivered).
 */
export function textInfo(t: { title: string; currentFile?: string | null }): { label: TextLabel; preview: string } | null {
  const preview = textPreviewOf(t.title);
  if (preview !== null) return { label: textLabelOf(preview), preview };
  if (/^text-\d+\.txt$/.test(t.title) || (!t.title && t.currentFile && /^text-\d+\.txt$/.test(t.currentFile))) {
    return { label: 'Text', preview: '' };
  }
  return null;
}

export function shortPreview(text: string, max = 80): string {
  const flat = text.replace(/\s+/g, ' ').trim();
  return flat.length > max ? `${flat.slice(0, max)}…` : flat;
}
