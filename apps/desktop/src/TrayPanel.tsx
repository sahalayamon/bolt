import { useEffect, useMemo } from 'react';
import { toast, Toaster } from 'sonner';
import {
  ArrowLeftRight,
  FilePlus2,
  FolderPlus,
  History,
  MonitorSmartphone,
  Power,
  Settings,
} from 'lucide-react';
import { api, errorMessage } from './lib/api';
import { textInfo } from './lib/text';
import { formatSpeed, percent } from './lib/format';
import { useAppearance } from './hooks/useAppearance';
import { useApp } from './store/app';
import { ACTIVE_STATES } from './components/ProgressRing';
import { DeviceAvatar } from './components/DeviceAvatar';
import boltIcon from './assets/bolt-icon.png';

/** The small dashboard that opens when you click the tray icon. */
export default function TrayPanel() {
  const init = useApp((s) => s.init);
  const info = useApp((s) => s.info);
  const devices = useApp((s) => s.devices);
  const transfers = useApp((s) => s.transfers);
  const active = useMemo(() => transfers.filter((t) => ACTIVE_STATES.includes(t.state)), [transfers]);
  useAppearance();

  useEffect(() => {
    let off: (() => void) | undefined;
    let cancelled = false;
    void init().then((u) => (cancelled ? u() : (off = u)));
    return () => {
      cancelled = true;
      off?.();
    };
  }, [init]);

  // Esc closes the panel.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && void api.hideTrayPanel();
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  const run = (fn: () => Promise<unknown>) => void fn().catch((e) => toast.error(errorMessage(e)));

  const pick = (kind: 'files' | 'folder', deviceId?: string) =>
    run(async () => {
      const paths = kind === 'files' ? await api.pickFiles() : [await api.pickFolder()].filter((x): x is string => !!x);
      if (paths.length) await api.draftInMain(paths, deviceId);
    });

  return (
    <div className="flex h-full flex-col gap-3 bg-bg p-3.5 text-[13px]">
      <header className="flex items-center gap-2.5 px-1 pt-0.5">
        <img src={boltIcon} alt="" width={26} height={26} className="h-[26px] w-[26px]" draggable={false} />
        <div className="min-w-0 flex-1">
          <p className="text-[15px] leading-tight font-semibold tracking-tight text-accent">Bolt</p>
          <p className="t-caption flex items-center gap-1.5 truncate">
            <span
              className={`h-1.5 w-1.5 shrink-0 rounded-full ${info?.discoveryActive ? 'bg-ok' : 'bg-warn'}`}
              aria-hidden="true"
            />
            {info ? `${info.deviceName} · ${info.discoveryActive ? 'visible' : 'not discoverable'}` : 'Starting…'}
          </p>
        </div>
      </header>

      <button
        type="button"
        onClick={() => run(() => api.openMain())}
        className="min-h-[38px] w-full cursor-pointer rounded-full bg-accent font-medium text-accent-fg transition duration-150 hover:brightness-110 active:scale-[0.98]"
      >
        Open Bolt
      </button>

      <div className="grid grid-cols-2 gap-2">
        <PanelButton icon={<FilePlus2 />} label="Send files" onClick={() => pick('files')} />
        <PanelButton icon={<FolderPlus />} label="Send folder" onClick={() => pick('folder')} />
      </div>

      {active.length > 0 && (
        <section aria-label="Active transfers" className="space-y-1.5">
          <h2 className="t-overline px-1">Active</h2>
          {active.slice(0, 3).map((t) => (
            <button
              key={t.id}
              type="button"
              onClick={() => run(() => api.openMain('transfers'))}
              className="w-full cursor-pointer rounded-control bg-surface p-2.5 text-left ring-1 ring-line transition-colors hover:bg-elevated"
            >
              <div className="flex items-center justify-between gap-2">
                <span className="truncate font-medium">{(() => {
                    const x = textInfo(t);
                    return x ? x.label : t.title || (t.direction === 'sent' ? 'Sending' : 'Receiving');
                  })()}</span>
                <span className="t-caption shrink-0 tabular-nums">
                  {t.state === 'transferring' ? `${Math.round(percent(t.bytesDone, t.bytesTotal))}% · ${formatSpeed(t.speedBps)}` : 'Starting…'}
                </span>
              </div>
              <div className="mt-1.5 h-1 overflow-hidden rounded-full bg-elevated">
                <div className="h-full rounded-full bg-accent" style={{ width: `${percent(t.bytesDone, t.bytesTotal)}%` }} />
              </div>
            </button>
          ))}
        </section>
      )}

      <section aria-label="Nearby devices" className="flex min-h-0 flex-1 flex-col">
        <h2 className="t-overline px-1 pb-1.5">Send to</h2>
        <ul className="scroll-thin min-h-0 flex-1 space-y-0.5 overflow-y-auto">
          {devices.map((d) => (
            <li key={d.id}>
              <button
                type="button"
                onClick={() => pick('files', d.id)}
                className="flex w-full cursor-pointer items-center gap-2.5 rounded-control px-2 py-1.5 text-left transition-colors hover:bg-fg/[0.06]"
              >
                <DeviceAvatar kind={d.kind} size="sm" />
                <span className="min-w-0 flex-1 truncate text-[14px]">{d.name}</span>
              </button>
            </li>
          ))}
          {devices.length === 0 && <li className="t-caption px-2 py-3">No devices nearby. Open Bolt on another device.</li>}
        </ul>
      </section>

      <footer className="flex items-center justify-between border-t border-line pt-2.5">
        <div className="flex items-center gap-0.5">
          <IconLink label="Transfers" onClick={() => run(() => api.openMain('transfers'))}>
            <ArrowLeftRight />
          </IconLink>
          <IconLink label="History" onClick={() => run(() => api.openMain('history'))}>
            <History />
          </IconLink>
          <IconLink label="Devices" onClick={() => run(() => api.openMain('devices'))}>
            <MonitorSmartphone />
          </IconLink>
          <IconLink label="Settings" onClick={() => run(() => api.openMain('settings'))}>
            <Settings />
          </IconLink>
        </div>
        <button
          type="button"
          onClick={() => run(() => api.quitApp())}
          className="flex cursor-pointer items-center gap-1.5 rounded-control px-2 py-1.5 text-muted transition-colors hover:bg-fg/[0.06] hover:text-danger"
        >
          <Power className="h-4 w-4" strokeWidth={1.75} aria-hidden="true" /> Quit
        </button>
      </footer>
      <Toaster position="bottom-center" theme="dark" />
    </div>
  );
}

function PanelButton({ icon, label, onClick }: { icon: React.ReactNode; label: string; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex min-h-[56px] cursor-pointer flex-col items-center justify-center gap-1 rounded-card bg-surface text-[12px] ring-1 ring-line transition duration-150 hover:bg-elevated active:scale-[0.98] [&>svg]:h-[18px] [&>svg]:w-[18px] [&>svg]:stroke-[1.6]"
    >
      {icon}
      {label}
    </button>
  );
}

function IconLink({ label, onClick, children }: { label: string; onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={onClick}
      className="grid h-8 w-8 cursor-pointer place-items-center rounded-control text-muted transition-colors hover:bg-fg/[0.06] hover:text-fg [&>svg]:h-[17px] [&>svg]:w-[17px] [&>svg]:stroke-[1.75]"
    >
      {children}
    </button>
  );
}
