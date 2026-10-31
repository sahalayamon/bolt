import { useEffect, useState } from 'react';
import { AnimatePresence, MotionConfig, motion } from 'framer-motion';
import { Toaster, toast } from 'sonner';
import { ChevronLeft, Download } from 'lucide-react';
import { api, errorMessage } from './lib/api';
import { useApp } from './store/app';
import { DevicePane } from './components/DevicePane';
import { TitleBar } from './components/TitleBar';
import { useIsWide, useSelectedDevice } from './hooks/useSelected';
import { useAppearance } from './hooks/useAppearance';
import { pathsFromPasteEvent } from './lib/paste';
import { IncomingOfferDialog, PairRequestDialog, SentDialog } from './components/Dialogs';
import { ReceivingDialog } from './components/ReceivingDialog';
import { ReceivedTextDialog } from './components/TextDialogs';
import { HomeView } from './views/Home';
import { SendReviewView } from './views/SendReview';
import { SendingView } from './views/Sending';
import { TransfersView } from './views/Transfers';
import { HistoryView } from './views/History';
import { DevicesView } from './views/Devices';
import { SettingsView } from './views/Settings';
import type { View } from './lib/types';
import { T_FAST, cx } from './components/ui';

const VIEWS: Record<View, () => React.JSX.Element> = {
  home: HomeView,
  send: SendReviewView,
  sending: SendingView,
  transfers: TransfersView,
  history: HistoryView,
  devices: DevicesView,
  settings: SettingsView,
};

const SHORTCUTS: View[] = ['home', 'transfers', 'history', 'devices', 'settings'];

/** OS drag-and-drop anywhere in the window starts a send. */
function useFileDrop(): boolean {
  const [dragging, setDragging] = useState(false);
  const startDraft = useApp((s) => s.startDraft);
  const deviceId = useSelectedDevice()?.id;
  useEffect(() => {
    let off: (() => void) | undefined;
    let cancelled = false;
    api
      .onFileDrop({
        onEnter: () => setDragging(true),
        onLeave: () => setDragging(false),
        onDrop: (paths) => startDraft(paths, deviceId),
      })
      .then((u) => (cancelled ? u() : (off = u)))
      .catch(() => {
        /* not running inside Tauri (tests / plain browser) */
      });
    return () => {
      cancelled = true;
      off?.();
    };
  }, [startDraft, deviceId]);
  return dragging;
}

export default function App() {
  const view = useApp((s) => s.view);
  const navigate = useApp((s) => s.navigate);
  const init = useApp((s) => s.init);
  const loaded = useApp((s) => s.loaded);
  const motionPref = useAppearance();
  const themePref = useApp((s) => s.settings?.theme ?? 'dark');
  const dragging = useFileDrop();
  const wide = useIsWide();
  const startDraft = useApp((s) => s.startDraft);
  const selectedId = useSelectedDevice()?.id;

  // Ctrl+V anywhere (outside text fields) sends the clipboard's files, image or text.
  useEffect(() => {
    const onPaste = (e: ClipboardEvent) => {
      const t = e.target as HTMLElement | null;
      if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable)) return;
      if (!e.clipboardData || useApp.getState().offers.length > 0) return;
      e.preventDefault();
      pathsFromPasteEvent(e.clipboardData)
        .then((paths) => (paths.length ? startDraft(paths, selectedId) : toast('Nothing to paste')))
        .catch((err) => toast.error(errorMessage(err)));
    };
    window.addEventListener('paste', onPaste);
    return () => window.removeEventListener('paste', onPaste);
  }, [startDraft, selectedId]);

  // Show the (initially hidden) window once there is something to paint; fall back after 1.5 s.
  useEffect(() => {
    if (!loaded) return;
    const t = requestAnimationFrame(() => void api.win.show().catch(() => {}));
    return () => cancelAnimationFrame(t);
  }, [loaded]);
  useEffect(() => {
    const t = setTimeout(() => void api.win.show().catch(() => {}), 1500);
    return () => clearTimeout(t);
  }, []);
  const selectDevice = useApp((s) => s.selectDevice);
  const hasSelection = useApp((s) => s.devices.some((d) => d.id === s.selectedId));
  // Narrow windows show one pane at a time: the list, or the detail/section you opened.
  const listOnly = !wide && view === 'home' && !hasSelection;

  useEffect(() => {
    let off: (() => void) | undefined;
    let cancelled = false;
    void init().then((u) => (cancelled ? u() : (off = u)));
    return () => {
      cancelled = true;
      off?.();
    };
  }, [init]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey) || e.altKey || e.shiftKey) return;
      const i = Number(e.key) - 1;
      const v = SHORTCUTS[i];
      if (v) {
        e.preventDefault();
        navigate(v);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [navigate]);

  const View = VIEWS[view];
  const reduced = motionPref === 'on' ? 'always' : motionPref === 'off' ? 'never' : 'user';

  return (
    <MotionConfig reducedMotion={reduced}>
      <a
        href="#main"
        className="sr-only focus:not-sr-only focus:absolute focus:top-2 focus:left-2 focus:z-[60] focus:rounded-control focus:bg-surface focus:px-3 focus:py-2"
      >
        Skip to content
      </a>
      <TitleBar />
      <div className="flex h-full">
        <DevicePane
          className={cx(
            'shrink-0 border-line min-[720px]:w-[300px] min-[720px]:border-r',
            wide ? '' : listOnly ? 'w-full' : 'hidden',
          )}
        />
        {(wide || !listOnly) && (
          <main id="main" tabIndex={-1} className="scroll-thin flex min-w-0 flex-1 flex-col overflow-y-auto pt-9">
            <div
              className={cx(
                'mx-auto flex flex-col px-5 py-5 min-[720px]:px-10 min-[720px]:py-8',
                'w-full flex-1',
                view === 'send' ? 'max-w-4xl' : 'max-w-2xl',
              )}
            >
              {!wide && view !== 'send' && (
                <button
                  type="button"
                  onClick={() => selectDevice(null)}
                  className="-ml-2 mb-3 inline-flex min-h-11 w-fit cursor-pointer items-center gap-0.5 rounded-control px-2 text-accent"
                >
                  <ChevronLeft className="h-5 w-5" strokeWidth={2} aria-hidden="true" /> Devices
                </button>
              )}
              {!loaded ? (
                <p className="text-muted">Starting…</p>
              ) : (
                <AnimatePresence mode="wait" initial={false}>
                  <motion.div
                    key={view}
                    className="flex min-h-0 flex-1 flex-col"
                    initial={{ opacity: 0, y: 6 }}
                    animate={{ opacity: 1, y: 0 }}
                    exit={{ opacity: 0 }}
                    transition={T_FAST}
                  >
                    <View />
                  </motion.div>
                </AnimatePresence>
              )}
            </div>
          </main>
        )}
      </div>

      <AnimatePresence>
        {dragging && (
          <motion.div
            aria-hidden="true"
            className="pointer-events-none fixed top-12 right-3 bottom-3 left-3 z-30 grid place-items-center min-[720px]:left-[312px] rounded-[22px] border border-dashed border-accent/70 bg-bg/70 backdrop-blur-md"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
          >
            <div className="flex flex-col items-center gap-3 text-accent">
              <Download className="h-10 w-10" strokeWidth={1.5} />
              <p className="text-lg font-semibold tracking-tight">Drop to send</p>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      <IncomingOfferDialog />
      <ReceivingDialog />
      <ReceivedTextDialog />
      <PairRequestDialog />
      <SentDialog />
      <Toaster
        position="bottom-right"
        theme={themePref}
        offset={16}
        mobileOffset={16}
        toastOptions={{
          style: {
            background: 'var(--c-material)',
            color: 'var(--c-fg)',
            border: '1px solid var(--c-line)',
            backdropFilter: 'saturate(180%) blur(24px)',
            borderRadius: '14px',
            boxShadow: 'var(--shadow-pop)',
          },
          classNames: { description: '!text-muted' },
        }}
      />
    </MotionConfig>
  );
}
