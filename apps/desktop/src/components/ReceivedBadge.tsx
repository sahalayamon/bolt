import { useEffect, useState } from 'react';
import { toast } from 'sonner';
import { Check, File as FileIcon } from 'lucide-react';
import { api, errorMessage } from '../lib/api';
import { cx } from './ui';

const SIZE = 150;
/** How long the green check is shown before the ring turns into the picture. */
const CHECK_MS = 1100;

/**
 * The "received" mark. First the green ring with a check; then the check fades and the circle turns into a
 * rounded square showing the received picture (or a file icon). That square is a real drag source: pull it out of
 * the window and drop it into any folder or app.
 */
export function ReceivedBadge({ id }: { id: string }) {
  const [morphed, setMorphed] = useState(false);
  const [preview, setPreview] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    api
      .receivedPreview(id)
      .then((p) => live && setPreview(p))
      .catch(() => {});
    const t = setTimeout(() => setMorphed(true), CHECK_MS);
    return () => {
      live = false;
      clearTimeout(t);
    };
  }, [id]);

  // Listen on the window once the button is down, so a quick pull out of the tile is still caught.
  const onMouseDown = (e: React.MouseEvent) => {
    if (e.button !== 0 || !morphed) return;
    const from = { x: e.clientX, y: e.clientY };
    const move = (m: MouseEvent) => {
      if ((m.buttons & 1) === 0) return stop();
      if (Math.hypot(m.clientX - from.x, m.clientY - from.y) < 5) return;
      stop();
      // Hands over to the operating system's own drag-and-drop, so the file can land in any app or folder.
      api.dragReceived(id).catch((err) => toast.error(errorMessage(err)));
    };
    const stop = () => {
      window.removeEventListener('mousemove', move);
      window.removeEventListener('mouseup', stop);
    };
    window.addEventListener('mousemove', move);
    window.addEventListener('mouseup', stop);
  };

  return (
    <div className="flex flex-col items-center gap-2">
      <div
        role="img"
        aria-label={morphed ? 'Received file. Drag it into another app or folder.' : 'Received'}
        title={morphed ? 'Drag me into any app or folder' : undefined}
        onMouseDown={onMouseDown}
        className={cx(
          'relative my-1 grid place-items-center overflow-hidden border-[10px] transition-all duration-500 ease-out select-none',
          morphed ? 'cursor-grab border-ok bg-elevated shadow-lg active:cursor-grabbing' : 'border-ok',
        )}
        style={{ width: SIZE, height: SIZE, borderRadius: morphed ? 28 : SIZE / 2, touchAction: 'none' }}
      >
        <Check
          className={cx('absolute h-14 w-14 text-ok transition-opacity duration-300', morphed ? 'opacity-0' : 'opacity-100')}
          strokeWidth={2.25}
          aria-hidden="true"
        />
        <div className={cx('absolute inset-0 grid place-items-center transition-opacity duration-500', morphed ? 'opacity-100' : 'opacity-0')}>
          {preview ? (
            <img src={preview} alt="" draggable={false} className="h-full w-full object-cover" />
          ) : (
            <FileIcon className="h-14 w-14 text-muted" strokeWidth={1.25} aria-hidden="true" />
          )}
        </div>
      </div>
      <p className={cx('t-caption h-4 transition-opacity duration-500', morphed ? 'opacity-100' : 'opacity-0')} aria-hidden={!morphed}>
        Drag it into any app or folder
      </p>
    </div>
  );
}
