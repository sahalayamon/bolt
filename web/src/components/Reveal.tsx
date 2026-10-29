import { createElement, useEffect, useRef, type CSSProperties, type ReactNode } from 'react';
import { prefersReducedMotion } from '../hooks/motion';

type Tag = 'div' | 'article' | 'li' | 'p' | 'h1' | 'h2' | 'figure' | 'span';

interface Props {
  as?: Tag;
  delay?: number;
  className?: string;
  style?: CSSProperties;
  children: ReactNode;
  onPointerMove?: React.PointerEventHandler<HTMLElement>;
  'aria-label'?: string;
}

// One shared observer for every Reveal on the page.
let observer: IntersectionObserver | null = null;
function getObserver(): IntersectionObserver {
  observer ??= new IntersectionObserver(
    (entries) => {
      for (const en of entries) {
        if (en.isIntersecting) {
          en.target.classList.add('in');
          observer?.unobserve(en.target);
        }
      }
    },
    { threshold: 0.12, rootMargin: '0px 0px -6% 0px' },
  );
  return observer;
}

/** Fades and lifts its element into view the first time it scrolls on screen. */
export function Reveal({ as = 'div', delay, className = '', style, children, ...rest }: Props) {
  const ref = useRef<HTMLElement>(null);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    if (prefersReducedMotion() || !('IntersectionObserver' in window)) {
      el.classList.add('in');
      return;
    }
    const io = getObserver();
    io.observe(el);
    return () => io.unobserve(el);
  }, []);

  return createElement(
    as,
    { ref, className: `reveal ${className}`.trim(), style: delay ? { ...style, ['--d' as string]: `${delay}s` } : style, ...rest },
    children,
  );
}
