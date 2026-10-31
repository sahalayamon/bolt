import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('../lib/api', async () => (await import('./apiMock')).apiModule);
vi.mock('sonner', () => ({
  toast: Object.assign(vi.fn(), { success: vi.fn(), error: vi.fn(), warning: vi.fn() }),
  Toaster: () => null,
}));

import { defaultSettings, makeTransfer, resetApiMock } from './apiMock';
import { IncomingOfferDialog, SentDialog } from '../components/Dialogs';
import { useApp } from '../store/app';
import type { Offer } from '../lib/types';

const initial = useApp.getState();
beforeEach(() => {
  resetApiMock();
  vi.clearAllMocks();
  useApp.setState(
    { ...initial, settings: defaultSettings, devices: [], offers: [], receivedTexts: [], pairRequests: [], sentNotice: null, transfers: [], draft: null, view: 'home', loaded: true },
    true,
  );
});

const offer: Offer = {
  id: 'o1', peerId: 'c'.repeat(64), peerShortId: 'cccccccc', peerName: 'Alex Laptop', peerKind: 'desktop', trusted: true,
  totalBytes: 10, freeSpace: 10 ** 10, fileCount: 1, files: [{ path: 'a.txt', size: 10, kind: 'file' }], downloadDir: 'C:\Downloads\Bolt',
};
const sent = (state: 'transferring' | 'completed') =>
  makeTransfer({ id: 's1', direction: 'sent', state, title: 'holiday.mp4', peerName: 'Pixel 8' });

describe('sent popup', () => {
  it('shows after a send completes and stays until Done', async () => {
    render(<SentDialog />);
    expect(screen.queryByRole('dialog')).toBeNull();
    act(() => useApp.getState().upsertTransfer(sent('transferring')));
    expect(screen.queryByRole('dialog')).toBeNull();
    act(() => useApp.getState().upsertTransfer(sent('completed')));
    const dialog = await screen.findByRole('dialog');
    expect(dialog).toHaveTextContent('Sent successfully');
    expect(dialog).toHaveTextContent('holiday.mp4 to Pixel 8');
    await new Promise((r) => setTimeout(r, 300));
    expect(screen.getByRole('dialog')).toBeInTheDocument(); // no timer
    await userEvent.click(screen.getByRole('button', { name: 'Done' }));
    await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
  });

  it('is not shown for received transfers', () => {
    render(<SentDialog />);
    act(() => useApp.getState().upsertTransfer(makeTransfer({ id: 'r1', direction: 'received', state: 'completed' })));
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('a new incoming offer dismisses it, and the offer is the only dialog', async () => {
    render(
      <>
        <SentDialog />
        <IncomingOfferDialog />
      </>,
    );
    act(() => useApp.getState().upsertTransfer(sent('completed')));
    await screen.findByRole('dialog');
    act(() => useApp.getState().pushOffer(offer));
    await waitFor(() => expect(screen.queryByRole('button', { name: 'Done' })).toBeNull());
    expect(useApp.getState().sentNotice).toBeNull();
    expect(screen.getAllByRole('dialog')).toHaveLength(1);
    expect(screen.getByRole('button', { name: 'Accept' })).toBeInTheDocument();
  });

  it('a pairing request or received text also dismisses it', async () => {
    render(<SentDialog />);
    act(() => useApp.getState().upsertTransfer(sent('completed')));
    await screen.findByRole('dialog');
    act(() =>
      useApp.getState().pushPairRequest({ id: 'p1', peerId: 'd'.repeat(64), peerShortId: 'dddddddd', peerName: 'X', peerKind: 'phone', code: '123456' } as never),
    );
    expect(useApp.getState().sentNotice).toBeNull();
    act(() => useApp.getState().upsertTransfer({ ...sent('completed'), id: 's2' }));
    await screen.findByRole('dialog');
    act(() => useApp.setState((s) => ({ receivedTexts: [...s.receivedTexts, { transferId: 't', peerName: 'X', texts: ['hi'] }], sentNotice: null })));
    expect(useApp.getState().sentNotice).toBeNull();
  });

  it('does not appear while an offer is already waiting', () => {
    useApp.setState({ offers: [offer] });
    render(<SentDialog />);
    act(() => useApp.getState().upsertTransfer(sent('completed')));
    expect(useApp.getState().sentNotice).toBeNull();
  });
});
