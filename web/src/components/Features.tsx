import { FolderOpen, Lock, Monitor, Radio, RotateCw, ShieldCheck, Wifi, type LucideIcon } from 'lucide-react';
import { pointerGlow } from '../hooks/usePointerGlow';
import { Reveal } from './Reveal';

interface Item {
  icon: LucideIcon;
  title: string;
  body: string;
  className?: string;
  delay?: number;
  ticks?: readonly string[];
}

const ITEMS: readonly Item[] = [
  {
    icon: Radio,
    title: 'Finds nearby devices',
    body: 'Open Bolt on both devices and they show up on their own using mDNS. If your network blocks discovery, type an IP address instead. Only real devices are ever listed.',
    className: 'c-wide',
  },
  {
    icon: ShieldCheck,
    title: 'Verified, or not done',
    body: 'Every file is hashed with BLAKE3 and flushed to disk before Bolt calls it finished.',
    delay: 0.06,
  },
  {
    icon: FolderOpen,
    title: 'Folders, kept intact',
    body: 'Send whole folders with structure preserved. Names are never overwritten, only numbered.',
  },
  {
    icon: RotateCw,
    title: 'Cancel & resume',
    body: 'Interrupted? Retry picks up from the verified partial, not from zero.',
    delay: 0.06,
  },
  {
    icon: Monitor,
    title: 'Native on both sides',
    body: 'A Windows app and an Android app, sharing one Rust transfer engine. Streaming I/O with a single small buffer, so huge files never fill memory.',
    className: 'c-tall',
    delay: 0.06,
    ticks: ['Windows 10 / 11', 'Android 10+', 'Light & dark themes'],
  },
  {
    icon: Lock,
    title: 'You approve everything',
    body: 'Unknown devices can’t send silently. Auto-accept is opt-in and only for devices you paired.',
  },
  {
    icon: Wifi,
    title: 'Wi-Fi, hotspot or USB',
    body: 'Same Wi-Fi, a phone hotspot or USB tethering. Any local IP link between the two works.',
    delay: 0.06,
  },
];

export function Features() {
  return (
    <section className="section" id="features">
      <div className="wrap">
        <Reveal as="p" className="eyebrow">
          Features
        </Reveal>
        <Reveal as="h2" className="h2">
          Built like a network tool.
          <br />
          <span className="muted">Feels like a tap.</span>
        </Reveal>

        <div className="bento">
          {ITEMS.map(({ icon: Icon, title, body, className, delay, ticks }) => (
            <Reveal key={title} as="article" delay={delay} className={`card ${className ?? ''}`} onPointerMove={pointerGlow}>
              <div className="ic">
                <Icon aria-hidden="true" />
              </div>
              <h3>{title}</h3>
              <p>{body}</p>
              {ticks && (
                <ul className="ticks">
                  {ticks.map((t) => (
                    <li key={t}>{t}</li>
                  ))}
                </ul>
              )}
            </Reveal>
          ))}
        </div>
      </div>
    </section>
  );
}
