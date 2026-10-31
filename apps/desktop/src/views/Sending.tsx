import { useState } from 'react';
import { toast } from 'sonner';
import { Check } from 'lucide-react';
import { api, errorMessage } from '../lib/api';
import { percent } from '../lib/format';
import { textInfo } from '../lib/text';
import { useApp } from '../store/app';
import { Button } from '../components/ui';
import { ACTIVE_STATES, ProgressRing, transferSubtitle } from '../components/ProgressRing';

/** Shown right after Send: progress ring, current file, speed / ETA and Cancel. */
export function SendingView() {
  const id = useApp((s) => s.sendingId);
  const t = useApp((s) => s.transfers.find((x) => x.id === s.sendingId));
  const navigate = useApp((s) => s.navigate);
  const setSendingId = useApp((s) => s.setSendingId);
  const [cancelling, setCancelling] = useState(false);

  const close = () => {
    setSendingId(null);
    navigate('home');
  };

  if (!id || !t) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-4 text-center">
        <p className="text-muted">Nothing is being sent.</p>
        <Button onClick={close}>Back</Button>
      </div>
    );
  }

  const running = ACTIVE_STATES.includes(t.state);
  const done = t.state === 'completed';
  const pct = done ? 100 : percent(t.bytesDone, t.bytesTotal);

  const cancel = async () => {
    setCancelling(true);
    try {
      await api.cancelTransfer(t.id);
    } catch (e) {
      toast.error(errorMessage(e));
      setCancelling(false);
    }
  };

  const retry = async () => {
    try {
      await api.retryTransfer(t.id);
      setCancelling(false);
    } catch (e) {
      toast.error(errorMessage(e));
    }
  };

  const text = textInfo(t);
  return (
    <div className="flex flex-1 flex-col">
      <h1 className="text-[28px] leading-[1.15] font-bold tracking-tight">
        {done ? 'Sent to ' : 'Sending to '}
        <span className="text-accent">{t.peerName || 'device'}</span>
      </h1>

      <section className="flex flex-1 flex-col items-center justify-center gap-3 py-6 text-center" aria-live="polite">
        <ProgressRing pct={pct} done={done} failed={!running && !done} waiting={t.state === 'connecting' || t.state === 'awaiting_approval'} label="Sending progress">
          {done ? (
            <Check className="h-14 w-14 text-ok" strokeWidth={2.25} aria-hidden="true" />
          ) : t.state === 'connecting' || t.state === 'awaiting_approval' ? null : (
            <span className="text-[32px] font-semibold tabular-nums">{Math.round(pct)}%</span>
          )}
        </ProgressRing>
        <div className="mt-2 min-w-0 max-w-full">
          <p className="truncate text-[17px] font-semibold tracking-tight" title={text ? text.preview || undefined : (t.currentFile ?? t.title)}>
            {text ? (
              <>
                {text.label}
                {text.preview && <span className="font-normal text-muted"> · {text.preview}</span>}
              </>
            ) : (
              (running && t.currentFile) || t.title || 'Files'
            )}
          </p>
          <p className="t-caption mt-1 tabular-nums">{transferSubtitle(t, running, done)}</p>
          {t.error && !running && (
            <p role="alert" className="mt-2 text-[13px] text-bad">
              {t.error}
            </p>
          )}
        </div>
      </section>

      <div className="mt-auto flex justify-center gap-2 pb-1">
        {running ? (
          <Button
            variant="dangerSolid"
            className="min-h-[46px] w-full max-w-[320px] !rounded-full"
            disabled={cancelling}
            onClick={() => void cancel()}
          >
            {cancelling ? 'Cancelling…' : 'Cancel'}
          </Button>
        ) : (
          <>
            {t.canRetry && (
              <Button className="min-h-[46px] w-[190px] !rounded-full" onClick={() => void retry()}>
                Retry
              </Button>
            )}
            <Button variant="primary" className="min-h-[46px] w-[190px] !rounded-full" onClick={close}>
              {done ? 'Done' : 'Close'}
            </Button>
          </>
        )}
      </div>
    </div>
  );
}
