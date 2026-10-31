import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('../lib/api', async () => (await import('./apiMock')).apiModule);
vi.mock('sonner', () => ({
  toast: Object.assign(vi.fn(), { success: vi.fn(), error: vi.fn(), warning: vi.fn() }),
  Toaster: () => null,
}));

import { api, defaultSettings, emit, makeTransfer, resetApiMock } from './apiMock';
import { toast } from 'sonner';
import App from '../App';
import { IncomingOfferDialog, PairRequestDialog } from '../components/Dialogs';
import { ReceivingDialog } from '../components/ReceivingDialog';
import { SendingView } from '../views/Sending';
import { TransferRow } from '../views/Transfers';
import { HistoryView } from '../views/History';
import { DevicesView } from '../views/Devices';
import { SettingsView } from '../views/Settings';
import { HomeView } from '../views/Home';
import { SendReviewView } from '../views/SendReview';
import { DevicePane } from '../components/DevicePane';
import { useApp } from '../store/app';
import type { Device, HistoryEntry, Offer, TrustedDevice } from '../lib/types';

const initial = useApp.getState();
beforeEach(() => {
  resetApiMock();
  vi.clearAllMocks();
  useApp.setState(
    {
      ...initial,
      settings: defaultSettings,
      info: null,
      devices: [],
      trusted: [],
      transfers: [],
      offers: [],
      pairRequests: [],
      draft: null,
      selectedId: null,
      view: 'home',
      loaded: true,
    },
    true,
  );
});

const offer = (p: Partial<Offer> = {}): Offer => ({
  id: 'o1',
  peerId: 'c'.repeat(64),
  peerShortId: 'cccccccc',
  peerName: 'Alex Laptop',
  peerKind: 'desktop',
  trusted: false,
  totalBytes: 3 * 1024 * 1024,
  freeSpace: 10 * 1024 ** 3,
  fileCount: 2,
  files: [
    { path: 'photos/a.jpg', size: 1024 * 1024, kind: 'file' },
    { path: 'photos/b.jpg', size: 2 * 1024 * 1024, kind: 'file' },
  ],
  downloadDir: 'C:\\Downloads\\Bolt',
  ...p,
});

describe('incoming offer dialog', () => {
  it('warns about unknown devices, focuses Decline, and declines via Escape', async () => {
    useApp.setState({ offers: [offer()] });
    render(<IncomingOfferDialog />);
    const dialog = await screen.findByRole('dialog', { name: 'Incoming transfer' });
    expect(within(dialog).queryByText(/has not been paired/i)).toBeNull();
    expect(within(dialog).getByText('photos/b.jpg')).toBeInTheDocument();
    expect(within(dialog).getByText('Not paired')).toBeInTheDocument();
    expect(within(dialog).getByText('From Alex Laptop')).toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole('button', { name: 'Decline' })).toHaveFocus());
    await userEvent.keyboard('{Escape}');
    expect(api.respondOffer).toHaveBeenCalledWith('o1', false, undefined);
    await waitFor(() => expect(useApp.getState().offers).toHaveLength(0));
  });

  it('focuses Accept for paired devices and accepts', async () => {
    useApp.setState({ offers: [offer({ trusted: true })] });
    render(<IncomingOfferDialog />);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Accept' })).toHaveFocus());
    expect(screen.getByText('Paired device')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Accept' }));
    expect(api.respondOffer).toHaveBeenCalledWith('o1', true, undefined);
  });

  it('blocks Accept when the disk is too small', async () => {
    useApp.setState({ offers: [offer({ freeSpace: 1024 })] });
    render(<IncomingOfferDialog />);
    expect(await screen.findByRole('alert')).toHaveTextContent(/not enough free space/i);
    expect(screen.getByRole('button', { name: 'Accept' })).toBeDisabled();
  });

  it('keeps Tab focus inside the dialog', async () => {
    useApp.setState({ offers: [offer()] });
    render(<IncomingOfferDialog />);
    await screen.findByRole('dialog');
    for (let i = 0; i < 8; i++) await userEvent.tab();
    expect(screen.getByRole('dialog')).toContainElement(document.activeElement as HTMLElement);
  });
});

describe('pair request dialog', () => {
  it('shows the verification code and approves', async () => {
    useApp.setState({
      pairRequests: [
        { id: 'p1', peerId: 'd'.repeat(64), peerShortId: 'dddddddd', peerName: 'Phone', code: '123456' },
      ],
    });
    render(<PairRequestDialog />);
    expect(await screen.findByText('123 456')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: /codes match/i }));
    expect(api.respondPair).toHaveBeenCalledWith('p1', true);
  });
});

describe('transfer row', () => {
  it('shows progress, speed, ETA and cancels a running transfer', async () => {
    render(
      <ul>
        <TransferRow t={makeTransfer()} onDismiss={() => {}} />
      </ul>,
    );
    expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '25');
    expect(screen.getByText(/2.00 MB\/s · 7s left/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: /cancel/i }));
    expect(api.cancelTransfer).toHaveBeenCalledWith('t1');
  });

  it('offers Retry for a failed send and shows the error', async () => {
    render(
      <ul>
        <TransferRow
          t={makeTransfer({ state: 'failed', error: 'connection error', canRetry: true })}
          onDismiss={() => {}}
        />
      </ul>,
    );
    expect(screen.getByRole('alert')).toHaveTextContent('connection error');
    await userEvent.click(screen.getByRole('button', { name: /retry/i }));
    expect(api.retryTransfer).toHaveBeenCalledWith('t1');
  });

  it('marks completed transfers as verified', () => {
    render(
      <ul>
        <TransferRow
          t={makeTransfer({ state: 'completed', bytesDone: 20 * 1024 * 1024, bytesTotal: 20 * 1024 * 1024 })}
          onDismiss={() => {}}
        />
      </ul>,
    );
    expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '100');
    expect(screen.getByText(/verified by BLAKE3/i)).toBeInTheDocument();
  });
});

describe('store', () => {
  it('toasts once when a transfer reaches a terminal state', () => {
    const { upsertTransfer } = useApp.getState();
    upsertTransfer(makeTransfer());
    expect(toast.success).not.toHaveBeenCalled();
    upsertTransfer(makeTransfer({ state: 'completed' }));
    upsertTransfer(makeTransfer({ state: 'completed' }));
    expect(toast.success).toHaveBeenCalledTimes(1);
    expect(useApp.getState().transfers).toHaveLength(1);
  });

  it('stays quiet when notifications are off', () => {
    useApp.setState({ settings: { ...defaultSettings, notifications: false } });
    useApp.getState().upsertTransfer(makeTransfer({ state: 'failed', error: 'x' }));
    expect(toast.error).not.toHaveBeenCalled();
  });
});

describe('home (device list + detail)', () => {
  const dev: Device = {
    id: 'e'.repeat(64),
    shortId: 'eeeeeeee',
    name: 'Pixel 8',
    kind: 'phone',
    addrs: ['192.168.1.5:47600'],
    trusted: true,
  };
  const other: Device = { ...dev, id: 'd'.repeat(64), shortId: 'dddddddd', name: 'Office PC', kind: 'desktop', trusted: false };

  it('shows an honest empty state without devices', () => {
    render(<><DevicePane /><HomeView /></>);
    expect(screen.getByRole('heading', { name: 'No devices found yet' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Pixel 8' })).toBeNull();
  });

  it('lists discovered devices, filters them, and selects one', async () => {
    useApp.setState({ devices: [dev, other] });
    render(<><DevicePane /><HomeView /></>);
    await userEvent.type(screen.getByRole('searchbox', { name: 'Search devices' }), 'office');
    expect(screen.queryByRole('button', { name: 'Pixel 8' })).toBeNull();
    await userEvent.click(screen.getByRole('button', { name: 'Office PC' }));
    expect(useApp.getState().selectedId).toBe(other.id);
    expect(await screen.findByRole('heading', { name: 'Office PC' })).toBeInTheDocument();
    expect(screen.getByText(/Not paired/)).toBeInTheDocument();
  });

  it('sends files to the selected device', async () => {
    useApp.setState({ devices: [dev], selectedId: dev.id });
    api.pickFiles.mockResolvedValueOnce(['C:/x/a.txt']);
    render(<HomeView />);
    expect(screen.getByRole('heading', { name: 'Pixel 8' })).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Send files' }));
    await waitFor(() => expect(useApp.getState().view).toBe('send'));
    expect(useApp.getState().draft).toEqual({ paths: ['C:/x/a.txt'], deviceId: dev.id });
  });

  it('sends a folder to the selected device', async () => {
    useApp.setState({ devices: [dev], selectedId: dev.id });
    api.pickFolder.mockResolvedValueOnce('C:/photos');
    render(<HomeView />);
    await userEvent.click(screen.getByRole('button', { name: 'Send a folder' }));
    await waitFor(() => expect(useApp.getState().draft).toEqual({ paths: ['C:/photos'], deviceId: dev.id }));
  });

  it('offers manual IP sending from the list', async () => {
    api.pickFiles.mockResolvedValueOnce(['C:/x/a.txt']);
    render(<DevicePane />);
    await userEvent.click(screen.getByRole('button', { name: /send to an ip address/i }));
    await waitFor(() => expect(useApp.getState().draft).toEqual({ paths: ['C:/x/a.txt'], deviceId: undefined }));
  });

  it('warns when discovery is unavailable', () => {
    useApp.setState({
      info: {
        deviceId: 'f',
        shortId: 'f',
        deviceName: 'x',
        port: 1,
        downloadDir: '',
        discoveryActive: false,
        discoveryError: 'no multicast',
        localIp: '',
        version: '',
      },
    });
    render(<DevicePane />);
    expect(screen.getByRole('alert')).toHaveTextContent(/no multicast/);
  });

  it('refresh button asks the backend to look again', async () => {
    render(<DevicePane />);
    await userEvent.click(screen.getByRole('button', { name: 'Refresh devices' }));
    expect(api.refreshDevices).toHaveBeenCalled();
  });

  it('switches section from the icon row', async () => {
    render(<DevicePane />);
    await userEvent.click(screen.getByRole('button', { name: 'Settings' }));
    expect(useApp.getState().view).toBe('settings');
  });
});

describe('history', () => {
  const row = (p: Partial<HistoryEntry> = {}): HistoryEntry => ({
    transferId: 'h1',
    direction: 'received',
    peerId: 'x',
    peerName: 'Alex',
    status: 'completed',
    fileCount: 3,
    title: 'report.pdf',
    totalBytes: 1024,
    startedUnix: 1,
    finishedUnix: 1_700_000_000,
    error: null,
    ...p,
  });

  it('searches and filters through the backend', async () => {
    api.historyList.mockResolvedValue([row()]);
    render(<HistoryView />);
    expect(await screen.findByText('report.pdf')).toBeInTheDocument();
    await userEvent.type(screen.getByRole('searchbox'), 'rep');
    await waitFor(() =>
      expect(api.historyList).toHaveBeenLastCalledWith({ text: 'rep', direction: undefined, status: undefined }),
    );
    await userEvent.click(screen.getByRole('button', { name: 'Failed' }));
    await waitFor(() =>
      expect(api.historyList).toHaveBeenLastCalledWith({ text: 'rep', direction: undefined, status: 'failed' }),
    );
  });

  it('shows a no-match state when filtering returns nothing', async () => {
    api.historyList.mockResolvedValue([]);
    render(<HistoryView />);
    await userEvent.click(await screen.findByRole('button', { name: 'Sent' }));
    expect(await screen.findByText('No matching transfers')).toBeInTheDocument();
  });

  it('requires confirmation before clearing', async () => {
    api.historyList.mockResolvedValue([row()]);
    render(<HistoryView />);
    await screen.findByText('report.pdf');
    await userEvent.click(screen.getByRole('button', { name: /clear history/i }));
    expect(api.historyClear).not.toHaveBeenCalled();
    const dlg = await screen.findByRole('dialog', { name: 'Clear all history?' });
    await userEvent.click(within(dlg).getByRole('button', { name: 'Clear history' }));
    await waitFor(() => expect(api.historyClear).toHaveBeenCalled());
  });

  it('reloads when the backend reports a history change', async () => {
    api.historyList.mockResolvedValue([]);
    render(<HistoryView />);
    await screen.findByText('No history yet');
    api.historyList.mockResolvedValue([row({ title: 'fresh.zip' })]);
    act(() => useApp.setState((s) => ({ historyVersion: s.historyVersion + 1 })));
    expect(await screen.findByText('fresh.zip')).toBeInTheDocument();
  });
});

describe('devices', () => {
  const td: TrustedDevice = {
    id: 'b'.repeat(64),
    shortId: 'bbbbbbbb',
    name: 'Old Name',
    pairedAt: 1_700_000_000,
    lastSeen: null,
    online: true,
  };

  it('renames a paired device', async () => {
    useApp.setState({ trusted: [td] });
    api.listTrusted.mockResolvedValue([td]);
    render(<DevicesView />);
    await userEvent.click(screen.getByRole('button', { name: 'Rename Old Name' }));
    const input = screen.getByLabelText('New name for Old Name');
    await userEvent.clear(input);
    await userEvent.type(input, 'Living room{Enter}');
    expect(api.renameTrusted).toHaveBeenCalledWith(td.id, 'Living room');
  });

  it('confirms before revoking', async () => {
    useApp.setState({ trusted: [td] });
    api.listTrusted.mockResolvedValue([td]);
    render(<DevicesView />);
    await userEvent.click(screen.getByRole('button', { name: 'Remove Old Name' }));
    expect(api.revokeTrusted).not.toHaveBeenCalled();
    const dlg = await screen.findByRole('dialog', { name: 'Remove this device?' });
    await userEvent.click(within(dlg).getByRole('button', { name: /^Remove$/ }));
    await waitFor(() => expect(api.revokeTrusted).toHaveBeenCalledWith(td.id));
  });

  it('shows the QR and pairing link when pairing', async () => {
    render(<DevicesView />);
    await userEvent.click(screen.getByRole('button', { name: /pair a device/i }));
    expect(await screen.findByRole('img', { name: 'Pairing QR code' })).toBeInTheDocument();
    expect(screen.getByLabelText('Pairing link')).toHaveValue('bolt://pair?v=1&id=x');
  });

  it('shows the code on the initiating side while waiting for approval', async () => {
    let finish: (v: TrustedDevice) => void = () => {};
    api.pairWithUri.mockReturnValueOnce(new Promise((r) => (finish = r)));
    render(<DevicesView />);
    await userEvent.click(screen.getByRole('button', { name: /enter pairing link/i }));
    await userEvent.type(screen.getByRole('textbox'), 'bolt://pair?v=1&id=y');
    await userEvent.click(screen.getByRole('button', { name: 'Pair' }));
    await act(async () => { await useApp.getState().init(); emit('pair-code', '654321'); });
    expect(await screen.findByText('654 321')).toBeInTheDocument();
    await act(async () => finish(td));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Enter pairing link' })).toBeNull());
  });
});

describe('settings', () => {
  it('saves edits and keeps Save disabled until something changes', async () => {
    render(<SettingsView />);
    const save = screen.getByRole('button', { name: 'Save changes' });
    expect(save).toBeDisabled();
    const name = screen.getByLabelText('Device name');
    await userEvent.clear(name);
    await userEvent.type(name, 'Studio PC');
    await userEvent.click(screen.getByRole('switch', { name: /auto-accept/i }));
    expect(save).toBeEnabled();
    await userEvent.click(save);
    await waitFor(() =>
      expect(api.updateSettings).toHaveBeenCalledWith({
        ...defaultSettings,
        deviceName: 'Studio PC',
        autoAcceptTrusted: true,
      }),
    );
    expect(useApp.getState().settings?.deviceName).toBe('Studio PC');
  });
});

describe('app shell', () => {
  it('loads state, navigates with Ctrl+number and applies appearance preferences', async () => {
    api.getSettings.mockResolvedValue({ ...defaultSettings, reduceMotion: 'on', theme: 'light' });
    useApp.setState({ loaded: false, settings: null });
    render(<App />);
    expect(await screen.findByRole('searchbox', { name: 'Search devices' })).toBeInTheDocument();
    expect(document.documentElement.dataset.reduceMotion).toBe('on');
    expect(document.documentElement.dataset.theme).toBe('light');
    await userEvent.keyboard('{Control>}4{/Control}');
    expect(await screen.findByRole('heading', { name: 'Devices' })).toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: 'Devices' }).some((b) => b.getAttribute('aria-current') === 'page')).toBe(true);
  });

  it('opens the offer dialog when the backend emits an incoming offer', async () => {
    useApp.setState({ loaded: false, settings: null });
    render(<App />);
    await screen.findByRole('searchbox', { name: 'Search devices' });
    act(() => emit('incoming-offer', offer()));
    expect(await screen.findByRole('dialog', { name: 'Incoming transfer' })).toBeInTheDocument();
  });
});

describe('send review', () => {
  const dev: Device = {
    id: 'e'.repeat(64),
    shortId: 'eeeeeeee',
    name: 'Pixel 8',
    kind: 'phone',
    addrs: ['192.168.1.5:47600'],
    trusted: true,
  };

  it('does not ask for a recipient when a device is already chosen', async () => {
    useApp.setState({ devices: [dev], draft: { paths: ['C:/x/a.txt'], deviceId: dev.id }, view: 'send' });
    render(<SendReviewView />);
    expect(await screen.findByRole('heading', { name: 'Send to Pixel 8' })).toBeInTheDocument();
    expect(await screen.findByText('a.txt')).toBeInTheDocument();
    expect(screen.queryByRole('radiogroup')).toBeNull();
    await userEvent.click(screen.getByRole('button', { name: 'Send' }));
    await waitFor(() => expect(api.startSend).toHaveBeenCalledWith(['C:/x/a.txt'], { deviceId: dev.id }));
    expect(useApp.getState().view).toBe('sending');
    expect(useApp.getState().sendingId).toBe('t1');
  });

  it('asks for a recipient when none was chosen', async () => {
    useApp.setState({ devices: [dev], draft: { paths: ['C:/x/a.txt'] }, view: 'send' });
    render(<SendReviewView />);
    expect(await screen.findByRole('radiogroup')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Send' })).toBeDisabled();
    await userEvent.click(screen.getByRole('radio', { name: /Pixel 8/ }));
    expect(screen.getByRole('button', { name: 'Send' })).toBeEnabled();
  });
});

describe('paste', () => {
  it('Ctrl+V of text stages a file and opens the review for the selected device', async () => {
    useApp.setState({ loaded: false, settings: null });
    render(<App />);
    await screen.findByRole('searchbox', { name: 'Search devices' });
    api.stageClipboard.mockResolvedValueOnce('C:/tmp/Pasted text.txt');
    fireEvent.paste(document.body, { clipboardData: { files: [], getData: () => 'hello' } });
    await waitFor(() => expect(useApp.getState().draft?.paths).toEqual(['C:/tmp/Pasted text.txt']));
    expect(api.stageClipboard.mock.calls[0]?.[0]).toMatch(/^Pasted text .*\.txt$/);
    expect(useApp.getState().view).toBe('send');
  });

  it('Paste tile reports a blocked clipboard instead of failing silently', async () => {
    const dev: Device = { id: 'e'.repeat(64), shortId: 'eeeeeeee', name: 'Pixel 8', kind: 'phone', addrs: [], trusted: true };
    Object.defineProperty(navigator, 'clipboard', {
      value: { read: vi.fn().mockRejectedValue(new Error('denied')) },
      configurable: true,
    });
    useApp.setState({ devices: [dev], selectedId: dev.id });
    render(<HomeView />);
    await userEvent.click(screen.getByRole('button', { name: 'Paste' }));
    await waitFor(() => expect(toast.error).toHaveBeenCalled());
  });
});

describe('receiving card', () => {
  const incoming = (p: Partial<ReturnType<typeof makeTransfer>> = {}) =>
    makeTransfer({ direction: 'received', peerName: "Sahal's S25", filesTotal: 45, filesDone: 22, currentFile: 'IMG-0093.jpg', title: 'IMG-0093.jpg', ...p });

  it('shows ring percent, file, speed/ETA and cancels', async () => {
    useApp.setState({ transfers: [incoming({ bytesDone: 31_700_000, bytesTotal: 61_400_000 })] });
    render(<ReceivingDialog />);
    const dialog = await screen.findByRole('dialog', { name: 'Receiving files' });
    expect(within(dialog).getByText("From Sahal's S25")).toBeInTheDocument();
    expect(within(dialog).getByText('52%')).toBeInTheDocument();
    expect(within(dialog).getByText('IMG-0093.jpg')).toBeInTheDocument();
    expect(within(dialog).getByText(/File 23 of 45/)).toBeInTheDocument();
    await userEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
    expect(api.cancelTransfer).toHaveBeenCalledWith('t1');
  });

  it('shows a finished state with Done, and Escape only hides it', async () => {
    useApp.setState({ transfers: [incoming({ state: 'completed', bytesDone: 10, bytesTotal: 10, filesDone: 45 })] });
    render(<ReceivingDialog />);
    // not active, so it only appears once an active transfer has been focused first
    expect(screen.queryByRole('dialog')).toBeNull();
    act(() => useApp.setState({ transfers: [incoming()] }));
    await screen.findByRole('dialog', { name: 'Receiving files' });
    act(() => useApp.setState({ transfers: [incoming({ state: 'completed', bytesDone: 10, bytesTotal: 10, filesDone: 45 })] }));
    expect(await screen.findByText('Done')).toBeInTheDocument();
    await userEvent.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
    expect(api.cancelTransfer).not.toHaveBeenCalled();
  });
});

describe('sending screen', () => {
  const sending = (p: Partial<ReturnType<typeof makeTransfer>> = {}) =>
    makeTransfer({ peerName: "Sahal's S25", filesTotal: 8, filesDone: 3, currentFile: 'detail_portrait_HD.png', bytesDone: 3_000_000, bytesTotal: 20_000_000, ...p });

  it('shows the target, ring percent, file line and cancels', async () => {
    useApp.setState({ transfers: [sending()], sendingId: 't1', view: 'sending' });
    render(<SendingView />);
    expect(screen.getByRole('heading', { name: /Sending to Sahal's S25/ })).toBeInTheDocument();
    expect(screen.getByText('15%')).toBeInTheDocument();
    expect(screen.getByText('detail_portrait_HD.png')).toBeInTheDocument();
    expect(screen.getByText(/File 4 of 8/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(api.cancelTransfer).toHaveBeenCalledWith('t1');
  });

  it('shows Sent to ... with Done when finished, and Done returns home', async () => {
    useApp.setState({ transfers: [sending({ state: 'completed', bytesDone: 20_000_000 })], sendingId: 't1', view: 'sending' });
    render(<SendingView />);
    expect(screen.getByRole('heading', { name: /Sent to Sahal's S25/ })).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Done' }));
    expect(useApp.getState().view).toBe('home');
    expect(useApp.getState().sendingId).toBeNull();
  });

  it('offers Retry after a failure', async () => {
    useApp.setState({ transfers: [sending({ state: 'failed', error: 'connection error', canRetry: true })], sendingId: 't1', view: 'sending' });
    render(<SendingView />);
    expect(screen.getByRole('alert')).toHaveTextContent('connection error');
    await userEvent.click(screen.getByRole('button', { name: 'Retry' }));
    expect(api.retryTransfer).toHaveBeenCalledWith('t1');
  });
});
