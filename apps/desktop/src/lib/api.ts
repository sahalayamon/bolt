// Typed wrappers over the Tauri commands/events defined in src-tauri/src/commands.rs.
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { open } from '@tauri-apps/plugin-dialog';
import type {
  AppInfo,
  Device,
  HistoryEntry,
  HistoryFilter,
  Offer,
  PairRequest,
  PairingInfo,
  ScanSummary,
  SendTarget,
  Settings,
  Transfer,
  TrustedDevice,
} from './types';

export interface EventMap {
  'devices-changed': Device[];
  'trusted-changed': null;
  'transfer-update': Transfer;
  'history-changed': null;
  'incoming-offer': Offer;
  'incoming-pair': PairRequest;
  'pair-code': string;
  /** Sent by the tray dashboard to the main window. */
  navigate: string;
  'start-draft': { paths: string[]; deviceId?: string | null };
  /** Files chosen from the Explorer right-click menu are waiting (fetch them with takePendingDraft). */
  'draft-ready': null;
}

export const api = {
  appInfo: () => invoke<AppInfo>('app_info'),
  listDevices: () => invoke<Device[]>('list_devices'),
  refreshDevices: () => invoke<void>('refresh_devices'),
  listTrusted: () => invoke<TrustedDevice[]>('list_trusted'),
  renameTrusted: (id: string, name: string) => invoke<void>('rename_trusted', { id, name }),
  revokeTrusted: (id: string) => invoke<void>('revoke_trusted', { id }),
  scanPaths: (paths: string[]) => invoke<ScanSummary>('scan_paths', { paths }),
  startSend: (paths: string[], target: SendTarget) =>
    invoke<Transfer>('start_send', { paths, target }),
  listTransfers: () => invoke<Transfer[]>('list_transfers'),
  cancelTransfer: (id: string) => invoke<void>('cancel_transfer', { id }),
  retryTransfer: (id: string) => invoke<Transfer>('retry_transfer', { id }),
  dismissTransfer: (id: string) => invoke<void>('dismiss_transfer', { id }),
  respondOffer: (id: string, accept: boolean, destDir?: string) =>
    invoke<void>('respond_offer', { id, accept, destDir: destDir ?? null }),
  respondPair: (id: string, approve: boolean) => invoke<void>('respond_pair', { id, approve }),
  beginPairing: () => invoke<PairingInfo>('begin_pairing'),
  endPairing: () => invoke<void>('end_pairing'),
  pairWithUri: (uri: string) => invoke<TrustedDevice>('pair_with_uri', { uri }),
  historyList: (filter?: HistoryFilter) => invoke<HistoryEntry[]>('history_list', { filter: filter ?? null }),
  historyClear: () => invoke<number>('history_clear'),
  getSettings: () => invoke<Settings>('get_settings'),
  updateSettings: (settings: Settings) => invoke<Settings>('update_settings', { settings }),
  openDownloadDir: () => invoke<void>('open_download_dir'),
  /** Opens the folder with the received files of this transfer selected. */
  revealTransfer: (id: string) => invoke<void>('reveal_transfer', { id }),
  /** Opens a received file with its default app (never launches programs). Works for history entries too. */
  openReceived: (id: string) => invoke<void>('open_received', { id }),
  /** Data URL of the first received picture (null for other file types). */
  receivedPreview: (id: string) => invoke<string | null>('received_preview', { id }),
  /** Starts a native drag of the received files (drop them into any app or folder). */
  dragReceived: (id: string) => invoke<void>('drag_received', { id }),
  /** Writes pasted bytes to a private temp file and returns its path (raw IPC body). */
  /** Tray dashboard: show the main window (optionally on a screen). */
  openMain: (view?: string) => invoke<void>('open_main_window', { view: view ?? null }),
  /** Tray dashboard: open the review screen in the main window with these files. */
  draftInMain: (paths: string[], deviceId?: string) => invoke<void>('draft_in_main', { paths, deviceId: deviceId ?? null }),
  quitApp: () => invoke<void>('quit_app'),
  /** Files picked from the Explorer right-click menu (null when none). */
  takePendingDraft: () => invoke<{ paths: string[]; deviceId: string | null } | null>('take_pending_draft'),
  hideTrayPanel: () => invoke<void>('hide_tray_panel'),
  stageClipboard: (name: string, bytes: Uint8Array) =>
    invoke<string>('stage_clipboard', bytes, { headers: { 'x-name': encodeURIComponent(name) } }),

  /** Native pickers (tauri-plugin-dialog). */
  async pickFiles(): Promise<string[]> {
    const r = await open({ multiple: true, directory: false });
    return Array.isArray(r) ? r : r ? [r] : [];
  },
  async pickFolder(): Promise<string | null> {
    const r = await open({ multiple: false, directory: true });
    return typeof r === 'string' ? r : null;
  },

  /** Frameless-window controls (the title bar is drawn by the app). */
  win: {
    minimize: () => getCurrentWindow().minimize(),
    toggleMaximize: () => getCurrentWindow().toggleMaximize(),
    close: () => getCurrentWindow().close(),
    isMaximized: () => getCurrentWindow().isMaximized(),
    onResized: (cb: () => void): Promise<UnlistenFn> => getCurrentWindow().onResized(cb),
    /** The window starts hidden so there is no flash of an unpainted background. */
    show: () => invoke<void>('show_when_ready'),
  },

  async on<K extends keyof EventMap>(name: K, cb: (payload: EventMap[K]) => void): Promise<UnlistenFn> {
    return listen<EventMap[K]>(name, (e) => cb(e.payload));
  },

  /** OS drag-and-drop of files/folders onto the window (Tauri delivers absolute paths). */
  async onFileDrop(handlers: {
    onEnter: () => void;
    onLeave: () => void;
    onDrop: (paths: string[]) => void;
  }): Promise<UnlistenFn> {
    return getCurrentWebview().onDragDropEvent((e) => {
      const p = e.payload;
      if (p.type === 'enter' || p.type === 'over') handlers.onEnter();
      else if (p.type === 'leave') handlers.onLeave();
      else if (p.type === 'drop') {
        handlers.onLeave();
        if (p.paths.length) handlers.onDrop(p.paths);
      }
    });
  },
};

export type Api = typeof api;

export function errorMessage(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e instanceof Error) return e.message;
  return 'Something went wrong';
}
