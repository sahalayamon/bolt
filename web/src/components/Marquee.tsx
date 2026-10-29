import { MARQUEE } from '../data';

export function Marquee() {
  // The list is rendered twice so the -50% translate loops seamlessly.
  return (
    <section className="marquee" aria-label="Highlights">
      <div className="mq-track">
        {[...MARQUEE, ...MARQUEE].map((t, i) => (
          <span key={i}>{t}</span>
        ))}
      </div>
    </section>
  );
}
