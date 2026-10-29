import { formatBytes, formatEta, formatSpeed } from '../lib/format';
import type { Transfer } from '../lib/types';
import { cx } from './ui';

export const ACTIVE_STATES = ['connecting', 'awaiting_approval', 'transferring'];

/** "File 4 of 8 · 777 KB of 20.1 MB · 2.2 MB/s · 8 s left" (or the state while there are no bytes yet). */
export function transferSubtitle(t: Transfer, running: boolean, done: boolean): string {
  const parts: string[] = [];
  if (t.filesTotal > 1) {
    parts.push(done ? `${t.filesTotal} files` : `File ${Math.min(t.filesDone + 1, t.filesTotal)} of ${t.filesTotal}`);
  }
  if (done) {
    parts.push(formatBytes(t.bytesTotal));
  } else if (t.state === 'awaiting_approval') {
    parts.push(t.direction === 'sent' ? 'Waiting for the other device to accept…' : 'Starting…');
  } else if (t.state === 'connecting') {
    parts.push(t.direction === 'sent' ? 'Connecting…' : 'Starting…');
  } else if (running) {
    parts.push(`${formatBytes(t.bytesDone)} of ${formatBytes(t.bytesTotal)}`);
    if (t.speedBps > 0) parts.push(formatSpeed(t.speedBps));
    if (t.etaSecs != null) parts.push(`${formatEta(t.etaSecs)} left`);
  } else {
    parts.push(t.state === 'cancelled' ? 'Cancelled' : t.state === 'rejected' ? 'Declined' : 'Transfer failed');
  }
  return parts.join(' · ');
}

/** Circular progress with content (percentage / check) in the middle. */
export function ProgressRing({
  pct,
  done,
  failed,
  waiting = false,
  size = 150,
  label,
  children,
}: {
  pct: number;
  done: boolean;
  failed: boolean;
  /** Still connecting / waiting for the other side to accept: spin an arc instead of showing progress. */
  waiting?: boolean;
  size?: number;
  label: string;
  children: React.ReactNode;
}) {
  const stroke = 10;
  const r = (size - stroke) / 2;
  const c = 2 * Math.PI * r;
  return (
    <div
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(pct)}
      className="relative my-1 grid place-items-center"
      style={{ width: size, height: size }}
    >
      <div className={cx(waiting && 'animate-spin')} style={waiting ? { animationDuration: '1.2s', animationTimingFunction: 'linear' } : undefined}>
      <svg width={size} height={size} className="-rotate-90" aria-hidden="true">
        <circle cx={size / 2} cy={size / 2} r={r} fill="none" strokeWidth={stroke} className="stroke-elevated" />
        <circle
          cx={size / 2}
          cy={size / 2}
          r={r}
          fill="none"
          strokeWidth={stroke}
          strokeLinecap="round"
          strokeDasharray={waiting ? `${c * 0.28} ${c}` : c}
          strokeDashoffset={waiting ? 0 : c * (1 - Math.max(pct, 1.5) / 100)}
          className={cx(
            !waiting && 'transition-[stroke-dashoffset] duration-300 ease-out',
            done ? 'stroke-ok' : failed ? 'stroke-bad' : 'stroke-accent',
          )}
        />
      </svg>
      </div>
      <div className="absolute inset-0 grid place-items-center">{children}</div>
    </div>
  );
}
