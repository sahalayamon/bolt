import { useState } from 'react';
import { toast } from 'sonner';
import { AlertTriangle } from 'lucide-react';
import { api, errorMessage } from '../lib/api';
import { formatBytes, groupDigits } from '../lib/format';
import { useApp } from '../store/app';
import offerFile from '../assets/offer-file.webp';
import { Button, Modal } from './ui';

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
      {offer && (
        <div className="flex flex-col items-center gap-1.5 text-center">
          <p className="text-[17px] font-semibold tracking-tight">From {offer.peerName}</p>
          <img src={offerFile} alt="" width={104} height={104} className="my-1.5 h-[104px] w-[104px] select-none" draggable={false} />
          <p className="max-w-full truncate text-[17px] font-semibold tracking-tight" title={offer.files[0]?.path}>
            {offer.fileCount > 1 ? `${offer.fileCount} files` : (offer.files[0]?.path.split('/').pop() ?? '1 file')}
          </p>
          <p className="text-muted">
            {formatBytes(offer.totalBytes)} ·{' '}
            {offer.trusted ? (
              <span>Paired device</span>
            ) : (
              <span className="text-warn">Not paired</span>
            )}
          </p>
          {offer.fileCount > 1 && (
            <ul aria-label="Files offered" className="mt-1 w-full space-y-0.5 text-[13px] text-muted">
              {offer.files.slice(0, 3).map((f) => (
                <li key={f.path} className="truncate" title={f.path}>
                  {f.path}
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

