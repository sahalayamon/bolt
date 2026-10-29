import { ABOUT, CREATOR } from '../data';
import { Reveal } from './Reveal';

export function MadeBy() {
  return (
    <section className="section about" id="about" aria-label="About Bolt">
      <div className="wrap about-grid">
        <div className="about-copy">
          <Reveal as="p" className="eyebrow">
            About Bolt
          </Reveal>
          <Reveal as="h2" className="h2">
            {ABOUT.title[0]}
            <br />
            <span className="muted">{ABOUT.title[1]}</span>
          </Reveal>
          {ABOUT.paragraphs.map((p) => (
            <Reveal key={p} as="p" className="lead">
              {p}
            </Reveal>
          ))}
        </div>

        <Reveal as="article" delay={0.08} className="maker">
          <p className="maker-label">Made by</p>
          <div className="maker-head">
            <span className="avatar">
              <img src={CREATOR.photo} alt={CREATOR.name} width={80} height={80} loading="lazy" />
            </span>
            <div>
              <h3>{CREATOR.name}</h3>
              <p className="maker-role">{CREATOR.role}</p>
            </div>
          </div>
          <p className="maker-bio">{CREATOR.bio}</p>
          <blockquote>{CREATOR.quote}</blockquote>
        </Reveal>
      </div>
    </section>
  );
}
