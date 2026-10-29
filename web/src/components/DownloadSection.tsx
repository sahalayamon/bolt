import type { ReactNode } from 'react';
import { ANDROID_APK, VERSION, WINDOWS_EXE, WINDOWS_MSI, type DownloadFile } from '../data';
import { pointerGlow } from '../hooks/usePointerGlow';
import { AndroidIcon, WindowsIcon } from './Icons';
import { Reveal } from './Reveal';

function Hashes({ files }: { files: readonly DownloadFile[] }) {
  return (
    <details>
      <summary>SHA-256</summary>
      {files.map((f) => (
        <code key={f.label}>
          {f.label} {f.sha256}
        </code>
      ))}
    </details>
  );
}

interface CardProps {
  icon: ReactNode;
  title: string;
  subtitle: string;
  delay?: number;
  children: ReactNode;
}

function PlatformCard({ icon, title, subtitle, delay, children }: CardProps) {
  return (
    <Reveal as="article" delay={delay} className="dl-card" onPointerMove={pointerGlow}>
      <div className="dl-top">
        {icon}
        <div>
          <h3>{title}</h3>
          <small>{subtitle}</small>
        </div>
      </div>
      {children}
    </Reveal>
  );
}

export function DownloadSection() {
  return (
    <section className="section dl" id="download">
      <div className="dl-glow" aria-hidden="true" />
      <div className="wrap">
        <Reveal as="p" className="eyebrow">
          Download
        </Reveal>
        <Reveal as="h2" className="h2">
          Get Bolt <span className="lime">{VERSION}</span>
        </Reveal>
        <div className="dl-grid">
          <PlatformCard icon={<WindowsIcon />} title="Windows" subtitle="Windows 10 / 11 · 64-bit">
            <a className="btn btn-lime btn-block" href={WINDOWS_EXE.href} download>
              Download installer <em>{WINDOWS_EXE.size}</em>
            </a>
            <a className="alt" href={WINDOWS_MSI.href} download>
              Or the .msi ({WINDOWS_MSI.size})
            </a>
            <Hashes files={[WINDOWS_EXE, WINDOWS_MSI]} />
            <p className="note">
              Not code-signed yet, so Windows SmartScreen may warn. Choose <i>More info → Run anyway</i>.
            </p>
          </PlatformCard>

          <PlatformCard icon={<AndroidIcon />} title="Android" subtitle="Android 10 or newer" delay={0.08}>
            <a className="btn btn-lime btn-block" href={ANDROID_APK.href} download>
              Download APK <em>{ANDROID_APK.size}</em>
            </a>
            <Hashes files={[ANDROID_APK]} />
            <p className="note">
              Test build, signed with a debug key. Allow “install unknown apps” for your browser or file manager to
              install it.
            </p>
          </PlatformCard>
        </div>
      </div>
    </section>
  );
}
