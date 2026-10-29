import { toast } from 'sonner';
import { ClipboardPaste, FilePlus2, FolderPlus, Radar, ShieldCheck, ShieldQuestion } from 'lucide-react';
import { pathsFromClipboardApi } from '../lib/paste';
import { api, errorMessage } from '../lib/api';
import { useSelectedDevice } from '../hooks/useSelected';
import { useApp } from '../store/app';
import { DeviceAvatar } from '../components/DeviceAvatar';

async function pick(kind: 'files' | 'folder'): Promise<string[]> {
  try {
    if (kind === 'files') return await api.pickFiles();
    const f = await api.pickFolder();
    return f ? [f] : [];
  } catch (e) {
    toast.error(errorMessage(e));
    return [];
  }
}

/** Detail pane for the selected device: who it is and the ways to send to it. */
export function HomeView() {
  const device = useSelectedDevice();
  const startDraft = useApp((s) => s.startDraft);

  const choose = async (kind: 'files' | 'folder') => {
    const paths = await pick(kind);
    if (paths.length) startDraft(paths, device?.id);
  };

  const paste = async () => {
    try {
      const paths = await pathsFromClipboardApi();
      if (paths.length) startDraft(paths, device?.id);
      else toast('Nothing to paste');
    } catch {
      toast.error('Clipboard access was blocked. Press Ctrl+V instead.');
    }
  };

  // One layout for both states, so the action tiles sit in exactly the same place whether or not a device is found.
  return (
    <div className="flex min-h-[22rem] flex-1 flex-col items-center justify-between gap-8 py-4 text-center">
      <div className="flex flex-1 flex-col items-center justify-center gap-6">
        {device ? (
          <>
            <DeviceAvatar kind={device.kind} size="lg" />
            <div>
              <h1 className="text-[26px] font-semibold tracking-tight">{device.name}</h1>
              <p className="mt-1.5 flex items-center justify-center gap-1.5 text-muted">
                {device.trusted ? (
                  <>
                    <ShieldCheck className="h-4 w-4 text-ok" strokeWidth={1.75} aria-hidden="true" /> Paired device
                  </>
                ) : (
                  <>
                    <ShieldQuestion className="h-4 w-4" strokeWidth={1.75} aria-hidden="true" /> Not paired
                  </>
                )}
                <span aria-hidden="true">·</span>
                <span className="font-mono text-xs">{device.shortId}</span>
              </p>
            </div>
          </>
        ) : (
          <>
            <Radar className="h-[72px] w-[72px] text-faint" strokeWidth={1.25} aria-hidden="true" />
            <div>
              <h1 className="text-[26px] font-semibold tracking-tight">No devices found yet</h1>
              <p className="mx-auto mt-4 max-w-md text-[15px] leading-relaxed text-muted">
                Open Bolt on another device on the same Wi-Fi or hotspot. Only devices actually announcing themselves on
                your network appear here.
              </p>
            </div>
          </>
        )}
      </div>

      <div className="w-full max-w-lg">
        <div className="grid grid-cols-3 gap-2.5">
          <ActionTile icon={<ClipboardPaste />} label="Paste" onClick={() => void paste()} />
          <ActionTile icon={<FilePlus2 />} label="Send files" onClick={() => void choose('files')} />
          <ActionTile icon={<FolderPlus />} label="Send a folder" onClick={() => void choose('folder')} />
        </div>
        <p className="t-caption mt-4">You can also press Ctrl+V to paste, or drag &amp; drop files to send them.</p>
      </div>
    </div>
  );
}

function ActionTile({ icon, label, onClick }: { icon: React.ReactNode; label: string; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex min-h-[84px] cursor-pointer flex-col items-center justify-center gap-2 rounded-card bg-surface text-[13px] ring-1 ring-line transition duration-150 hover:bg-elevated active:scale-[0.98] [&>svg]:h-5 [&>svg]:w-5 [&>svg]:stroke-[1.6]"
    >
      {icon}
      {label}
    </button>
  );
}
