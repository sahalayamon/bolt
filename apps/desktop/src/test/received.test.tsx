import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';

vi.mock('../lib/api', async () => (await import('./apiMock')).apiModule);
vi.mock('sonner', () => ({
  toast: Object.assign(vi.fn(), { success: vi.fn(), error: vi.fn(), warning: vi.fn() }),
  Toaster: () => null,
}));

import { api, resetApiMock } from './apiMock';
import { ReceivedBadge } from '../components/ReceivedBadge';

beforeEach(() => {
  resetApiMock();
  vi.clearAllMocks();
});

describe('received badge', () => {
  it('shows the check first, then the picture, and can be dragged out', async () => {
    api.receivedPreview.mockResolvedValueOnce('data:image/png;base64,AAAA');
    const { container } = render(<ReceivedBadge id="t9" />);
    const badge = screen.getByRole('img', { name: 'Received' });

    // before the morph a drag does nothing
    fireEvent.mouseDown(badge, { button: 0, clientX: 10, clientY: 10, buttons: 1 });
    fireEvent.mouseMove(badge, { clientX: 60, clientY: 60, buttons: 1 });
    expect(api.dragReceived).not.toHaveBeenCalled();

    const draggable = await screen.findByRole('img', { name: /Drag it into another app/ }, { timeout: 3000 });
    await waitFor(() => expect(container.querySelector('img[src^="data:image/png"]')).not.toBeNull());

    fireEvent.mouseDown(draggable, { button: 0, clientX: 10, clientY: 10, buttons: 1 });
    fireEvent.mouseMove(draggable, { clientX: 12, clientY: 12, buttons: 1 }); // below the threshold
    expect(api.dragReceived).not.toHaveBeenCalled();
    fireEvent.mouseMove(draggable, { clientX: 60, clientY: 60, buttons: 1 });
    fireEvent.mouseMove(draggable, { clientX: 90, clientY: 90, buttons: 1 }); // only one drag per press
    expect(api.dragReceived).toHaveBeenCalledTimes(1);
    expect(api.dragReceived).toHaveBeenCalledWith('t9');
  });

  it('falls back to a file icon when there is no picture', async () => {
    api.receivedPreview.mockResolvedValueOnce(null);
    const { container } = render(<ReceivedBadge id="t10" />);
    await screen.findByRole('img', { name: /Drag it into another app/ }, { timeout: 3000 });
    expect(container.querySelector('img')).toBeNull();
  });
});
