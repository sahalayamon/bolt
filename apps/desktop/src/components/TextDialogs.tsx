import { useEffect, useState } from 'react';
import { toast } from 'sonner';
import { ClipboardPaste, Copy, ExternalLink, Link2, Type } from 'lucide-react';
import { api, errorMessage } from '../lib/api';
import { MAX_TEXT_BYTES, asHttpUrl, textBytes, textLabelOf } from '../lib/text';
import { formatBytes } from '../lib/format';
import { useApp } from '../store/app';
import { Button, Modal } from './ui';

/** Compose the text or link to send. Plain text only; pasted content is never interpreted. */
export function TextComposer({
  open,
  initial,
  onSave,
  onClose,
}: {
  open: boolean;
  initial: string;
  onSave: (text: string) => void;
  onClose: () => void;
}) {
  const [value, setValue] = useState(initial);
  useEffect(() => {
    if (open) setValue(initial);
  }, [open, initial]);

  const bytes = textBytes(value);
  const tooBig = bytes > MAX_TEXT_BYTES;
  const empty = value.trim() === '';

  const paste = async () => {
    try {
      const t = await navigator.clipboard.readText();
      if (t) setValue((v) => v + t);
      else toast('Nothing to paste');
    } catch {
      toast.error('Clipboard access was blocked. Press Ctrl+V in the box instead.');
    }
  };

  return (
    <Modal
      open={open}
      title="Send text or link"
      onClose={onClose}
      initialFocus="textarea"
      width="max-w-lg"
      footer={
        <>
          <Button onClick={onClose}>Cancel</Button>
          <Button variant="primary" disabled={empty || tooBig} onClick={() => onSave(value)}>
            Done
          </Button>
        </>
      }
    >
      <div className="space-y-2">
        <textarea
          aria-label="Text or link to send"
          value={value}
          onChange={(e) => setValue(e.target.value)}
          rows={7}
          placeholder="Type or paste a note or a link"
          aria-invalid={tooBig}
          className="field resize-y text-[14px]"
        />
        <div className="flex items-center justify-between gap-3">
          <Button size="sm" variant="ghost" onClick={() => void paste()}>
            <ClipboardPaste className="h-4 w-4" strokeWidth={1.75} aria-hidden="true" /> Paste
          </Button>
          <p className={tooBig ? 'text-xs text-bad' : 't-caption'} role={tooBig ? 'alert' : undefined}>
            {tooBig ? `Too long: the limit is ${formatBytes(MAX_TEXT_BYTES)}` : formatBytes(bytes)}
          </p>
        </div>
      </div>
    </Modal>
  );
}

/** Shows text that just arrived. Always plain text; a link is opened only by clicking Open. */
export function ReceivedTextDialog() {
  const item = useApp((s) => s.receivedTexts[0]);
  const dismiss = useApp((s) => s.dismissReceivedText);

  const copy = async (texts: string[]) => {
    try {
      await navigator.clipboard.writeText(texts.join('\n\n'));
      toast.success('Copied');
    } catch {
      toast.error('Could not copy to the clipboard');
    }
  };

  return (
    <Modal
      open={!!item}
      title={item ? `${textLabelOf(item.texts[0] ?? '')} from ${item.peerName}` : 'Received text'}
      hideTitle
      onClose={dismiss}
      initialFocus="[data-autofocus]"
      width="max-w-lg"
      footer={
        <>
          <Button className="rounded-full! sm:flex-1" onClick={dismiss}>
            Done
          </Button>
          <Button
            variant="primary"
            data-autofocus
            className="rounded-full! sm:flex-1"
            onClick={() => item && void copy(item.texts)}
          >
            <Copy className="h-4 w-4" strokeWidth={1.75} aria-hidden="true" /> Copy
          </Button>
        </>
      }
    >
      {item && (
        <div className="space-y-3">
        <p className="flex items-center justify-center gap-2 text-[17px] font-semibold tracking-tight">
          {textLabelOf(item.texts[0] ?? '') === 'Link' ? (
            <Link2 className="h-5 w-5 text-accent" strokeWidth={1.75} aria-hidden="true" />
          ) : (
            <Type className="h-5 w-5 text-accent" strokeWidth={1.75} aria-hidden="true" />
          )}{' '}
          {textLabelOf(item.texts[0] ?? '')} from {item.peerName}
        </p>
        <ul aria-label="Received text" className="space-y-3">
          {item.texts.map((t, i) => {
            const url = asHttpUrl(t);
            const label = textLabelOf(t);
            return (
              <li key={i} className="space-y-2">
                <textarea
                  readOnly
                  data-testid="received-text"
                  aria-label={`Received ${label.toLowerCase()}`}
                  value={t}
                  rows={Math.min(10, Math.max(7, t.split(/\n/).length))}
                  onFocus={(e) => e.currentTarget.select()}
                  className="field scroll-thin resize-none text-[14px]"
                />
                <div className="flex gap-2">
                  {url && (
                    <Button
                      size="sm"
                      onClick={() => void api.openLink(url).catch((e) => toast.error(errorMessage(e)))}
                    >
                      <ExternalLink className="h-4 w-4" strokeWidth={1.75} aria-hidden="true" /> Open
                    </Button>
                  )}
                </div>
              </li>
            );
          })}
        </ul>
        </div>
      )}
    </Modal>
  );
}
