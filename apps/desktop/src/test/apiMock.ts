import { vi } from 'vitest';
import type { EventMap } from '../lib/api';
import type { Settings, Transfer, TrustedDevice } from '../lib/types';

type Handler = (payload: never) => void;
const handlers = new Map<string, Set<Handler>>();

/** Deliver a backend event to subscribers registered via `api.on`. */
export function emit<K extends keyof EventMap>(name: K, payload: EventMap[K]) {
  handlers.get(name)?.forEach((h) => (h as (p: EventMap[K]) => void)(payload));
}

export const defaultSettings: Settings = {
  deviceName: 'TEST-PC',
  downloadDir: 'C:\Downloads\Bolt',
  autoAcceptTrusted: false,
  theme: 'dark',
  reduceMotion: 'system',
  notifications: true,
  startOnLogin: false,
  runInBackground: true,
  contextMenu: false,
};

export function makeTransfer(p: Partial<Transfer> = {}): Transfer {
  return {
    id: 't1',
    direction: 'sent',
    peerId: 'a'.repeat(64),
    peerName: 'Pixel 8',
    state: 'transferring',
    bytesDone: 5 * 1024 * 1024,
    bytesTotal: 20 * 1024 * 1024,
    filesDone: 0,
    filesTotal: 1,
    currentFile: 'holiday.mp4',
    speedBps: 2 * 1024 * 1024,
    etaSecs: 7,
    error: null,
    title: 'holiday.mp4',
    startedUnix: 1_700_000_000,
    finishedUnix: null,
    canRetry: false,
    ...p,
  };
}

const fn = <T>(v: T) => vi.fn(async () => v);

export const api = {
  appInfo: vi.fn(async () => ({
    deviceId: 'f'.repeat(64),
    shortId: 'ffffffff',
    deviceName: 'TEST-PC',
    port: 47600,
    downloadDir: 'C:\Downloads\Bolt',
    discoveryActive: true,
    discoveryError: null,
    localIp: '192.168.1.10',
    version: '0.1.0',
  })),
  listDevices: fn([] as unknown[]),
  refreshDevices: fn(undefined),
  listTrusted: fn([] as unknown[]),
  renameTrusted: fn(undefined),
  revokeTrusted: fn(undefined),
  scanPaths: fn({ fileCount: 2, totalBytes: 3072, files: [{ name: 'a.txt', size: 1024 }, { name: 'b.txt', size: 2048 }] }),
  startSend: fn(makeTransfer()),
  listTransfers: fn([] as unknown[]),
  cancelTransfer: fn(undefined),
  retryTransfer: fn(makeTransfer()),
  dismissTransfer: fn(undefined),
  respondOffer: fn(undefined),
  respondPair: fn(undefined),
  beginPairing: fn({ uri: 'bolt://pair?v=1&id=x', qrSvg: '<svg xmlns="http://www.w3.org/2000/svg"></svg>', expiresUnix: Math.floor(Date.now() / 1000) + 300, address: '192.168.1.10:47600' }),
  endPairing: fn(undefined),
  pairWithUri: fn<TrustedDevice>({ id: 'b'.repeat(64), shortId: 'bbbbbbbb', name: 'Phone', pairedAt: 1, lastSeen: 1, online: true }),
  historyList: fn([] as unknown[]),
  historyClear: fn(0),
  getSettings: vi.fn(async () => defaultSettings),
  updateSettings: vi.fn(async (s: Settings) => s),
  openDownloadDir: fn(undefined),
  revealTransfer: fn(undefined),
  openReceived: fn(undefined),
  openLink: fn(undefined),
  receivedPreview: fn(null as string | null),
  dragReceived: fn(undefined),
  openMain: fn(undefined),
  draftInMain: fn(undefined),
  quitApp: fn(undefined),
  takePendingDraft: fn(null as { paths: string[]; deviceId: string | null } | null),
  hideTrayPanel: fn(undefined),
  stageClipboard: vi.fn(async (name: string) => `C:/tmp/${name}`),
  pickFiles: fn([] as string[]),
  pickFolder: fn(null as string | null),
  on: vi.fn(async (name: string, cb: Handler) => {
    const set = handlers.get(name) ?? new Set();
    set.add(cb);
    handlers.set(name, set);
    return () => set.delete(cb);
  }),
  onFileDrop: vi.fn(async () => () => {}),
  win: {
    minimize: vi.fn(async () => {}),
    toggleMaximize: vi.fn(async () => {}),
    close: vi.fn(async () => {}),
    isMaximized: vi.fn(async () => false),
    onResized: vi.fn(async () => () => {}),
    show: vi.fn(async () => {}),
  },
};

export const apiModule = {
  api,
  errorMessage: (e: unknown) => (e instanceof Error ? e.message : String(e)),
};

export function resetApiMock() {
  handlers.clear();
  for (const v of Object.values(api)) {
    if (typeof v === 'function') (v as ReturnType<typeof vi.fn>).mockClear();
  }
}
