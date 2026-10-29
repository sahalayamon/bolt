import { useEffect } from 'react';
import { useApp } from '../store/app';

/** Applies the theme + motion preferences to <html>. Returns the motion preference. */
export function useAppearance() {
  const theme = useApp((s) => s.settings?.theme ?? 'dark');
  const motionPref = useApp((s) => s.settings?.reduceMotion ?? 'system');
  useEffect(() => {
    const mq = window.matchMedia('(prefers-color-scheme: light)');
    const apply = () => {
      document.documentElement.dataset.theme = theme === 'system' ? (mq.matches ? 'light' : 'dark') : theme;
    };
    apply();
    mq.addEventListener?.('change', apply);
    return () => mq.removeEventListener?.('change', apply);
  }, [theme]);
  useEffect(() => {
    document.documentElement.dataset.reduceMotion = motionPref;
  }, [motionPref]);
  return motionPref;
}
