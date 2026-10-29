import { SHOWCASE } from '../data';
import { Reveal } from './Reveal';

export function Showcase() {
  return (
    <section className="section showcase" id="app">
      <div className="wrap show-grid">
        <Reveal className="phone-shot">
          <div className="phone-frame">
            <img
              src="phone-home.jpg"
              alt="The Bolt Android home screen showing nearby devices and the Files, Photos and Folder buttons"
              width={560}
              height={1132}
              loading="lazy"
            />
          </div>
        </Reveal>
        <div className="show-copy">
          <Reveal as="p" className="eyebrow">
            The app
          </Reveal>
          <Reveal as="h2" className="h2">
            {SHOWCASE.title[0]}
            <br />
            <span className="muted">{SHOWCASE.title[1]}</span>
          </Reveal>
          <Reveal as="p" className="lead">
            {SHOWCASE.lead}
          </Reveal>
          <ul className="show-points">
            {SHOWCASE.points.map((p, i) => (
              <Reveal key={p.title} as="li" delay={i * 0.06}>
                <b>{p.title}</b>
                <span>{p.body}</span>
              </Reveal>
            ))}
          </ul>
        </div>
      </div>
    </section>
  );
}
