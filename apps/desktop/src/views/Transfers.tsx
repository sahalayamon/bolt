import { useState } from 'react';
import { toast } from 'sonner';
import { AnimatePresence, motion } from 'framer-motion';
import {
  ArrowDownToLine,
  ArrowUpFromLine,
  ArrowLeftRight,
  CheckCircle2,
  FolderOpen,
  Link2,
  Type,
  RotateCcw,
  X,
  XCircle,
} from 'lucide-react';
import { api, errorMessage } from '../lib/api';
import { formatBytes, formatEta, formatSpeed, percent, plural } from '../lib/format';
import { textInfo } from '../lib/text';
import { useApp } from '../store/app';
import { Badge, Button, Card, EmptyState, IconTile, PageHeader, ProgressBar, SectionTitle, T_BASE } from '../components/ui';
import type { Transfer, TransferState } from '../lib/types';

const ACTIVE: TransferState[] = ['connecting', 'awaiting_approval', 'transferring'];

const STATE_LABEL: Record<TransferState, string> = {
  connecting: 'Connecting…',
  awaiting_approval: 'Waiting for the other device to accept…',
  transferring: 'Transferring',
  completed: 'Completed — verified',
  failed: 'Failed',
  cancelled: 'Cancelled',
  rejected: 'Declined',
};

async function run(fn: () => Promise<unknown>) {
  try {
    await fn();
  } catch (e) {
    toast.error(errorMessage(e));
  }
}

export function TransfersView() {
  const transfers = useApp((s) => s.transfers);
  const active = transfers.filter((t) => ACTIVE.includes(t.state));
  const done = transfers.filter((t) => !ACTIVE.includes(t.state));
  const dismiss = (id: string) => {
    void run(() => api.dismissTransfer(id));
    useApp.setState((s) => ({ transfers: s.transfers.filter((t) => t.id !== id) }));
  };

  return (
    <div>
      <PageHeader title="Transfers" subtitle="Live progress for sends and receives in this session." />
      {transfers.length === 0 ? (
        <EmptyState icon={<ArrowLeftRight />} title="No transfers yet">
          Files you send or receive will show up here with speed and time remaining. Older transfers are in History.
        </EmptyState>
      ) : (
        <div className="space-y-8">
          <Section title="Active" count={active.length}>
            <AnimatePresence initial={false}>
              {active.map((t) => (
                <TransferRow key={t.id} t={t} onDismiss={dismiss} />
              ))}
            </AnimatePresence>
            {active.length === 0 && <li className="px-1 text-muted">Nothing in progress.</li>}
          </Section>
          {done.length > 0 && (
            <Section title="Finished" count={done.length}>
              <AnimatePresence initial={false}>
                {done.map((t) => (
                  <TransferRow key={t.id} t={t} onDismiss={dismiss} />
                ))}
              </AnimatePresence>
            </Section>
          )}
        </div>
      )}
    </div>
  );
}

function Section({ title, count, children }: { title: string; count: number; children: React.ReactNode }) {
  return (
    <section aria-label={title}>
      <SectionTitle aside={<span className="t-caption tabular-nums">{count}</span>}>{title}</SectionTitle>
      <ul className="space-y-2.5">{children}</ul>
    </section>
  );
}

export function TransferRow({ t, onDismiss }: { t: Transfer; onDismiss: (id: string) => void }) {
  const running = ACTIVE.includes(t.state);
  const [cancelling, setCancelling] = useState(false);
  const pct = percent(t.bytesDone, t.bytesTotal);
  const text = textInfo(t);
  const Icon = text ? (text.label === 'Link' ? Link2 : Type) : t.direction === 'sent' ? ArrowUpFromLine : ArrowDownToLine;
  const name = text ? text.label : t.title;
  const label = `${t.direction === 'sent' ? 'Sending' : 'Receiving'} ${name || 'files'}`;

  return (
    <motion.li
      layout="position"
      initial={{ opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0 }}
      transition={T_BASE}
      data-testid="transfer-row"
    >
      <Card className="p-4">
        <div className="flex items-start gap-3">
          <IconTile tone={running ? 'accent' : 'neutral'}>
            <Icon className="h-[17px] w-[17px]" strokeWidth={1.75} aria-hidden="true" />
          </IconTile>
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              {text ? (
                <p className="truncate font-medium" title={text.preview || undefined}>
                  {text.label}
                  {text.preview && <span className="font-normal text-muted"> · {text.preview}</span>}
                </p>
              ) : (
                <p className="truncate font-medium" title={t.title}>
                  {t.title || (t.direction === 'sent' ? 'Preparing…' : 'Incoming files')}
                </p>
              )}
              {!text && t.filesTotal > 1 && <Badge>{plural(t.filesTotal, 'file')}</Badge>}
            </div>
            <p className="t-caption mt-0.5">
              {t.direction === 'sent' ? 'To' : 'From'} {t.peerName || 'device'} ·{' '}
              <span aria-live="polite">{STATE_LABEL[t.state]}</span>
            </p>
          </div>
          <div className="flex shrink-0 flex-wrap items-center justify-end gap-1">
            {running && (
              <Button
                size="sm"
                variant="dangerSolid"
                disabled={cancelling}
                onClick={() => {
                  // Answer the click at once; the card flips to "Cancelled" when the engine confirms.
                  setCancelling(true);
                  void run(() => api.cancelTransfer(t.id)).finally(() => setTimeout(() => setCancelling(false), 4000));
                }}
              >
                {cancelling ? 'Cancelling…' : 'Cancel'}
              </Button>
            )}
            {t.state === 'completed' && t.direction === 'received' && !text && (
              <Button size="sm" variant="ghost" onClick={() => void run(() => api.openReceived(t.id))}>
                Open
              </Button>
            )}
            {t.state === 'completed' && t.direction === 'received' && !text && (
              <Button size="sm" variant="ghost" onClick={() => void run(() => api.revealTransfer(t.id))}>
                <FolderOpen className="h-4 w-4" strokeWidth={1.75} aria-hidden="true" /> Show folder
              </Button>
            )}
            {!running && t.canRetry && (
              <Button size="sm" onClick={() => void run(() => api.retryTransfer(t.id))}>
                <RotateCcw className="h-3.5 w-3.5" strokeWidth={1.75} aria-hidden="true" /> Retry
              </Button>
            )}
            {!running && (
              <Button
                size="sm"
                variant="ghost"
                aria-label={`Dismiss ${name || 'transfer'}`}
                onClick={() => onDismiss(t.id)}
              >
                <X className="h-4 w-4" strokeWidth={1.75} aria-hidden="true" />
              </Button>
            )}
          </div>
        </div>

        {(running || t.state === 'completed') && (
          <div className="mt-3.5">
            <ProgressBar
              value={t.state === 'completed' ? 100 : pct}
              label={label}
              tone={t.state === 'completed' ? 'ok' : 'accent'}
              indeterminate={running && t.bytesTotal === 0}
            />
            <div className="t-caption mt-2 flex flex-wrap justify-between gap-x-4 tabular-nums">
              <span>
                {formatBytes(t.bytesDone)} of {formatBytes(t.bytesTotal)} · {Math.round(t.state === 'completed' ? 100 : pct)}%
                {!text && t.filesTotal > 1 && ` · file ${Math.min(t.filesDone + 1, t.filesTotal)} of ${t.filesTotal}`}
              </span>
              {t.state === 'transferring' && (
                <span>
                  {formatSpeed(t.speedBps)} · {formatEta(t.etaSecs)} left
                </span>
              )}
            </div>
            {t.state === 'transferring' && t.currentFile && !text && (
              <p className="t-caption mt-1 truncate" title={t.currentFile}>
                {t.currentFile}
              </p>
            )}
          </div>
        )}

        {t.state === 'completed' && (
          <p className="mt-2.5 flex items-center gap-1.5 text-xs text-ok">
            <CheckCircle2 className="h-3.5 w-3.5" strokeWidth={2} aria-hidden="true" />{' '}
            {text ? `${text.label} verified by BLAKE3.` : 'Every file was verified by BLAKE3 before being saved.'}
          </p>
        )}
        {t.error && !running && (
          <p role="alert" className="mt-2.5 flex items-start gap-1.5 text-[13px] text-bad">
            <XCircle className="mt-0.5 h-4 w-4 shrink-0" strokeWidth={1.75} aria-hidden="true" /> {t.error}
          </p>
        )}
      </Card>
    </motion.li>
  );
}
