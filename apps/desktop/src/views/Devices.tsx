import { useEffect, useRef, useState } from 'react';
import { toast } from 'sonner';
import { Check, Link2, Pencil, QrCode, ShieldOff, X, MonitorSmartphone, Copy, ShieldCheck } from 'lucide-react';
import { api, errorMessage } from '../lib/api';
import { formatWhen, groupDigits } from '../lib/format';
import { useApp } from '../store/app';
import { Badge, Button, Card, EmptyState, IconTile, Modal, PageHeader } from '../components/ui';
import type { PairingInfo, TrustedDevice } from '../lib/types';

export function DevicesView() {
  const trusted = useApp((s) => s.trusted);
  const refresh = useApp((s) => s.refreshTrusted);
  const [showPair, setShowPair] = useState(false);
  const [showLink, setShowLink] = useState(false);
  const [revoking, setRevoking] = useState<TrustedDevice | null>(null);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const revoke = async () => {
    if (!revoking) return;
    try {
      await api.revokeTrusted(revoking.id);
      toast.success(`Removed ${revoking.name}`);
    } catch (e) {
      toast.error(errorMessage(e));
    } finally {
      setRevoking(null);
    }
  };

  return (
    <div>
      <PageHeader
        title="Devices"
        subtitle="Paired devices can send you files without a warning that they are unknown."
        actions={
          <>
            <Button onClick={() => setShowLink(true)}>
              <Link2 className="h-4 w-4" strokeWidth={1.75} aria-hidden="true" /> Enter pairing link
            </Button>
            <Button variant="primary" onClick={() => setShowPair(true)}>
              <QrCode className="h-4 w-4" strokeWidth={1.75} aria-hidden="true" /> Pair a device
            </Button>
          </>
        }
      />

      {trusted.length === 0 ? (
        <EmptyState icon={<MonitorSmartphone />} title="No paired devices">
          Pair a device to recognise it in the future. Pairing shows the same 6-digit code on both screens so you can
          confirm you are talking to the right device.
        </EmptyState>
      ) : (
        <Card>
          <ul aria-label="Paired devices" className="divide-y divide-line">
            {trusted.map((d) => (
              <TrustedRow key={d.id} d={d} onRevoke={() => setRevoking(d)} />
            ))}
          </ul>
        </Card>
      )}

      <PairDialog open={showPair} onClose={() => setShowPair(false)} />
      <PairLinkDialog open={showLink} onClose={() => setShowLink(false)} />

      <Modal
        open={!!revoking}
        title="Remove this device?"
        onClose={() => setRevoking(null)}
        initialFocus="[data-autofocus]"
        footer={
          <>
            <Button data-autofocus onClick={() => setRevoking(null)}>
              Keep
            </Button>
            <Button variant="danger" onClick={() => void revoke()}>
              <ShieldOff className="h-4 w-4" aria-hidden="true" /> Remove
            </Button>
          </>
        }
      >
        <p className="text-muted">
          <strong className="text-fg">{revoking?.name}</strong> will no longer be trusted. You can pair again later.
        </p>
      </Modal>
    </div>
  );
}

function TrustedRow({ d, onRevoke }: { d: TrustedDevice; onRevoke: () => void }) {
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState(d.name);
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (editing) input.current?.select();
  }, [editing]);

  const save = async () => {
    const next = name.trim();
    if (next === d.name) return setEditing(false);
    try {
      await api.renameTrusted(d.id, next);
      setEditing(false);
    } catch (e) {
      toast.error(errorMessage(e));
    }
  };

  return (
    <li className="flex items-center gap-3 px-4 py-3 max-sm:flex-wrap" data-testid="trusted-row">
      <IconTile tone="accent">
        <ShieldCheck className="h-[17px] w-[17px]" strokeWidth={1.75} aria-hidden="true" />
      </IconTile>
      <div className="min-w-0 flex-1">
        {editing ? (
          <form
            className="flex items-center gap-1"
            onSubmit={(e) => {
              e.preventDefault();
              void save();
            }}
            onKeyDown={(e) => {
              if (e.key === 'Escape') {
                setName(d.name);
                setEditing(false);
              }
            }}
          >
            <input
              ref={input}
              aria-label={`New name for ${d.name}`}
              value={name}
              maxLength={60}
              onChange={(e) => setName(e.target.value)}
              className="field !min-h-8 w-56 max-w-full"
            />
            <Button size="sm" type="submit" variant="primary" aria-label="Save name">
              <Check className="h-4 w-4" aria-hidden="true" />
            </Button>
            <Button
              size="sm"
              variant="ghost"
              aria-label="Cancel rename"
              onClick={() => {
                setName(d.name);
                setEditing(false);
              }}
            >
              <X className="h-4 w-4" aria-hidden="true" />
            </Button>
          </form>
        ) : (
          <p className="flex items-center gap-2 font-medium">
            <span className="truncate">{d.name}</span>
            {d.online ? <Badge tone="ok">Online</Badge> : <Badge>Offline</Badge>}
          </p>
        )}
        <p className="t-caption mt-0.5">
          <span className="font-mono">{d.shortId}</span> · Paired {formatWhen(d.pairedAt)}
        </p>
      </div>
      {!editing && (
        <div className="flex gap-1 max-sm:w-full max-sm:justify-end max-sm:border-t max-sm:border-line max-sm:pt-2">
          <Button size="sm" variant="ghost" aria-label={`Rename ${d.name}`} onClick={() => setEditing(true)}>
            <Pencil className="h-3.5 w-3.5" strokeWidth={1.75} aria-hidden="true" /> Rename
          </Button>
          <Button size="sm" variant="ghost" aria-label={`Remove ${d.name}`} onClick={onRevoke}>
            <ShieldOff className="h-3.5 w-3.5" strokeWidth={1.75} aria-hidden="true" /> Remove
          </Button>
        </div>
      )}
    </li>
  );
}

/** Show a QR + link for another device to scan/paste; confirmation happens in the global pair-request dialog. */
function PairDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [info, setInfo] = useState<PairingInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [left, setLeft] = useState(0);
  const trustedCount = useApp((s) => s.trusted.length);
  const initial = useRef(trustedCount);

  useEffect(() => {
    if (!open) return;
    initial.current = useApp.getState().trusted.length;
    let live = true;
    setInfo(null);
    setError(null);
    api
      .beginPairing()
      .then((i) => live && setInfo(i))
      .catch((e) => live && setError(errorMessage(e)));
    return () => {
      live = false;
      void api.endPairing().catch(() => {});
    };
  }, [open]);

  useEffect(() => {
    if (!info) return;
    const tick = () => setLeft(Math.max(0, info.expiresUnix - Math.floor(Date.now() / 1000)));
    tick();
    const t = setInterval(tick, 1000);
    return () => clearInterval(t);
  }, [info]);

  // A new trusted device while this dialog is open means pairing succeeded.
  useEffect(() => {
    if (open && trustedCount > initial.current) onClose();
  }, [open, trustedCount, onClose]);

  const copy = async () => {
    if (!info) return;
    try {
      await navigator.clipboard.writeText(info.uri);
      toast.success('Pairing link copied');
    } catch {
      toast.error('Could not copy. Select the link and copy it manually.');
    }
  };

  const mm = String(Math.floor(left / 60));
  const ss = String(left % 60).padStart(2, '0');

  return (
    <Modal open={open} title="Pair a device" onClose={onClose} footer={<Button onClick={onClose}>Done</Button>}>
      {error ? (
        <p role="alert" className="text-bad">
          {error}
        </p>
      ) : !info ? (
        <p className="text-muted">Preparing…</p>
      ) : (
        <div className="space-y-4">
          <p className="text-muted">
            On the other device, scan this code or open <em>Devices → Enter pairing link</em> and paste the link. A
            6-digit code then appears on both screens — approve only if they match.
          </p>
          <div
            className="mx-auto aspect-square w-52 overflow-hidden rounded-[14px] bg-white p-2.5 [&>svg]:block [&>svg]:h-full [&>svg]:w-full"
            role="img"
            aria-label="Pairing QR code"
            dangerouslySetInnerHTML={{ __html: info.qrSvg }}
          />
          <p className="text-center text-[13px] text-muted">
            {left > 0 ? (
              <>
                Valid for {mm}:{ss} · one use only · {info.address}
              </>
            ) : (
              <span className="text-warn">This code expired. Close and reopen to get a new one.</span>
            )}
          </p>
          <div className="flex items-center gap-2">
            <input
              readOnly
              aria-label="Pairing link"
              value={info.uri}
              onFocus={(e) => e.currentTarget.select()}
              className="field min-w-0 flex-1 font-mono text-xs"
            />
            <Button size="sm" onClick={() => void copy()}>
              <Copy className="h-4 w-4" aria-hidden="true" /> Copy
            </Button>
          </div>
        </div>
      )}
    </Modal>
  );
}

/** The initiating side: paste a pairing link, then show the code to compare while the other user approves. */
function PairLinkDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [uri, setUri] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const code = useApp((s) => s.pairCode);
  const setCode = useApp((s) => s.setPairCode);

  useEffect(() => {
    if (open) {
      setUri('');
      setError(null);
      setBusy(false);
      setCode(null);
    }
  }, [open, setCode]);

  const submit = async () => {
    setBusy(true);
    setError(null);
    setCode(null);
    try {
      const peer = await api.pairWithUri(uri.trim());
      toast.success(`Paired with ${peer.name}`);
      onClose();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      open={open}
      title="Enter pairing link"
      onClose={onClose}
      dismissible={!busy}
      footer={
        <>
          <Button onClick={onClose} disabled={busy}>
            Cancel
          </Button>
          <Button variant="primary" disabled={busy || !uri.trim().startsWith('bolt://pair?')} onClick={() => void submit()}>
            {busy ? 'Waiting for approval…' : 'Pair'}
          </Button>
        </>
      }
    >
      <div className="space-y-3">
        <label className="block">
          <span className="mb-1 block text-muted">Paste the link shown on the other device</span>
          <textarea
            value={uri}
            onChange={(e) => setUri(e.target.value)}
            disabled={busy}
            rows={3}
            placeholder="bolt://pair?v=1&id=…"
            className="field font-mono text-xs"
          />
        </label>
        {code && (
          <div className="rounded-card bg-elevated p-4 text-center" aria-live="polite">
            <p className="text-[13px] text-muted">Make sure the other device shows this code, then approve it there:</p>
            <p className="mt-1 font-mono text-4xl font-medium tracking-[0.18em] text-fg">{groupDigits(code)}</p>
          </div>
        )}
        {error && (
          <p role="alert" className="text-bad">
            {error}
          </p>
        )}
      </div>
    </Modal>
  );
}
