import { useEffect, useState } from 'react';
import { toast } from 'sonner';
import { ArrowDownToLine, ArrowUpFromLine, History as HistoryIcon, Search, Trash2 } from 'lucide-react';
import { api, errorMessage } from '../lib/api';
import { formatBytes, formatWhen, plural } from '../lib/format';
import { useApp } from '../store/app';
import { Badge, Button, Card, EmptyState, IconTile, Modal, PageHeader, Segmented } from '../components/ui';
import type { HistoryEntry, HistoryStatus } from '../lib/types';

type DirFilter = 'all' | 'sent' | 'received';
type StatusFilter = 'all' | HistoryStatus;

const STATUS_TONE: Record<HistoryStatus, 'ok' | 'bad' | 'warn' | 'neutral'> = {
  completed: 'ok',
  failed: 'bad',
  cancelled: 'neutral',
  rejected: 'warn',
};

export function HistoryView() {
  const version = useApp((s) => s.historyVersion);
  const [text, setText] = useState('');
  const [query, setQuery] = useState('');
  const [dir, setDir] = useState<DirFilter>('all');
  const [status, setStatus] = useState<StatusFilter>('all');
  const [rows, setRows] = useState<HistoryEntry[] | null>(null);
  const [confirm, setConfirm] = useState(false);

  // Debounce typing; the search itself runs in SQLite.
  useEffect(() => {
    const t = setTimeout(() => setQuery(text), 200);
    return () => clearTimeout(t);
  }, [text]);

  useEffect(() => {
    let live = true;
    api
      .historyList({
        text: query || undefined,
        direction: dir === 'all' ? undefined : dir,
        status: status === 'all' ? undefined : status,
      })
      .then((r) => live && setRows(r))
      .catch((e) => {
        if (live) {
          setRows([]);
          toast.error(errorMessage(e));
        }
      });
    return () => {
      live = false;
    };
  }, [query, dir, status, version]);

  const filtering = query !== '' || dir !== 'all' || status !== 'all';

  const clear = async () => {
    try {
      const n = await api.historyClear();
      toast.success(`Cleared ${plural(n, 'entry', 'entries')}`);
    } catch (e) {
      toast.error(errorMessage(e));
    } finally {
      setConfirm(false);
    }
  };

  return (
    <div>
      <PageHeader
        title="History"
        subtitle="Only names, sizes and results are kept — never file contents or full paths."
        actions={
          <Button variant="danger" disabled={!rows?.length && !filtering} onClick={() => setConfirm(true)}>
            <Trash2 className="h-4 w-4" strokeWidth={1.75} aria-hidden="true" /> Clear history
          </Button>
        }
      />

      <div className="mb-5 flex flex-wrap items-center gap-x-3 gap-y-2.5">
        <label className="relative min-w-52 flex-1">
          <span className="sr-only">Search history</span>
          <Search className="pointer-events-none absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2 text-faint" strokeWidth={1.75} aria-hidden="true" />
          <input
            type="search"
            value={text}
            onChange={(e) => setText(e.target.value)}
            placeholder="Search by file or device name"
            className="field pl-9"
          />
        </label>
        <Segmented
          label="Direction"
          value={dir}
          onChange={setDir}
          options={[
            ['all', 'All'],
            ['sent', 'Sent'],
            ['received', 'Received'],
          ]}
        />
        <Segmented
          label="Status"
          value={status}
          onChange={setStatus}
          options={[
            ['all', 'Any'],
            ['completed', 'Completed'],
            ['failed', 'Failed'],
            ['cancelled', 'Cancelled'],
            ['rejected', 'Declined'],
          ]}
        />
      </div>

      {rows === null ? (
        <p className="text-muted">Loading…</p>
      ) : rows.length === 0 ? (
        <EmptyState
          icon={<HistoryIcon className="h-8 w-8" />}
          title={filtering ? 'No matching transfers' : 'No history yet'}
        >
          {filtering ? 'Try a different search or clear the filters.' : 'Completed, failed and cancelled transfers are listed here.'}
        </EmptyState>
      ) : (
        <Card>
          <ul aria-label="Transfer history" className="divide-y divide-line">
            {rows.map((r) => (
              <li key={r.transferId} className="flex items-center gap-3 px-4 py-3 max-sm:flex-wrap" data-testid="history-row">
                <IconTile>
                  {r.direction === 'sent' ? (
                    <ArrowUpFromLine className="h-[17px] w-[17px]" strokeWidth={1.75} aria-label="Sent" />
                  ) : (
                    <ArrowDownToLine className="h-[17px] w-[17px]" strokeWidth={1.75} aria-label="Received" />
                  )}
                </IconTile>
                <div className="min-w-0 flex-1">
                  <p className="truncate font-medium" title={r.title}>
                    {r.title || 'Files'}
                    {r.fileCount > 1 && <span className="text-muted"> +{r.fileCount - 1} more</span>}
                  </p>
                  <p className="t-caption mt-0.5 truncate">
                    {r.direction === 'sent' ? 'To' : 'From'} {r.peerName} · {formatBytes(r.totalBytes)} ·{' '}
                    {formatWhen(r.finishedUnix)}
                  </p>
                  {r.error && r.status !== 'completed' && (
                    <p className="mt-0.5 truncate text-xs text-bad" title={r.error}>
                      {r.error}
                    </p>
                  )}
                </div>
                {r.direction === 'received' && r.status === 'completed' && (
                  <Button
                    size="sm"
                    variant="ghost"
                    aria-label={`Open ${r.title || 'file'}`}
                    onClick={() => void api.openReceived(r.transferId).catch((e) => toast.error(errorMessage(e)))}
                  >
                    Open
                  </Button>
                )}
                <Badge tone={STATUS_TONE[r.status]}>{r.status === 'rejected' ? 'declined' : r.status}</Badge>
              </li>
            ))}
          </ul>
        </Card>
      )}

      <Modal
        open={confirm}
        title="Clear all history?"
        onClose={() => setConfirm(false)}
        initialFocus="[data-autofocus]"
        footer={
          <>
            <Button data-autofocus onClick={() => setConfirm(false)}>
              Keep history
            </Button>
            <Button variant="danger" onClick={() => void clear()}>
              Clear history
            </Button>
          </>
        }
      >
        <p className="text-muted">
          This removes the list of past transfers from this device. Received files and paired devices are not affected.
        </p>
      </Modal>
    </div>
  );
}

