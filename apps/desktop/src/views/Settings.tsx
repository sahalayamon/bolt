import { useEffect, useState, type ReactNode } from 'react';
import { toast } from 'sonner';
import { FolderOpen } from 'lucide-react';
import { api, errorMessage } from '../lib/api';
import { useApp } from '../store/app';
import { Button, Card, PageHeader, SectionTitle, Switch } from '../components/ui';
import type { MotionPref, Settings, ThemePref } from '../lib/types';

/** One labelled row inside a grouped card; control sits on the right (below on narrow widths). */
function Row({ label, hint, htmlFor, children }: { label: string; hint?: string; htmlFor?: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-2 px-4 py-3 sm:flex-row sm:items-center sm:justify-between sm:gap-6">
      <div className="min-w-0 sm:w-[45%] sm:shrink-0">
        <label htmlFor={htmlFor} className="block">
          {label}
        </label>
        {hint && <p className="t-caption mt-0.5">{hint}</p>}
      </div>
      <div className="min-w-0 sm:flex-1">{children}</div>
    </div>
  );
}

export function SettingsView() {
  const saved = useApp((s) => s.settings);
  const info = useApp((s) => s.info);
  const setSettings = useApp((s) => s.setSettings);
  const refreshInfo = useApp((s) => s.refreshInfo);
  const [draft, setDraft] = useState<Settings | null>(saved);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    setDraft(saved);
  }, [saved]);

  if (!draft || !saved) return <p className="text-muted">Loading…</p>;
  const dirty = JSON.stringify(draft) !== JSON.stringify(saved);
  const patch = (p: Partial<Settings>) => setDraft({ ...draft, ...p });
  const nameInvalid = draft.deviceName.trim() === '';

  const save = async () => {
    setSaving(true);
    try {
      setSettings(await api.updateSettings(draft));
      await refreshInfo();
      toast.success('Settings saved');
    } catch (e) {
      toast.error(errorMessage(e));
    } finally {
      setSaving(false);
    }
  };

  const chooseFolder = async () => {
    try {
      const f = await api.pickFolder();
      if (f) patch({ downloadDir: f });
    } catch (e) {
      toast.error(errorMessage(e));
    }
  };

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        void save();
      }}
    >
      <PageHeader
        title="Settings"
        actions={
          <>
            <Button type="button" variant="ghost" disabled={!dirty || saving} onClick={() => setDraft(saved)}>
              Revert
            </Button>
            <Button type="submit" variant="primary" disabled={!dirty || saving || nameInvalid}>
              {saving ? 'Saving…' : 'Save changes'}
            </Button>
          </>
        }
      />

      <div className="space-y-8">
        <section aria-labelledby="s-recv">
          <SectionTitle id="s-recv">This device</SectionTitle>
          <Card className="divide-y divide-line">
            <Row label="Device name" hint="How this computer appears to nearby devices." htmlFor="device-name">
              <input
                id="device-name"
                value={draft.deviceName}
                maxLength={60}
                aria-invalid={nameInvalid}
                onChange={(e) => patch({ deviceName: e.target.value })}
                className="field"
              />
            </Row>
            <Row
              label="Download folder"
              hint="Accepted files are saved here. Existing files are never overwritten."
              htmlFor="dl-dir"
            >
              <div className="flex items-center gap-1.5">
                <input
                  id="dl-dir"
                  aria-label="Download folder"
                  value={draft.downloadDir}
                  onChange={(e) => patch({ downloadDir: e.target.value })}
                  className="field min-w-0 flex-1 font-mono text-xs"
                />
                <Button type="button" onClick={() => void chooseFolder()}>
                  Browse…
                </Button>
                <Button
                  type="button"
                  variant="ghost"
                  aria-label="Open download folder"
                  onClick={() => void api.openDownloadDir().catch((e) => toast.error(errorMessage(e)))}
                >
                  <FolderOpen className="h-4 w-4" strokeWidth={1.75} aria-hidden="true" />
                </Button>
              </div>
            </Row>
            <Switch
              label="Auto-accept from paired devices"
              description="Skip the confirmation for devices you paired. Unknown devices always ask first."
              checked={draft.autoAcceptTrusted}
              onChange={(v) => patch({ autoAcceptTrusted: v })}
            />
            <Switch
              label="In-app notifications"
              description="Show a message when a transfer finishes or fails."
              checked={draft.notifications}
              onChange={(v) => patch({ notifications: v })}
            />
            <Switch
              label="Start Bolt when I sign in"
              description="Starts quietly in the system tray, so sharing needs no extra steps. Uses almost no power while idle."
              checked={draft.startOnLogin}
              onChange={(v) => patch({ startOnLogin: v, ...(v ? { runInBackground: true } : {}) })}
            />
            <Switch
              label="Add Bolt to the right-click menu"
              description="Right-click files or folders, then Show more options, then Bolt and pick a device. Only for your Windows account."
              checked={draft.contextMenu}
              onChange={(v) => patch({ contextMenu: v })}
            />
            <Switch
              label="Keep running in the tray when closed"
              description="Closing the window keeps Bolt receiving. Quit it from the tray icon."
              checked={draft.runInBackground}
              onChange={(v) => patch({ runInBackground: v, ...(!v ? { startOnLogin: false } : {}) })}
            />
          </Card>
        </section>

        <section aria-labelledby="s-look">
          <SectionTitle id="s-look">Appearance</SectionTitle>
          <Card className="divide-y divide-line">
            <Row label="Theme" htmlFor="theme">
              <select
                id="theme"
                value={draft.theme}
                onChange={(e) => patch({ theme: e.target.value as ThemePref })}
                className="field"
              >
                <option value="dark">Dark</option>
                <option value="light">Light</option>
                <option value="system">Match system</option>
              </select>
            </Row>
            <Row
              label="Reduce motion"
              hint="“Match system” follows your OS accessibility setting."
              htmlFor="motion"
            >
              <select
                id="motion"
                value={draft.reduceMotion}
                onChange={(e) => patch({ reduceMotion: e.target.value as MotionPref })}
                className="field"
              >
                <option value="system">Match system</option>
                <option value="on">Always reduce</option>
                <option value="off">Never reduce</option>
              </select>
            </Row>
          </Card>
        </section>

        {info && (
          <section aria-labelledby="s-about">
            <SectionTitle id="s-about">About</SectionTitle>
            <Card className="px-4 py-3">
              <dl className="grid grid-cols-1 gap-x-6 gap-y-2.5 text-[13px] sm:grid-cols-[7.5rem_1fr]">
                <dt className="text-muted">Device ID</dt>
                <dd className="font-mono text-xs break-all select-all">{info.deviceId}</dd>
                <dt className="text-muted">Listening on</dt>
                <dd>
                  {info.localIp}:{info.port} (QUIC / UDP)
                </dd>
                <dt className="text-muted">Discovery</dt>
                <dd>
                  {info.discoveryActive ? 'mDNS active' : `Unavailable${info.discoveryError ? `: ${info.discoveryError}` : ''}`}
                </dd>
                <dt className="text-muted">Version</dt>
                <dd>{info.version}</dd>
              </dl>
            </Card>
          </section>
        )}
      </div>
    </form>
  );
}
