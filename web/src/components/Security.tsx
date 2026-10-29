import { SECURITY } from '../data';
import { Reveal } from './Reveal';

export function Security() {
  return (
    <section className="section" id="security">
      <div className="wrap sec">
        <div>
          <Reveal as="p" className="eyebrow">
            Security
          </Reveal>
          <Reveal as="h2" className="h2">
            Private by design,
            <br />
            <span className="muted">not by promise.</span>
          </Reveal>
          <Reveal as="p" className="lead">
            No cloud, no relay, no account, no telemetry. Your files go device to device and nowhere else.
          </Reveal>
        </div>
        <ul className="sec-list">
          {SECURITY.map((s) => (
            <Reveal key={s.title} as="li">
              <b>{s.title}</b>
              <span>{s.body}</span>
            </Reveal>
          ))}
        </ul>
      </div>
    </section>
  );
}
