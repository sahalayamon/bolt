import { useEffect, useState } from 'react';
import { toast } from 'sonner';
import { FilePlus2, FolderPlus, Globe, Type } from 'lucide-react';
import offerFile from '../assets/offer-file.webp';
import { api, errorMessage } from '../lib/api';
import { formatBytes, plural } from '../lib/format';
import { shortPreview } from '../lib/text';
import { useApp } from '../store/app';
import { Button, DeviceIcon, EmptyState, cx } from '../components/ui';
import { TextComposer } from '../components/TextDialogs';
import type { ScanSummary } from '../lib/types';

const IP_RE = /^(\d{1,3}(\.\d{1,3}){3}|\[[0-9a-fA-F:]+\]|[0-9a-fA-F:]+)(:\d{1,5})?$/;
const MANUAL = '__manual__';

/** Files as tiles (plus add buttons), then Cancel / Send pinned bottom-right. The device list stays visible. */
export function SendReviewView() {
  const draft = useApp((s) => s.draft);
  const setDraft = useApp((s) => s.setDraft);
  const navigate = useApp((s) => s.navigate);
  const devices = useApp((s) => s.devices);
  const startDraft = useApp((s) => s.startDraft);
  const setSendingId = useApp((s) => s.setSendingId);

  const [scan, setScan] = useState<ScanSummary | null>(null);
  const [scanError, setScanError] = useState<string | null>(null);
  const [target, setTarget] = useState<string>(draft?.deviceId ?? '');
  const [address, setAddress] = useState('');
  const [sending, setSending] = useState(false);
  const [editing, setEditing] = useState(false);

  const paths = draft?.paths;
  const text = draft?.text ?? '';
  const hasText = text.trim() !== '';
  // "Send text or link" from the home screen arrives with an empty text: open the editor straight away.
  useEffect(() => {
    if (draft?.text === '') setEditing(true);
  }, [draft]);
  useEffect(() => {
    if (!paths) return;
    let live = true;
    setScanError(null);
    if (paths.length === 0) {
      setScan({ fileCount: 0, totalBytes: 0, files: [] });
      return;
    }
    setScan(null);
    api
      .scanPaths(paths)
      .then((s) => live && setScan(s))
      .catch((e) => live && setScanError(errorMessage(e)));
    return () => {
      live = false;
    };
  }, [paths]);

  // The draft's device can disappear from the network; never send to a stale address.
  const fixedDevice = draft?.deviceId ? devices.find((d) => d.id === draft.deviceId) : undefined;
  const targetGone = target !== '' && target !== MANUAL && !devices.some((d) => d.id === target);
  useEffect(() => {
    if (targetGone) setTarget('');
  }, [targetGone]);

  if (!draft) {
    return (
      <div>
        <h1 className="t-title mb-7">Review &amp; send</h1>
        <EmptyState icon={<FilePlus2 />} title="Nothing selected">
          Choose files or drop them on the window to start.
        </EmptyState>
      </div>
    );
  }

  const manual = !fixedDevice && target === MANUAL;
  const addressOk = IP_RE.test(address.trim());
  const recipientOk = fixedDevice ? true : manual ? addressOk : target !== '' && !targetGone;
  const canSend = !sending && !!scan && (scan.fileCount > 0 || hasText) && recipientOk;

  const addMore = async (kind: 'files' | 'folder') => {
    try {
      const more = kind === 'files' ? await api.pickFiles() : [await api.pickFolder()].filter((x): x is string => !!x);
      if (more.length) startDraft([...new Set([...draft.paths, ...more])], draft.deviceId);
    } catch (e) {
      toast.error(errorMessage(e));
    }
  };

  const send = async () => {
    setSending(true);
    try {
      const to = fixedDevice ? { deviceId: fixedDevice.id } : manual ? { address: address.trim() } : { deviceId: target };
      const started = hasText ? await api.startSend(draft.paths, to, text) : await api.startSend(draft.paths, to);
      setDraft(null);
      setSendingId(started.id);
      navigate('sending');
    } catch (e) {
      toast.error(errorMessage(e));
    } finally {
      setSending(false);
    }
  };

  const cancel = () => {
    setDraft(null);
    navigate('home');
  };

  return (
    <div className="flex flex-1 flex-col">
      <h1 className="text-[28px] leading-[1.15] font-bold tracking-tight">
        Send to{' '}
        <span className="text-accent">
          {fixedDevice?.name ?? (manual ? (addressOk ? address.trim() : '\u2026') : (devices.find((d) => d.id === target)?.name ?? '\u2026'))}
        </span>
      </h1>

      <section aria-label="Files to send" className="flex flex-1 flex-col items-center justify-center gap-1.5 py-6 text-center">
        <img
          src={offerFile}
          alt=""
          width={fixedDevice ? 144 : 104}
          height={fixedDevice ? 144 : 104}
          className={cx('select-none', fixedDevice ? 'mb-3 h-[144px] w-[144px]' : 'mb-1 h-[104px] w-[104px]')}
          draggable={false}
        />
        {scanError ? (
          <p role="alert" className="text-bad">
            {scanError}
          </p>
        ) : !scan ? (
          <p className="text-muted">Reading selection…</p>
        ) : (
          <>
            {scan.fileCount > 0 && (
              <>
                <p className="text-[18px] font-semibold tracking-tight" aria-live="polite">
                  {plural(scan.fileCount, 'file')} selected
                </p>
                <p className="text-[16px] text-muted">{formatBytes(scan.totalBytes)}</p>
              </>
            )}
            {hasText && (
              <div className="mt-1 flex w-full max-w-sm items-center gap-2 rounded-card bg-surface px-3 py-2 text-left ring-1 ring-line">
                <Type className="h-4 w-4 shrink-0 text-muted" strokeWidth={1.75} aria-hidden="true" />
                <p className="min-w-0 flex-1 truncate text-[13px]" title={shortPreview(text, 200)}>
                  <span className="font-medium">Text</span> <span className="text-muted">· {shortPreview(text, 60)}</span>
                </p>
                <Button size="sm" variant="ghost" onClick={() => setEditing(true)}>
                  Edit
                </Button>
                <Button size="sm" variant="ghost" aria-label="Remove text" onClick={() => setDraft({ ...draft, text: undefined })}>
                  Remove
                </Button>
              </div>
            )}
            <ul aria-label="Files to send" className={cx('mt-2 w-full max-w-xs space-y-0.5 text-[12px] text-muted', !fixedDevice && 'sr-only')}>
              {scan.files.slice(0, 3).map((f) => (
                <li key={f.name} className="truncate" title={f.name}>
                  {f.name}
                </li>
              ))}
              {scan.fileCount > 3 && <li className="text-faint">…and {scan.fileCount - 3} more</li>}
            </ul>
            <div className={cx('flex items-center gap-1', fixedDevice ? 'mt-3' : 'mt-1')}>
              <Button size="sm" variant="ghost" onClick={() => void addMore('files')}>
                <FilePlus2 className="h-4 w-4" strokeWidth={1.75} aria-hidden="true" /> Add files
              </Button>
              <Button size="sm" variant="ghost" onClick={() => void addMore('folder')}>
                <FolderPlus className="h-4 w-4" strokeWidth={1.75} aria-hidden="true" /> Add folder
              </Button>
              {!hasText && (
                <Button size="sm" variant="ghost" onClick={() => setEditing(true)}>
                  <Type className="h-4 w-4" strokeWidth={1.75} aria-hidden="true" /> Add text or link
                </Button>
              )}
            </div>
            {!fixedDevice && (
              <div className="mt-3 flex w-full max-w-sm flex-col items-center gap-2">
                <div role="radiogroup" aria-label="Send to" className="flex flex-wrap justify-center gap-1.5">
                  {devices.map((d) => (
                    <label
                      key={d.id}
                      className={cx(
                        'flex cursor-pointer items-center gap-1.5 rounded-full px-3 py-1.5 text-[13px] transition-colors duration-150',
                        target === d.id ? 'bg-fg/15 text-fg' : 'bg-elevated text-muted hover:text-fg',
                      )}
                    >
                      <input type="radio" name="target" className="sr-only" checked={target === d.id} onChange={() => setTarget(d.id)} />
                      <DeviceIcon kind={d.kind} className="h-3.5 w-3.5" />
                      {d.name}
                    </label>
                  ))}
                  <label
                    className={cx(
                      'flex cursor-pointer items-center gap-1.5 rounded-full px-3 py-1.5 text-[13px] transition-colors duration-150',
                      manual ? 'bg-fg/15 text-fg' : 'bg-elevated text-muted hover:text-fg',
                    )}
                  >
                    <input type="radio" name="target" className="sr-only" checked={manual} onChange={() => setTarget(MANUAL)} />
                    <Globe className="h-3.5 w-3.5" strokeWidth={1.75} aria-hidden="true" />
                    IP address
                  </label>
                </div>
                {manual && (
                  <div className="w-full">
                    <input
                      aria-label="IP address"
                      autoFocus
                      value={address}
                      onChange={(e) => setAddress(e.target.value)}
                      placeholder="192.168.1.20 or 192.168.1.20:47600"
                      aria-invalid={!!address && !addressOk}
                      className="field text-center font-mono text-[13px]"
                    />
                    {address && !addressOk && (
                      <p role="alert" className="mt-1 text-xs text-bad">
                        Enter an IPv4/IPv6 address, optionally with :port.
                      </p>
                    )}
                  </div>
                )}
                {devices.length === 0 && !manual && <p className="t-caption">No devices found. Choose IP address to enter one.</p>}
              </div>
            )}
          </>
        )}
      </section>

      <div className="sticky bottom-0 -mx-5 mt-auto flex justify-center gap-2 bg-gradient-to-t from-bg from-70% to-transparent px-5 pt-8 pb-1 min-[720px]:-mx-10 min-[720px]:px-10">
        <Button
          variant="dangerSolid"
          className="min-h-[38px] w-[190px] !rounded-full"
          onClick={cancel}
        >
          Cancel
        </Button>
        <Button variant="primary" className="min-h-[38px] w-[190px] !rounded-full" disabled={!canSend} onClick={() => void send()}>
          {sending ? 'Starting…' : 'Send'}
        </Button>
      </div>
      <TextComposer
        open={editing}
        initial={text}
        onSave={(t) => {
          setDraft({ ...draft, text: t });
          setEditing(false);
        }}
        onClose={() => {
          setEditing(false);
          if (draft.text === '') setDraft({ ...draft, text: undefined });
        }}
      />
    </div>
  );
}

