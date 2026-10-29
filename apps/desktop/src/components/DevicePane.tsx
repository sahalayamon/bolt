import { useMemo, useState } from 'react';
import { toast } from 'sonner';
import { ArrowLeftRight, Globe, History, MonitorSmartphone, RefreshCw, Search, Settings, ShieldCheck, WifiOff } from 'lucide-react';
import type { ComponentType } from 'react';
import { api, errorMessage } from '../lib/api';
import { useSelectedDevice } from '../hooks/useSelected';
import { useApp } from '../store/app';
import type { View } from '../lib/types';
import boltIcon from '../assets/bolt-icon.png';
import { DeviceAvatar } from './DeviceAvatar';
import { cx } from './ui';

const NAV: Array<{ view: View; label: string; icon: ComponentType<{ className?: string; strokeWidth?: number }> }> = [
  { view: 'transfers', label: 'Transfers', icon: ArrowLeftRight },
  { view: 'history', label: 'History', icon: History },
  { view: 'devices', label: 'Devices', icon: MonitorSmartphone },
  { view: 'settings', label: 'Settings', icon: Settings },
];

/** Left pane: title + section icons, search, and the list of devices actually found on the network. */
export function DevicePane({ className }: { className?: string }) {
  const view = useApp((s) => s.view);
  const navigate = useApp((s) => s.navigate);
  const devices = useApp((s) => s.devices);
  const info = useApp((s) => s.info);
  const selectDevice = useApp((s) => s.selectDevice);
  const startDraft = useApp((s) => s.startDraft);
  const active = useApp(
    (s) => s.transfers.filter((t) => ['connecting', 'awaiting_approval', 'transferring'].includes(t.state)).length,
  );
  const selected = useSelectedDevice();
  const [q, setQ] = useState('');
  const [refreshing, setRefreshing] = useState(false);

  const refresh = async () => {
    setRefreshing(true);
    try {
      await api.refreshDevices();
    } catch (e) {
      toast.error(errorMessage(e));
    } finally {
      setTimeout(() => setRefreshing(false), 800);
    }
  };

  const shown = useMemo(() => {
    const needle = q.trim().toLowerCase();
    return needle
      ? devices.filter((d) => d.name.toLowerCase().includes(needle) || d.shortId.toLowerCase().includes(needle))
      : devices;
  }, [devices, q]);

  const sendToAddress = async () => {
    try {
      const paths = await api.pickFiles();
      if (paths.length) startDraft(paths);
    } catch (e) {
      toast.error(errorMessage(e));
    }
  };

  return (
    <nav aria-label="Devices and sections" className={cx('flex min-h-0 flex-col bg-surface', className)}>
      {/* Sits in the title-bar row; only the title and icons take clicks, the gaps stay draggable. */}
      <div className="pointer-events-none relative z-50 flex h-9 items-center justify-between pr-3 pl-4">
        <button
          type="button"
          onClick={() => selectDevice(selected?.id ?? null)}
          className="pointer-events-auto flex cursor-pointer items-center gap-2 rounded-control text-[17px] font-semibold tracking-tight text-accent"
          title="Home (Ctrl+1)"
        >
          <img src={boltIcon} alt="" width={18} height={18} className="h-[18px] w-[18px] select-none" draggable={false} />
          Bolt
        </button>
        <ul className="pointer-events-auto flex items-center gap-0.5">
          {NAV.map(({ view: v, label, icon: Icon }, i) => (
            <li key={v}>
              <button
                type="button"
                onClick={() => navigate(v)}
                aria-label={label}
                aria-current={view === v ? 'page' : undefined}
                title={`${label} (Ctrl+${i + 2})`}
                className={cx(
                  'relative grid h-8 w-8 cursor-pointer place-items-center rounded-control transition-colors duration-150',
                  view === v ? 'bg-fg/[0.08] text-fg' : 'text-muted hover:bg-fg/[0.06] hover:text-fg',
                )}
              >
                <Icon className="h-[18px] w-[18px]" strokeWidth={1.75} />
                {v === 'transfers' && active > 0 && (
                  <span
                    className="absolute top-0.5 right-0.5 min-w-4 rounded-full bg-accent px-1 text-center text-[10px] leading-4 font-semibold text-accent-fg"
                    aria-label={`${active} active`}
                  >
                    {active}
                  </span>
                )}
              </button>
            </li>
          ))}
        </ul>
      </div>

      <div className="flex items-center gap-1.5 px-4 pt-3 pb-3">
        <label className="relative block min-w-0 flex-1">
          <span className="sr-only">Search devices</span>
          <input
            type="search"
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder="Search devices"
            className="field pr-9"
          />
          <Search
            className="pointer-events-none absolute top-1/2 right-3 h-4 w-4 -translate-y-1/2 text-faint"
            strokeWidth={1.75}
            aria-hidden="true"
          />
        </label>
        <button
          type="button"
          onClick={() => void refresh()}
          aria-label="Refresh devices"
          title="Look for devices again"
          className="grid h-9 w-9 shrink-0 cursor-pointer place-items-center rounded-control text-muted transition-colors duration-150 hover:bg-fg/[0.06] hover:text-fg"
        >
          <RefreshCw className={refreshing ? 'h-4 w-4 animate-spin' : 'h-4 w-4'} strokeWidth={1.75} aria-hidden="true" />
        </button>
      </div>

      <ul aria-label="Nearby devices" className="scroll-thin min-h-0 flex-1 space-y-0.5 overflow-y-auto px-2 pb-2">
        {shown.map((d) => {
          const on = view === 'home' && selected?.id === d.id;
          return (
            <li key={d.id}>
              <button
                type="button"
                onClick={() => selectDevice(d.id)}
                aria-pressed={on}
                className={cx(
                  'relative flex w-full cursor-pointer items-center gap-3 rounded-control px-3 py-2 text-left transition-colors duration-150 max-[719px]:py-2.5',
                  on ? 'bg-fg/[0.07]' : 'hover:bg-fg/[0.045]',
                )}
              >
                {on && <span className="absolute top-2.5 bottom-2.5 left-0 w-[3px] rounded-full bg-accent" aria-hidden="true" />}
                <DeviceAvatar kind={d.kind} size="sm" />
                <span className="min-w-0 flex-1 truncate text-[15px]">{d.name}</span>
                {d.trusted && (
                  <ShieldCheck
                    className="h-4 w-4 shrink-0 text-faint"
                    strokeWidth={1.75}
                    aria-label="Paired"
                  />
                )}
              </button>
            </li>
          );
        })}
        {devices.length === 0 && (
          <li className="px-3 py-6 text-center text-[13px] text-muted">
            {info && !info.discoveryActive ? (
              <span role="alert" className="flex flex-col items-center gap-2 text-warn">
                <WifiOff className="h-5 w-5" strokeWidth={1.75} aria-hidden="true" />
                Discovery is unavailable{info.discoveryError ? ` (${info.discoveryError})` : ''}. Use an IP address below.
              </span>
            ) : (
              'No devices found yet. Open Bolt on another device on this Wi-Fi.'
            )}
          </li>
        )}
        {devices.length > 0 && shown.length === 0 && (
          <li className="px-3 py-6 text-center text-[13px] text-muted">No devices match “{q}”.</li>
        )}
        <li>
          <button
            type="button"
            onClick={() => void sendToAddress()}
            className="flex w-full cursor-pointer items-center gap-3 rounded-control px-3 py-2 text-left transition-colors duration-150 hover:bg-fg/[0.045] max-[719px]:py-2.5"
          >
            <span className="grid h-9 w-9 shrink-0 place-items-center rounded-full bg-elevated text-muted">
              <Globe className="h-[18px] w-[18px]" strokeWidth={1.75} aria-hidden="true" />
            </span>
            <span className="min-w-0 flex-1 truncate text-[15px]">Send to an IP address…</span>
          </button>
        </li>
      </ul>
    </nav>
  );
}
