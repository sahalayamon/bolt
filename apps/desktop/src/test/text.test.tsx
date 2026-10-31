import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('../lib/api', async () => (await import('./apiMock')).apiModule);
vi.mock('sonner', () => ({
  toast: Object.assign(vi.fn(), { success: vi.fn(), error: vi.fn(), warning: vi.fn() }),
  Toaster: () => null,
}));

import { api, defaultSettings, emit, resetApiMock } from './apiMock';
import { IncomingOfferDialog, } from '../components/Dialogs';
import { ReceivedTextDialog } from '../components/TextDialogs';
import { SendReviewView } from '../views/SendReview';
import { HistoryView } from '../views/History';
import { TransfersView } from '../views/Transfers';
import { HomeView } from '../views/Home';
import { ReceivingDialog } from '../components/ReceivingDialog';
import { makeTransfer } from './apiMock';
import { asHttpUrl } from '../lib/text';
import { useApp } from '../store/app';
import type { Device, HistoryEntry, Offer } from '../lib/types';

const initial = useApp.getState();
beforeEach(() => {
  resetApiMock();
  vi.clearAllMocks();
  useApp.setState(
    { ...initial, settings: defaultSettings, devices: [], offers: [], receivedTexts: [], draft: null, selectedId: null, view: 'home', loaded: true },
    true,
  );
});

const dev: Device = { id: 'e'.repeat(64), shortId: 'eeeeeeee', name: 'Pixel 8', kind: 'phone', addrs: [], trusted: true };

describe('asHttpUrl', () => {
  it('accepts only a single http(s) link', () => {
    expect(asHttpUrl('https://bolt.aymn.in/x?y=1')).toBe('https://bolt.aymn.in/x?y=1');
    expect(asHttpUrl('  http://example.com  ')).toBe('http://example.com');
    for (const bad of ['javascript:alert(1)', 'file:///C:/Windows', 'ftp://x.y', 'see https://a.b', 'https://', 'hello', '']) {
      expect(asHttpUrl(bad)).toBeNull();
    }
  });
});

describe('sending text', () => {
  it('Home has an inline text box: Enter sends the text alone to the selected device', async () => {
    useApp.setState({ devices: [dev], selectedId: dev.id });
    render(<HomeView />);
    const box = screen.getByRole('textbox', { name: 'Text or link to send' });
    expect(screen.getByRole('button', { name: 'Send' })).toBeDisabled();
    expect(screen.queryByRole('button', { name: 'Send text or link' })).toBeNull();
    await userEvent.type(box, 'https://bolt.aymn.in{Enter}');
    await waitFor(() => expect(api.startSend).toHaveBeenCalledWith([], { deviceId: dev.id }, 'https://bolt.aymn.in'));
    expect(api.scanPaths).not.toHaveBeenCalled();
    await waitFor(() => expect(useApp.getState().view).toBe('sending'));
  });

  it('Home Send button works, and Shift+Enter adds a line instead of sending', async () => {
    useApp.setState({ devices: [dev], selectedId: dev.id });
    render(<HomeView />);
    const box = screen.getByRole('textbox', { name: 'Text or link to send' });
    await userEvent.type(box, 'a{Shift>}{Enter}{/Shift}b');
    expect(api.startSend).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole('button', { name: 'Send' }));
    await waitFor(() => expect(api.startSend).toHaveBeenCalledWith([], { deviceId: dev.id }, 'a\nb'));
  });

  it('review screen composes text with the editor and sends it alone', async () => {
    useApp.setState({ devices: [dev], draft: { paths: [], deviceId: dev.id, text: '' }, view: 'send' });
    render(<SendReviewView />);
    const box = await screen.findByRole('textbox', { name: 'Text or link to send' });
    const send = screen.getByRole('button', { name: 'Send' });
    expect(send).toBeDisabled();
    await userEvent.type(box, 'https://bolt.aymn.in');
    await userEvent.click(screen.getByRole('button', { name: 'Done' }));
    expect(await screen.findByText(/bolt\.aymn\.in/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Send' }));
    await waitFor(() => expect(api.startSend).toHaveBeenCalledWith([], { deviceId: dev.id }, 'https://bolt.aymn.in'));
    expect(api.scanPaths).not.toHaveBeenCalled();
  });

  it('can be added next to files and removed again', async () => {
    useApp.setState({ devices: [dev], draft: { paths: ['C:/x/a.txt'], deviceId: dev.id }, view: 'send' });
    render(<SendReviewView />);
    await userEvent.click(await screen.findByRole('button', { name: /Add text or link/ }));
    await userEvent.type(screen.getByRole('textbox', { name: 'Text or link to send' }), 'note');
    await userEvent.click(screen.getByRole('button', { name: 'Done' }));
    await userEvent.click(await screen.findByRole('button', { name: 'Send' }));
    await waitFor(() => expect(api.startSend).toHaveBeenCalledWith(['C:/x/a.txt'], { deviceId: dev.id }, 'note'));
  });

  it('Paste fills the box from the clipboard and Done stays off for empty or oversize text', async () => {
    Object.defineProperty(navigator, 'clipboard', { value: { readText: vi.fn().mockResolvedValue('from clipboard') }, configurable: true });
    useApp.setState({ devices: [dev], draft: { paths: [], deviceId: dev.id, text: '' }, view: 'send' });
    render(<SendReviewView />);
    const box = await screen.findByRole('textbox', { name: 'Text or link to send' });
    expect(screen.getByRole('button', { name: 'Done' })).toBeDisabled();
    await userEvent.click(screen.getByRole('button', { name: 'Paste' }));
    await waitFor(() => expect(box).toHaveValue('from clipboard'));
    expect(screen.getByRole('button', { name: 'Done' })).toBeEnabled();
    await userEvent.clear(box);
    await userEvent.click(box);
    await userEvent.paste('x'.repeat(1024 * 1024 + 1));
    expect(await screen.findByRole('alert')).toHaveTextContent(/Too long/);
    expect(screen.getByRole('button', { name: 'Done' })).toBeDisabled();
  });
});

describe('incoming text offer', () => {
  it('shows a text card, with no file image, name, size or folder row', () => {
    const o: Offer = {
      id: 'o9', peerId: 'c'.repeat(64), peerShortId: 'cccccccc', peerName: 'Alex Laptop', peerKind: 'desktop', trusted: true,
      totalBytes: 21, freeSpace: 10 ** 10, fileCount: 1, files: [{ path: 'text-0.txt', size: 21, kind: 'text' }], downloadDir: 'C:\\Downloads\\Bolt',
    };
    useApp.setState({ offers: [o] });
    render(<IncomingOfferDialog />);
    const dialog = screen.getByRole('dialog');
    expect(within(dialog).getAllByText(/Text from Alex Laptop/).length).toBeGreaterThan(0);
    expect(within(dialog).getByTestId('offer-text-card')).toBeInTheDocument();
    expect(dialog.querySelector('img')).toBeNull();
    expect(dialog.textContent).not.toMatch(/text-0|bytes|B|Save to/);
    expect(within(dialog).getByRole('button', { name: 'Accept' })).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Decline' })).toBeInTheDocument();
  });
});

describe('received text dialog', () => {
  const arrive = (texts: string[]) =>
    act(() => emit('text-received', { transferId: 't1', peerName: 'Alex Laptop', texts }));

  it('renders text as plain text, never as HTML', async () => {
    const { container } = render(<ReceivedTextDialog />);
    useApp.getState().init();
    await waitFor(() => expect(api.on).toHaveBeenCalled());
    arrive(['<img src=x onerror=alert(1)><b>bold</b>']);
    const box = await screen.findByTestId('received-text');
    expect(box).toHaveValue('<img src=x onerror=alert(1)><b>bold</b>');
    expect(box).toHaveAttribute('readonly');
    expect(box.tagName).toBe('TEXTAREA');
    expect(document.body.querySelector('b')).toBeNull();
    expect(document.querySelector('img[src="x"]')).toBeNull();
    expect(container).toBeTruthy();
    expect(screen.queryByRole('button', { name: /Open/ })).toBeNull();
  });

  it('copies, and opens a link only after clicking Open', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
    useApp.setState({ receivedTexts: [{ transferId: 't2', peerName: 'Alex Laptop', texts: ['https://bolt.aymn.in'] }] });
    render(<ReceivedTextDialog />);
    const dialog = await screen.findByRole('dialog');
    expect(api.openLink).not.toHaveBeenCalled(); // never auto-opened
    expect(within(dialog).getAllByText(/Link from Alex Laptop/).length).toBeGreaterThan(0);
    expect(within(dialog).queryByRole('button', { name: /folder/i })).toBeNull();
    await userEvent.click(within(dialog).getByRole('button', { name: /Copy/ }));
    expect(writeText).toHaveBeenCalledWith('https://bolt.aymn.in');
    await userEvent.click(within(dialog).getByRole('button', { name: /Open/ }));
    expect(api.openLink).toHaveBeenCalledWith('https://bolt.aymn.in');
  });

  it('offers no Open for non-http text, and Done moves on to the next item', async () => {
    useApp.setState({
      receivedTexts: [
        { transferId: 't3', peerName: 'A', texts: ['javascript:alert(1)'] },
        { transferId: 't4', peerName: 'B', texts: ['second'] },
      ],
    });
    render(<ReceivedTextDialog />);
    expect(await screen.findByDisplayValue('javascript:alert(1)')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /Open/ })).toBeNull();
    await userEvent.click(screen.getByRole('button', { name: 'Done' }));
    expect(await screen.findByDisplayValue('second')).toBeInTheDocument();
  });
});

describe('history', () => {
  const row = (p: Partial<HistoryEntry>): HistoryEntry => ({
    transferId: 'h1', direction: 'received', peerId: 'c'.repeat(64), peerName: 'Alex Laptop', status: 'completed',
    fileCount: 1, title: 'Text: https://bolt.aymn.in/a-long-link', totalBytes: 21, startedUnix: 1_700_000_000, finishedUnix: 1_700_000_005, error: null, ...p,
  });

  it('lists text as "Text" with a preview and no Open button', async () => {
    api.historyList.mockResolvedValue([row({}), row({ transferId: 'h2', title: 'photo.jpg' })]);
    render(<HistoryView />);
    const rows = (await screen.findAllByTestId('history-row')) as [HTMLElement, HTMLElement];
    expect(within(rows[0]).getByText('Link')).toBeInTheDocument();
    expect(within(rows[0]).getByLabelText('Link')).toBeInTheDocument(); // text icon, not the received arrow
    expect(within(rows[0]).queryByLabelText('Received')).toBeNull();
    expect(within(rows[0]).getByText(/https:\/\/bolt\.aymn\.in\/a-long-link/)).toBeInTheDocument();
    expect(within(rows[0]).queryByRole('button', { name: /Open/ })).toBeNull();
    expect(within(rows[1]).getByRole('button', { name: /Open photo\.jpg/ })).toBeInTheDocument();
  });
});

describe('transfers list', () => {
  it('names a received text "Text" with a preview, a text icon and no file actions', () => {
    useApp.setState({
      transfers: [makeTransfer({ id: 'x1', direction: 'received', state: 'completed', title: 'Text: hello there', currentFile: null, filesTotal: 1, finishedUnix: 1 })],
    });
    render(<TransfersView />);
    const row = screen.getByTestId('transfer-row');
    expect(within(row).getByText('Text')).toBeInTheDocument();
    expect(within(row).getByText(/hello there/)).toBeInTheDocument();
    expect(within(row).queryByRole('button', { name: 'Open' })).toBeNull();
    expect(within(row).queryByRole('button', { name: /Show folder/ })).toBeNull();
    expect(row.textContent).not.toMatch(/file|\.txt/i);
  });

  it('calls a link "Link" and a running text "Text", never text-0.txt', () => {
    useApp.setState({
      transfers: [
        makeTransfer({ id: 'x2', direction: 'sent', state: 'completed', title: 'Text: https://bolt.aymn.in', currentFile: null, finishedUnix: 1 }),
        makeTransfer({ id: 'x3', direction: 'received', state: 'transferring', title: '', currentFile: 'text-0.txt' }),
      ],
    });
    render(<TransfersView />);
    const rows = screen.getAllByTestId('transfer-row');
    expect(within(rows[1] as HTMLElement).getByText('Link')).toBeInTheDocument();
    expect(within(rows[0] as HTMLElement).getByText('Text')).toBeInTheDocument();
    expect(document.body.textContent).not.toContain('text-0.txt');
  });
});

describe('receiving card', () => {
  it('stays closed for a finished text-only transfer (no Open / Show folder for text)', async () => {
    useApp.setState({
      transfers: [makeTransfer({ id: 'r1', direction: 'received', state: 'completed', title: 'Text: hi', currentFile: null, finishedUnix: 1_700_000_001 })],
    });
    render(<ReceivingDialog />);
    expect(screen.queryByRole('button', { name: 'Show folder' })).toBeNull();
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('calls a running text item "Text", not its internal file name', async () => {
    useApp.setState({
      transfers: [makeTransfer({ id: 'r2', direction: 'received', state: 'transferring', title: 'text-0.txt', currentFile: 'text-0.txt' })],
    });
    render(<ReceivingDialog />);
    expect(await screen.findByText('Text')).toBeInTheDocument();
    expect(screen.queryByText('text-0.txt')).toBeNull();
  });
});
