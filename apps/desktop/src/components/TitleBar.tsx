import { useEffect, useState } from 'react';
import { Copy, Minus, Square, X } from 'lucide-react';
import { api } from '../lib/api';
import { cx } from './ui';

/**
 * Frameless window chrome. The drag strip is transparent and overlays the top of both
 * panes, so the title area takes the colour of whatever is under it (no separate bar).
 */
export function TitleBar() {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    let off: (() => void) | undefined;
    let cancelled = false;
    const sync = () => void api.win.isMaximized().then(setMaximized).catch(() => {});
    sync();
    api.win
      .onResized(sync)
      .then((u) => (cancelled ? u() : (off = u)))
      .catch(() => {
        /* not inside Tauri */
      });
    return () => {
      cancelled = true;
      off?.();
    };
  }, []);

  const run = (fn: () => Promise<unknown>) => () => void fn().catch(() => {});

  return (
    <div className="pointer-events-none fixed inset-x-0 top-0 z-40 flex h-9 items-start justify-end">
      <div data-tauri-drag-region className="pointer-events-auto absolute inset-0" onDoubleClick={run(api.win.toggleMaximize)} />
      <div className="pointer-events-auto relative flex h-full">
        <Control label="Minimize" onClick={run(api.win.minimize)}>
          <Minus className="h-3.5 w-3.5" strokeWidth={1.5} />
        </Control>
        <Control label={maximized ? 'Restore' : 'Maximize'} onClick={run(api.win.toggleMaximize)}>
          {maximized ? <Copy className="h-3 w-3" strokeWidth={1.5} /> : <Square className="h-3 w-3" strokeWidth={1.5} />}
        </Control>
        <Control label="Close" danger onClick={run(api.win.close)}>
          <X className="h-4 w-4" strokeWidth={1.5} />
        </Control>
      </div>
    </div>
  );
}

function Control({
  label,
  onClick,
  danger,
  children,
}: {
  label: string;
  onClick: () => void;
  danger?: boolean;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      tabIndex={-1}
      onClick={onClick}
      className={cx(
        'grid h-9 w-[46px] cursor-default place-items-center text-muted transition-colors duration-100',
        danger ? 'hover:bg-danger hover:text-white' : 'hover:bg-fg/[0.08] hover:text-fg',
      )}
    >
      {children}
    </button>
  );
}
