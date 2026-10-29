import { PENDING, VERIFIED } from '../data';
import { Reveal } from './Reveal';

export function Status() {
  return (
    <section className="section" id="status">
      <div className="wrap">
        <Reveal as="p" className="eyebrow">
          Status
        </Reveal>
        <Reveal as="h2" className="h2">
          Early beta.
          <br />
          <span className="muted">Straight about it.</span>
        </Reveal>
        <Reveal className="status">
          <div>
            <h3>Verified</h3>
            <ul className="ticks">
              {VERIFIED.map((t) => (
                <li key={t}>{t}</li>
              ))}
            </ul>
          </div>
          <div>
            <h3>Still being proven</h3>
            <ul className="dots">
              {PENDING.map((t) => (
                <li key={t}>{t}</li>
              ))}
            </ul>
          </div>
        </Reveal>
      </div>
    </section>
  );
}
