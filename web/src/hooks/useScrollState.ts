import { useEffect, useRef, useState } from 'react';

/** Drives the top progress bar (via ref, no re-render per frame) and reports whether the page has scrolled. */
export function useScrollState() {
  const barRef = useRef<HTMLDivElement>(null);
  const [stuck, setStuck] = useState(false);

  useEffect(() => {
    const onScroll = () => {
      const max = document.documentElement.scrollHeight - innerHeight;
      if (barRef.current) barRef.current.style.transform = `scaleX(${max > 0 ? scrollY / max : 0})`;
      setStuck(scrollY > 24);
    };
    addEventListener('scroll', onScroll, { passive: true });
    onScroll();
    return () => removeEventListener('scroll', onScroll);
  }, []);

  return { barRef, stuck };
}
