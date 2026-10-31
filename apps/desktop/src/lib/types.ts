// Mirrors src-tauri/src/dto.rs (camelCase JSON). Keep in sync.

export type DeviceKind = 'desktop' | 'phone' | 'tablet' | 'unknown';

export interface Device {
  id: string;
  shortId: string;
  name: string;
  kind: DeviceKind;
  addrs: string[];
  trusted: boolean;
}

export interface TrustedDevice {
  id: string;
  shortId: string;
  name: string;
  pairedAt: number;
  lastSeen: number | null;
  online: boolean;
}

export type TransferState =
  | 'connecting'
  | 'awaiting_approval'
  | 'transferring'
  | 'completed'
  | 'failed'
  | 'cancelled'
  | 'rejected';

export interface Transfer {
  id: string;
  direction: 'sent' | 'received';
  peerId: string;
  peerName: string;
  state: TransferState;
  bytesDone: number;
  bytesTotal: number;
  filesDone: number;
  filesTotal: number;
  currentFile: string | null;
  speedBps: number;
  etaSecs: number | null;
  error: string | null;
  title: string;
  startedUnix: number;
  finishedUnix: number | null;
  canRetry: boolean;
}

export interface OfferFile {
  path: string;
  size: number;
  /** "text" items are a note or link, not a file. */
  kind: 'file' | 'text';
}

export interface ReceivedText {
  transferId: string;
  peerName: string;
  texts: string[];
}

export interface Offer {
  id: string;
  peerId: string;
  peerShortId: string;
  peerName: string;
  peerKind: DeviceKind;
  trusted: boolean;
  totalBytes: number;
  freeSpace: number | null;
  fileCount: number;
  files: OfferFile[];
  downloadDir: string;
}

export interface PairRequest {
  id: string;
  peerId: string;
  peerShortId: string;
  peerName: string;
  code: string;
}

export interface PairingInfo {
  uri: string;
  qrSvg: string;
  expiresUnix: number;
  address: string;
}

export type HistoryStatus = 'completed' | 'failed' | 'cancelled' | 'rejected';

export interface HistoryEntry {
  transferId: string;
  direction: 'sent' | 'received';
  peerId: string;
  peerName: string;
  status: HistoryStatus;
  fileCount: number;
  title: string;
  totalBytes: number;
  startedUnix: number;
  finishedUnix: number;
  error: string | null;
}

export interface HistoryFilter {
  text?: string;
  status?: HistoryStatus;
  direction?: 'sent' | 'received';
}

export interface AppInfo {
  deviceId: string;
  shortId: string;
  deviceName: string;
  port: number;
  downloadDir: string;
  discoveryActive: boolean;
  discoveryError: string | null;
  localIp: string;
  version: string;
}

export type ThemePref = 'dark' | 'light' | 'system';
export type MotionPref = 'system' | 'on' | 'off';

export interface Settings {
  deviceName: string;
  downloadDir: string;
  autoAcceptTrusted: boolean;
  theme: ThemePref;
  reduceMotion: MotionPref;
  notifications: boolean;
  startOnLogin: boolean;
  runInBackground: boolean;
  contextMenu: boolean;
}

export interface SendTarget {
  deviceId?: string;
  address?: string;
}

export interface ScanSummary {
  fileCount: number;
  totalBytes: number;
  files: { name: string; size: number }[];
}

export type View = 'home' | 'send' | 'sending' | 'transfers' | 'history' | 'devices' | 'settings';
