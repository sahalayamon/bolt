import { STEPS } from '../data';
import { Reveal } from './Reveal';

export function How() {
  return (
    <section className="section how" id="how">
      <div className="wrap how-grid">
        <div className="how-head">
          <Reveal as="p" className="eyebrow">
            How it works
          </Reveal>
          <Reveal as="h2" className="h2">
            Three steps.
            <br />
            <span className="muted">Then it’s routine.</span>
          </Reveal>
          <Reveal as="p" className="lead">
            Pair a device once with a QR code and a 6-digit check. After that it’s just pick, accept, done.
          </Reveal>
        </div>
        <ol className="steps">
          {STEPS.map((s) => (
            <Reveal key={s.n} as="li" className="step">
              <span className="num">{s.n}</span>
              <div>
                <h3>{s.title}</h3>
                <p>{s.body}</p>
              </div>
            </Reveal>
          ))}
        </ol>
      </div>
    </section>
  );
}
