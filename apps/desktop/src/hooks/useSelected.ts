import { useEffect, useState } from 'react';
import { useApp } from '../store/app';
import type { Device } from '../lib/types';

const QUERY = '(min-width: 720px)';

/** True when the window is wide enough for the two-pane layout. */
export function useIsWide(): boolean {
  const [wide, setWide] = useState(() => window.matchMedia(QUERY).matches);
  useEffect(() => {
    const mq = window.matchMedia(QUERY);
    const on = () => setWide(mq.matches);
    on();
    mq.addEventListener?.('change', on);
    return () => mq.removeEventListener?.('change', on);
  }, []);
  return wide;
}

/** The device shown in the detail pane. Wide: falls back to the first device. Narrow: only what was tapped. */
export function useSelectedDevice(): Device | undefined {
  const wide = useIsWide();
  const devices = useApp((s) => s.devices);
  const selectedId = useApp((s) => s.selectedId);
  return devices.find((d) => d.id === selectedId) ?? (wide ? devices[0] : undefined);
}
