import { useState } from 'react';
import { toast } from 'sonner';
import { AlertTriangle, CheckCircle2, Type } from 'lucide-react';
import { api, errorMessage } from '../lib/api';
import { formatBytes, groupDigits, plural } from '../lib/format';
import { useApp } from '../store/app';
import type { Offer } from '../lib/types';
import offerFile from '../assets/offer-file.webp';
import { Button, Modal } from './ui';

/** An offer of a single note or link: its content only arrives after Accept, so no file row at all. */
function isTextOnly(offer: Offer): boolean {
  return offer.files.length > 0 && offer.files.every((f) => f.kind === 'text');
}

/** Text items are named "Text", never by their internal file name (the content only arrives after Accept). */
function offerTitle(offer: Offer): string {
  const texts = offer.files.filter((f) => f.kind === 'text');
  if (texts.length === offer.fileCount) return texts.length === 1 ? 'Text' : plural(texts.length, 'text');
  if (texts.length === 0) {
    return offer.fileCount > 1 ? `${offer.fileCount} files` : (offer.files[0]?.path.split('/').pop() ?? '1 file');
  }
  const files = offer.fileCount - texts.length;
  return `${plural(files, 'file')} and ${texts.length > 1 ? plural(texts.length, 'text') : 'text'}`;
}

/** Shown for the oldest pending incoming offer. Declining is the default for unknown devices. */
export function IncomingOfferDialog() {
  const offer = useApp((s) => s.offers[0]);
  const resolve = useApp((s) => s.resolveOffer);
  const [busy, setBusy] = useState(false);
  const [dest, setDest] = useState<string | null>(null);

  const respond = async (accept: boolean) => {
    if (!offer) return;
    setBusy(true);
    try {
      await api.respondOffer(offer.id, accept, accept && dest ? dest : undefined);
    } catch (e) {
      toast.error(errorMessage(e));
    } finally {
      resolve(offer.id);
      setBusy(false);
      setDest(null);
    }
  };

  const notEnoughSpace = !!offer && offer.freeSpace != null && offer.freeSpace < offer.totalBytes;
  const folder = dest ?? offer?.downloadDir ?? '';

  return (
    <Modal
      open={!!offer}
      title="Incoming transfer"
      hideTitle
      onClose={() => void respond(false)}
      initialFocus="[data-autofocus]"
      footer={
        <>
          <Button
            variant="dangerSolid"
            className="rounded-full! sm:flex-1"
            onClick={() => void respond(false)}
            disabled={busy}
            data-autofocus={offer && !offer.trusted ? '' : undefined}
          >
            Decline
          </Button>
          <Button
            variant="primary"
            className="rounded-full! sm:flex-1"
            onClick={() => void respond(true)}
            disabled={busy || notEnoughSpace}
            data-autofocus={offer?.trusted ? '' : undefined}
          >
            Accept
          </Button>
        </>
      }
    >
      {offer && isTextOnly(offer) && (
        <div className="space-y-3">
          <p className="flex items-center justify-center gap-2 text-[17px] font-semibold tracking-tight">
            <Type className="h-5 w-5 text-accent" strokeWidth={1.75} aria-hidden="true" /> Text from {offer.peerName}
          </p>
          <div
            data-testid="offer-text-card"
            className="flex min-h-[9rem] flex-col items-center justify-center gap-1 rounded-card bg-elevated p-4 text-center"
          >
            <p className="text-muted">Accept to see the text.</p>
            <p className="text-[13px]">
              {offer.trusted ? <span className="text-muted">Paired device</span> : <span className="text-warn">Not paired</span>}
            </p>
          </div>
        </div>
      )}
      {offer && !isTextOnly(offer) && (
        <div className="flex flex-col items-center gap-1.5 text-center">
          <p className="text-[17px] font-semibold tracking-tight">From {offer.peerName}</p>
          <img src={offerFile} alt="" width={104} height={104} className="my-1.5 h-[104px] w-[104px] select-none" draggable={false} />
          <p className="max-w-full truncate text-[17px] font-semibold tracking-tight" title={offer.files[0]?.path}>
            {offerTitle(offer)}
          </p>
          <p className="text-muted">
            <>{formatBytes(offer.totalBytes)} · </>
            {offer.trusted ? (
              <span>Paired device</span>
            ) : (
              <span className="text-warn">Not paired</span>
            )}
          </p>
          {offer.fileCount > 1 && (
            <ul aria-label="Files offered" className="mt-1 w-full space-y-0.5 text-[13px] text-muted">
              {offer.files.slice(0, 3).map((f) => (
                <li key={f.path} className="truncate" title={f.kind === 'text' ? undefined : f.path}>
                  {f.kind === 'text' ? 'Text' : f.path}
                </li>
              ))}
              {offer.fileCount > 3 && <li className="text-faint">…and {offer.fileCount - 3} more</li>}
            </ul>
          )}
          {notEnoughSpace && (
            <p role="alert" className="mt-1 flex items-center gap-2 text-[13px] text-bad">
              <AlertTriangle className="h-4 w-4" aria-hidden="true" />
              Not enough free space ({formatBytes(offer.freeSpace ?? 0)} available).
            </p>
          )}
          {!isTextOnly(offer) && (
            <div className="mt-2 flex w-full items-center justify-between gap-3 text-[13px]">
              <p className="min-w-0 truncate text-left text-muted" title={folder}>
                Save to <span className="text-fg">{folder}</span>
              </p>
              <Button
                size="sm"
                variant="ghost"
                onClick={async () => {
                  const d = await api.pickFolder().catch(() => null);
                  if (d) setDest(d);
                }}
              >
                Change…
              </Button>
            </div>
          )}
        </div>
      )}
    </Modal>
  );
}

/**
 * "Sent successfully" card. It stays until Done is clicked, but it is low priority: the store clears it
 * the moment an offer, pairing request or received text arrives, so it never hides a newer dialog.
 */
export function SentDialog() {
  const notice = useApp((s) => s.sentNotice);
  const dismiss = useApp((s) => s.dismissSentNotice);
  const view = useApp((s) => s.view);
  const sendingId = useApp((s) => s.sendingId);
  const navigate = useApp((s) => s.navigate);
  const setSendingId = useApp((s) => s.setSendingId);

  const done = () => {
    dismiss();
    // The "Sending to ..." screen has nothing left to show once the send is acknowledged.
    if (view === 'sending' && sendingId === notice?.transferId) {
      setSendingId(null);
      navigate('home');
    }
  };

  return (
    <Modal
      open={!!notice}
      title="Sent successfully"
      hideTitle
      onClose={done}
      initialFocus="[data-autofocus]"
      width="max-w-sm"
      footer={
        <Button variant="primary" className="rounded-full! sm:flex-1" data-autofocus onClick={done}>
          Done
        </Button>
      }
    >
      {notice && (
        <div className="flex flex-col items-center gap-2 pb-1 text-center">
          <CheckCircle2 className="h-14 w-14 text-ok" strokeWidth={1.75} aria-hidden="true" />
          <p className="text-[17px] font-semibold tracking-tight">Sent successfully</p>
          <p className="max-w-full truncate text-muted">
            {notice.label} to {notice.peerName}
          </p>
        </div>
      )}
    </Modal>
  );
}

/** Incoming pairing request: both devices must show the same six digits. */
export function PairRequestDialog() {
  const req = useApp((s) => s.pairRequests[0]);
  const resolve = useApp((s) => s.resolvePairRequest);

  const respond = async (approve: boolean) => {
    if (!req) return;
    try {
      await api.respondPair(req.id, approve);
      if (approve) toast.success(`Paired with ${req.peerName}`);
    } catch (e) {
      toast.error(errorMessage(e));
    } finally {
      resolve(req.id);
    }
  };

  return (
    <Modal
      open={!!req}
      title="Pair with this device?"
      onClose={() => void respond(false)}
      initialFocus="[data-autofocus]"
      footer={
        <>
          <Button data-autofocus onClick={() => void respond(false)}>
            Codes don’t match
          </Button>
          <Button variant="primary" onClick={() => void respond(true)}>
            Codes match — pair
          </Button>
        </>
      }
    >
      {req && (
        <div className="space-y-4 text-center">
          <p>
            <strong>{req.peerName}</strong> <span className="font-mono text-xs text-muted">({req.peerShortId})</span>{' '}
            wants to pair. Check that its screen shows the same code:
          </p>
          <p
            aria-label={`Verification code ${req.code.replace(/\D/g, '').split('').join(' ')}`}
            className="font-mono text-5xl font-medium tracking-[0.18em]"
          >
            {groupDigits(req.code)}
          </p>
          <p className="text-[13px] text-muted">
            If the codes differ, someone may be trying to impersonate a device. Choose “Codes don’t match”.
          </p>
        </div>
      )}
    </Modal>
  );
}

