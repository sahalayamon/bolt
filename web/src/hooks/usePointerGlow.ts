import type { PointerEvent } from 'react';
import { prefersReducedMotion } from './motion';

/** Handler that feeds the pointer position into CSS variables for the card spotlight. */
export function pointerGlow(e: PointerEvent<HTMLElement>): void {
  if (prefersReducedMotion()) return;
  const el = e.currentTarget;
  const r = el.getBoundingClientRect();
  el.style.setProperty('--mx', `${e.clientX - r.left}px`);
  el.style.setProperty('--my', `${e.clientY - r.top}px`);
}
