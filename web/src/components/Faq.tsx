import { Plus } from 'lucide-react';
import { FAQ } from '../data';
import { Reveal } from './Reveal';

export function Faq() {
  return (
    <section className="section faq" id="faq">
      <div className="wrap faq-wrap">
        <Reveal as="p" className="eyebrow">
          Questions
        </Reveal>
        <Reveal as="h2" className="h2">
          Good to know.
        </Reveal>
        <div className="faq-list">
          {FAQ.map((f, i) => (
            <Reveal key={f.q} delay={Math.min(i, 3) * 0.04}>
              <details className="faq-item">
                <summary>
                  <span>{f.q}</span>
                  <Plus aria-hidden="true" size={18} />
                </summary>
                <p>{f.a}</p>
              </details>
            </Reveal>
          ))}
        </div>
      </div>
    </section>
  );
}
