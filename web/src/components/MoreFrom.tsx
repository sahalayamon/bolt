import { ArrowRight } from 'lucide-react';
import { MORE_FROM } from '../data';
import { Reveal } from './Reveal';

export function MoreFrom() {
  return (
    <section className="more" aria-label={`More from ${MORE_FROM.brand}`}>
      <div className="wrap">
        <Reveal as="p" className="more-label">
          More from {MORE_FROM.brand}
        </Reveal>
        <Reveal delay={0.06}>
          <a className="more-link" href={MORE_FROM.href} target="_blank" rel="noopener noreferrer">
            <span>Visit {MORE_FROM.host}</span>
            <ArrowRight aria-hidden="true" strokeWidth={1.6} />
          </a>
        </Reveal>
      </div>
    </section>
  );
}
