import type { DeviceKind } from '../lib/types';
import { DeviceIcon, cx } from './ui';

/** Round blue avatar carrying the device-kind icon. `lg` adds the dotted halo used in the detail pane. */
export function DeviceAvatar({ kind, size = 'sm' }: { kind: DeviceKind; size?: 'sm' | 'lg' }) {
  if (size === 'sm') {
    return (
      <span className="grid h-9 w-9 shrink-0 place-items-center rounded-full bg-accent text-accent-fg">
        <DeviceIcon kind={kind} className="h-[18px] w-[18px]" />
      </span>
    );
  }
  return (
    <span className="group relative grid h-[104px] w-[104px] place-items-center">
      <span
        className={cx(
          'absolute -inset-3 rounded-full border-2 border-dotted border-fg/15 transition-colors duration-300',
          'group-hover:border-accent/60 group-hover:animate-[breathe_2.6s_ease-in-out_infinite]',
        )}
        aria-hidden="true"
      />
      <span className="grid h-full w-full place-items-center rounded-full bg-accent text-accent-fg transition-transform duration-300 group-hover:scale-[1.03]">
        <DeviceIcon kind={kind} className="h-12 w-12 opacity-90" />
      </span>
    </span>
  );
}
