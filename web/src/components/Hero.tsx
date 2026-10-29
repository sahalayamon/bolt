import { Download, FileText } from 'lucide-react';
import { useEffect, useRef } from 'react';
import { prefersReducedMotion } from '../hooks/motion';
import { Reveal } from './Reveal';

const DURATION = 5200;
const HOLD = 1400;

/** Decorative looping progress for the illustration. It is not a measurement. */
function useDemoProgress() {
  const fillRef = useRef<HTMLSpanElement>(null);
  const pctRef = useRef<HTMLElement>(null);

  useEffect(() => {
    const set = (p: number) => {
      if (fillRef.current) fillRef.current.style.transform = `scaleX(${p})`;
      if (pctRef.current) pctRef.current.textContent = `${Math.round(p * 100)}%`;
    };
    if (prefersReducedMotion()) {
      set(1);
      return;
    }
    let start = performance.now();
    let frame = 0;
    const tick = (now: number) => {
      const t = now - start;
      set(1 - Math.pow(1 - Math.min(t / DURATION, 1), 2.2));
      if (t > DURATION + HOLD) start = now;
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, []);

  return { fillRef, pctRef };
}

function Scene() {
  const { fillRef, pctRef } = useDemoProgress();
  return (
    <Reveal as="figure" delay={0.28} className="scene" aria-label="Illustration: a laptop sending a file to a phone">
      <div className="dev laptop">
        <div className="dev-top">
          <i />
          <i />
          <i />
          <span>Bolt · Windows</span>
        </div>
        <div className="file">
          <div className="ficon">
            <FileText size={20} color="#14170a" strokeWidth={2.2} />
          </div>
          <div className="fmeta">
            <b>holiday-videos/</b>
            <small>38 files</small>
          </div>
          <span className="pill">Sending</span>
        </div>
        <div className="track">
          <span className="fill" ref={fillRef} />
        </div>
        <div className="row">
          <small>Verified on arrival</small>
          <small className="mono" ref={pctRef}>
            0%
          </small>
        </div>
      </div>

      <div className="beam" aria-hidden="true">
        <svg viewBox="0 0 400 40" preserveAspectRatio="none">
          <path d="M0 20 H400" />
        </svg>
        <i />
        <i />
        <i />
      </div>

      <div className="dev phone">
        <div className="notch" />
        <div className="phone-card">
          <small>Incoming</small>
          <b>From Sahal’s PC</b>
          <div className="row2">
            <span className="pill ghost">Decline</span>
            <span className="pill solid">Accept</span>
          </div>
        </div>
        <div className="ring r1" />
        <div className="ring r2" />
      </div>
    </Reveal>
  );
}

export function Hero() {
  return (
    <section className="hero" id="top">
      <div className="glow" aria-hidden="true" />
      <div className="grid-bg" aria-hidden="true" />
      <div className="wrap hero-inner">
        <Reveal as="p" className="eyebrow">
          <span className="dot" /> Free · offline · no account
        </Reveal>
        <Reveal as="h1" delay={0.05} className="display">
          Send anything.
          <br />
          <span className="lime">Straight</span> to the
          <br />
          other device.
        </Reveal>
        <Reveal as="p" delay={0.12} className="lead">
          Bolt moves files and whole folders between your Windows PC and Android phone over your own Wi-Fi or hotspot.
          Encrypted, verified, and it never touches the internet.
        </Reveal>
        <Reveal delay={0.2} className="cta">
          <a className="btn btn-lime" href="#download">
            <Download size={18} strokeWidth={2.4} aria-hidden="true" />
            Download Bolt
          </a>
          <a className="btn btn-ghost" href="#how">
            See how it works
          </a>
        </Reveal>
        <Scene />
      </div>
    </section>
  );
}
