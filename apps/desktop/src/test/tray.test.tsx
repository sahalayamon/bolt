import { beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('../lib/api', async () => (await import('./apiMock')).apiModule);
vi.mock('sonner', () => ({
  toast: Object.assign(vi.fn(), { success: vi.fn(), error: vi.fn(), warning: vi.fn() }),
  Toaster: () => null,
}));

import { api, defaultSettings, makeTransfer, resetApiMock } from './apiMock';
import TrayPanel from '../TrayPanel';
import { useApp } from '../store/app';
import type { Device } from '../lib/types';

const initial = useApp.getState();
const phone: Device = {
  id: 'e'.repeat(64),
  shortId: 'eeeeeeee',
  name: "Sahal's S25",
  kind: 'phone',
  addrs: ['192.168.1.5:47600'],
  trusted: true,
};

beforeEach(() => {
  resetApiMock();
  vi.clearAllMocks();
  api.listDevices.mockResolvedValue([phone]);
  useApp.setState({ ...initial, settings: defaultSettings, devices: [phone], transfers: [], loaded: true }, true);
});

describe('tray dashboard', () => {
  it('opens Bolt and the main screens', async () => {
    render(<TrayPanel />);
    await userEvent.click(screen.getByRole('button', { name: 'Open Bolt' }));
    expect(api.openMain).toHaveBeenCalledWith();
    await userEvent.click(screen.getByRole('button', { name: 'Settings' }));
    expect(api.openMain).toHaveBeenLastCalledWith('settings');
  });

  it('picks files for a nearby device and hands them to the main window', async () => {
    api.pickFiles.mockResolvedValueOnce(['C:/x/a.txt']);
    render(<TrayPanel />);
    await userEvent.click(await screen.findByRole('button', { name: /Sahal's S25/ }));
    await waitFor(() => expect(api.draftInMain).toHaveBeenCalledWith(['C:/x/a.txt'], phone.id));
  });

  it('Send files / Send folder go to the review screen without a recipient', async () => {
    api.pickFiles.mockResolvedValueOnce(['C:/x/a.txt']);
    api.pickFolder.mockResolvedValueOnce('C:/photos');
    render(<TrayPanel />);
    await userEvent.click(screen.getByRole('button', { name: 'Send files' }));
    await waitFor(() => expect(api.draftInMain).toHaveBeenCalledWith(['C:/x/a.txt'], undefined));
    await userEvent.click(screen.getByRole('button', { name: 'Send folder' }));
    await waitFor(() => expect(api.draftInMain).toHaveBeenLastCalledWith(['C:/photos'], undefined));
  });

  it('shows active transfers and can quit', async () => {
    useApp.setState({ transfers: [makeTransfer({ title: 'holiday.mp4' })] });
    render(<TrayPanel />);
    expect(screen.getByText('holiday.mp4')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: /Quit/ }));
    expect(api.quitApp).toHaveBeenCalled();
  });
});
