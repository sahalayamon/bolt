const UNITS = ['B', 'KB', 'MB', 'GB', 'TB'];

export function formatBytes(n: number): string {
  if (!Number.isFinite(n) || n < 0) return '—';
  let v = n;
  let i = 0;
  while (v >= 1024 && i < UNITS.length - 1) {
    v /= 1024;
    i++;
  }
  const digits = i === 0 || v >= 100 ? 0 : v >= 10 ? 1 : 2;
  return `${v.toFixed(digits)} ${UNITS[i]}`;
}

export function formatSpeed(bps: number): string {
  return bps > 0 ? `${formatBytes(bps)}/s` : '—';
}

export function formatEta(secs: number | null): string {
  if (secs == null || !Number.isFinite(secs)) return '—';
  const s = Math.max(0, Math.round(secs));
  if (s < 60) return `${s}s`;
  if (s < 3600) return `${Math.floor(s / 60)}m ${s % 60}s`;
  return `${Math.floor(s / 3600)}h ${Math.floor((s % 3600) / 60)}m`;
}

export function percent(done: number, total: number): number {
  if (total <= 0) return 0;
  return Math.min(100, Math.max(0, (done / total) * 100));
}

export function formatWhen(unix: number, nowMs: number = Date.now()): string {
  const d = new Date(unix * 1000);
  const diff = Math.floor(nowMs / 1000) - unix;
  if (diff < 45) return 'Just now';
  if (diff < 3600) return `${Math.max(1, Math.round(diff / 60))} min ago`;
  const today = new Date(nowMs);
  const time = d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  if (d.toDateString() === today.toDateString()) return `Today, ${time}`;
  const y = new Date(nowMs - 86_400_000);
  if (d.toDateString() === y.toDateString()) return `Yesterday, ${time}`;
  return `${d.toLocaleDateString([], { month: 'short', day: 'numeric', year: 'numeric' })}, ${time}`;
}

/** "Photos" for one folder, "2 folders and 3 files", "5 files": what was picked, with a folder counted once. */
export function selectionTitle(top: { isFolder: boolean; name: string }[], fileCount: number, suffix = ' selected'): string {
  const folders = top.filter((t) => t.isFolder);
  const loose = top.length - folders.length;
  if (folders.length === 0) return `${plural(fileCount, 'file')}${suffix}`;
  if (folders.length === 1 && loose === 0) return folders[0]!.name;
  const parts = [plural(folders.length, 'folder')];
  if (loose > 0) parts.push(plural(loose, 'file'));
  return `${parts.join(' and ')}${suffix}`;
}

/** Group an offer's `/`-separated file paths by their first component: a folder is one entry, a lone file has no `/`. */
export function topLevel(paths: string[]): { name: string; isFolder: boolean; fileCount: number }[] {
  const top: { name: string; isFolder: boolean; fileCount: number }[] = [];
  for (const p of paths) {
    const i = p.indexOf('/');
    const name = i < 0 ? p : p.slice(0, i);
    const isFolder = i >= 0;
    const hit = top.find((t) => t.isFolder === isFolder && t.name === name);
    if (hit) hit.fileCount += 1;
    else top.push({ name, isFolder, fileCount: 1 });
  }
  return top;
}

export function plural(n: number, one: string, many = `${one}s`): string {
  return `${n} ${n === 1 ? one : many}`;
}

export function groupDigits(code: string): string {
  const d = code.replace(/\D/g, '');
  return d.length === 6 ? `${d.slice(0, 3)} ${d.slice(3)}` : code;
}
