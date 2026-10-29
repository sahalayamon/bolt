import Lenis from 'lenis';
import { useEffect } from 'react';
import { prefersReducedMotion } from './motion';

/** Smooth scrolling for the whole page, and smooth in-page anchor navigation. */
export function useLenis(): void {
  useEffect(() => {
    const reduce = prefersReducedMotion();
    const lenis = reduce
      ? null
      : new Lenis({ duration: 1.15, easing: (t) => 1 - Math.pow(1 - t, 4), smoothWheel: true });

    let frame = 0;
    const raf = (time: number) => {
      lenis?.raf(time);
      frame = requestAnimationFrame(raf);
    };
    if (lenis) frame = requestAnimationFrame(raf);

    const onClick = (e: MouseEvent) => {
      const a = (e.target as Element | null)?.closest<HTMLAnchorElement>('a[href^="#"]');
      if (!a) return;
      const hash = a.getAttribute('href') ?? '';
      const target = hash === '#top' ? 0 : document.querySelector<HTMLElement>(hash);
      if (target === null) return;
      e.preventDefault();
      if (lenis) lenis.scrollTo(target, { offset: -70 });
      else if (target === 0) window.scrollTo({ top: 0 });
      else target.scrollIntoView({ block: 'start' });
      history.replaceState(null, '', hash);
    };
    document.addEventListener('click', onClick);

    return () => {
      document.removeEventListener('click', onClick);
      cancelAnimationFrame(frame);
      lenis?.destroy();
    };
  }, []);
}
