import { useEffect, useState } from 'react';
import { toast } from 'sonner';
import { api, errorMessage } from '../lib/api';
import { percent } from '../lib/format';
import { textInfo } from '../lib/text';
import { useApp } from '../store/app';
import type { Transfer } from '../lib/types';
import { ACTIVE_STATES, ProgressRing, transferSubtitle } from './ProgressRing';
import { ReceivedBadge } from './ReceivedBadge';
import { Button, Modal } from './ui';


/**
 * The card shown while someone sends you files: progress ring, current file, speed and ETA, Cancel.
 * Closing it (Esc / click outside) only hides it; the transfer keeps going and stays on the Transfers screen.
 */
export function ReceivingDialog() {
  const transfers = useApp((s) => s.transfers);
  const [focusId, setFocusId] = useState<string | null>(null);
  const [hidden, setHidden] = useState<Set<string>>(new Set());
  const [cancelling, setCancelling] = useState(false);

  const incoming = transfers.find((t) => t.direction === 'received' && ACTIVE_STATES.includes(t.state) && !hidden.has(t.id));
  useEffect(() => {
    if (incoming && incoming.id !== focusId) {
      setFocusId(incoming.id);
      setCancelling(false);
    }
  }, [incoming, focusId]);

  const shown: Transfer | undefined = incoming ?? transfers.find((t) => t.id === focusId);
  // A finished text-only transfer has nothing to open or reveal: the received-text dialog takes over.
  const textOnlyDone = shown?.state === 'completed' && textInfo(shown) !== null;
  const open = !!shown && shown.direction === 'received' && !hidden.has(shown.id) && !textOnlyDone;
  const hide = () => shown && setHidden((h) => new Set(h).add(shown.id));

  const running = !!shown && ACTIVE_STATES.includes(shown.state);
  const done = shown?.state === 'completed';
  const pct = shown ? (done ? 100 : percent(shown.bytesDone, shown.bytesTotal)) : 0;

  const cancel = async () => {
    if (!shown) return;
    setCancelling(true);
    try {
      await api.cancelTransfer(shown.id);
    } catch (e) {
      toast.error(errorMessage(e));
      setCancelling(false);
    }
  };

  return (
    <Modal open={open} title="Receiving files" hideTitle onClose={hide} width="max-w-sm" initialFocus="[data-autofocus]">
      {shown && (
        <div className="flex flex-col items-center gap-3 pb-3 text-center">
          <p className="text-[17px] font-semibold tracking-tight">
            {done ? 'Received from' : 'From'} {shown.peerName}
          </p>

          {done ? (
            <ReceivedBadge id={shown.id} />
          ) : (
            <ProgressRing pct={pct} done={false} failed={!running} waiting={shown.state === 'connecting' || shown.state === 'awaiting_approval'} label="Receiving progress">
              {shown.state === 'connecting' || shown.state === 'awaiting_approval' ? null : (
                <span className="text-[32px] font-semibold tabular-nums" aria-live="off">
                  {Math.round(pct)}%
                </span>
              )}
            </ProgressRing>
          )}

          <div className="min-w-0 max-w-full">
            <p className="truncate text-[17px] font-semibold tracking-tight" title={shown.currentFile ?? shown.title}>
              {textInfo(shown)?.label ?? ((running && shown.currentFile) || shown.title || 'Incoming files')}
            </p>
            <p className="t-caption mt-1 tabular-nums">{transferSubtitle(shown, running, done)}</p>
            {shown.error && !running && (
              <p role="alert" className="mt-2 text-[13px] text-bad">
                {shown.error}
              </p>
            )}
          </div>

          <div className="mt-2 flex w-full gap-2">
            {running ? (
              <Button
                data-autofocus
                variant="dangerSolid"
                className="min-h-[46px] w-full !rounded-full"
                disabled={cancelling}
                onClick={() => void cancel()}
              >
                {cancelling ? 'Cancelling…' : 'Cancel'}
              </Button>
            ) : (
              <>
                {done && (
                  <Button
                    className="min-h-[46px] flex-1 !rounded-full"
                    onClick={() => void api.openReceived(shown.id).catch((e) => toast.error(errorMessage(e)))}
                  >
                    Open
                  </Button>
                )}
                {done && (
                  <Button
                    className="min-h-[46px] flex-1 !rounded-full"
                    onClick={() => void api.revealTransfer(shown.id).catch((e) => toast.error(errorMessage(e)))}
                  >
                    Show folder
                  </Button>
                )}
                <Button data-autofocus variant="primary" className="min-h-[46px] flex-1 !rounded-full" onClick={hide}>
                  {done ? 'Done' : 'Close'}
                </Button>
              </>
            )}
          </div>
        </div>
      )}
    </Modal>
  );
}
