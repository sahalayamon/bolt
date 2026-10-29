import { create } from 'zustand';
import { toast } from 'sonner';
import { api, errorMessage } from '../lib/api';
import type {
  AppInfo,
  Device,
  Offer,
  PairRequest,
  Settings,
  Transfer,
  TrustedDevice,
  View,
} from '../lib/types';

export interface Draft {
  paths: string[];
  /** Pre-selected target (a discovered device id). */
  deviceId?: string;
}

interface AppState {
  view: View;
  info: AppInfo | null;
  settings: Settings | null;
  devices: Device[];
  trusted: TrustedDevice[];
  /** Transfers of this session, newest first. */
  transfers: Transfer[];
  offers: Offer[];
  pairRequests: PairRequest[];
  /** Verification code shown on the initiating side of a pairing. */
  pairCode: string | null;
  draft: Draft | null;
  /** Device chosen in the list (detail pane). */
  selectedId: string | null;
  /** The transfer shown on the "Sending to ..." screen. */
  sendingId: string | null;
  /** Bumped when the history table changes. */
  historyVersion: number;
  loaded: boolean;

  navigate: (v: View) => void;
  selectDevice: (id: string | null) => void;
  setSendingId: (id: string | null) => void;
  setDraft: (d: Draft | null) => void;
  startDraft: (paths: string[], deviceId?: string) => void;
  setPairCode: (c: string | null) => void;
  upsertTransfer: (t: Transfer) => void;
  pushOffer: (o: Offer) => void;
  resolveOffer: (id: string) => void;
  pushPairRequest: (p: PairRequest) => void;
  resolvePairRequest: (id: string) => void;
  refreshTrusted: () => Promise<void>;
  refreshInfo: () => Promise<void>;
  setSettings: (s: Settings) => void;
  /** Loads initial data and subscribes to backend events. Returns an unsubscribe function. */
  init: () => Promise<() => void>;
}

const TERMINAL = new Set(['completed', 'failed', 'cancelled', 'rejected']);

export const useApp = create<AppState>((set, get) => ({
  view: 'home',
  info: null,
  settings: null,
  devices: [],
  trusted: [],
  transfers: [],
  offers: [],
  pairRequests: [],
  pairCode: null,
  draft: null,
  selectedId: null,
  sendingId: null,
  historyVersion: 0,
  loaded: false,

  navigate: (view) => set({ view }),
  selectDevice: (selectedId) => set({ selectedId, view: 'home' }),
  setSendingId: (sendingId) => set({ sendingId }),
  setDraft: (draft) => set({ draft }),
  startDraft: (paths, deviceId) => set({ draft: { paths, deviceId }, view: 'send' }),
  setPairCode: (pairCode) => set({ pairCode }),
  setSettings: (settings) => set({ settings }),

  upsertTransfer: (t) => {
    const prev = get().transfers.find((x) => x.id === t.id);
    set((s) => ({
      transfers: prev
        ? s.transfers.map((x) => (x.id === t.id ? t : x))
        : [t, ...s.transfers],
    }));
    const wasTerminal = prev ? TERMINAL.has(prev.state) : false;
    if (TERMINAL.has(t.state) && !wasTerminal && get().settings?.notifications !== false) {
      const who = t.peerName || 'device';
      const what = t.title || 'files';
      if (t.state === 'completed')
        toast.success(t.direction === 'sent' ? `Sent ${what} to ${who}` : `Received ${what} from ${who}`, {
          description: 'Integrity verified',
        });
      else if (t.state === 'failed') toast.error(`Transfer failed`, { description: t.error ?? undefined });
      else if (t.state === 'rejected') toast.warning(`${who} declined the transfer`);
      else if (t.state === 'cancelled') toast(`Transfer cancelled`);
    }
  },

  pushOffer: (o) =>
    set((s) => (s.offers.some((x) => x.id === o.id) ? s : { offers: [...s.offers, o] })),
  resolveOffer: (id) => set((s) => ({ offers: s.offers.filter((o) => o.id !== id) })),
  pushPairRequest: (p) =>
    set((s) => (s.pairRequests.some((x) => x.id === p.id) ? s : { pairRequests: [...s.pairRequests, p] })),
  resolvePairRequest: (id) => set((s) => ({ pairRequests: s.pairRequests.filter((p) => p.id !== id) })),

  refreshTrusted: async () => {
    try {
      set({ trusted: await api.listTrusted() });
    } catch (e) {
      toast.error(errorMessage(e));
    }
  },
  refreshInfo: async () => {
    try {
      set({ info: await api.appInfo() });
    } catch (e) {
      toast.error(errorMessage(e));
    }
  },

  init: async () => {
    const unlisten: Array<() => void> = [];
    const sub = async <T extends Parameters<typeof api.on>[0]>(
      name: T,
      cb: Parameters<typeof api.on<T>>[1],
    ) => {
      unlisten.push(await api.on(name, cb));
    };
    // Subscribe first so nothing emitted during the initial load is lost.
    await sub('devices-changed', (devices) => {
      set({ devices });
      void get().refreshTrusted();
    });
    await sub('trusted-changed', () => void get().refreshTrusted());
    await sub('transfer-update', (t) => get().upsertTransfer(t));
    await sub('history-changed', () => set((s) => ({ historyVersion: s.historyVersion + 1 })));
    await sub('incoming-offer', (o) => get().pushOffer(o));
    await sub('incoming-pair', (p) => get().pushPairRequest(p));
    await sub('pair-code', (c) => set({ pairCode: c }));
    await sub('navigate', (v) => set({ view: v as View }));
    await sub('start-draft', (d) => get().startDraft(d.paths, d.deviceId ?? undefined));
    const pickUp = () =>
      void api
        .takePendingDraft()
        .then((d) => d && get().startDraft(d.paths, d.deviceId ?? undefined))
        .catch(() => {});
    await sub('draft-ready', pickUp);
    try {
      const [info, settings, devices, trusted, transfers] = await Promise.all([
        api.appInfo(),
        api.getSettings(),
        api.listDevices(),
        api.listTrusted(),
        api.listTransfers(),
      ]);
      set({ info, settings, devices, trusted, transfers, loaded: true });
    } catch (e) {
      toast.error(errorMessage(e));
      set({ loaded: true });
    }
    pickUp(); // files from a cold start via the right-click menu
    return () => unlisten.forEach((u) => u());
  },
}));
