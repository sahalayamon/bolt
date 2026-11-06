import { INSTALL_HELP } from '../data';
import { Reveal } from './Reveal';

export function InstallHelp() {
  return (
    <div className="install">
      <Reveal className="install-title">
        Installing it
      </Reveal>
      <div className="install-grid">
        {INSTALL_HELP.map((g, gi) => (
          <Reveal key={g.platform} as="article" delay={gi * 0.08} className="install-card">
            <h4>{g.platform}</h4>
            <ol>
              {g.steps.map((s) => (
                <li key={s}>{s}</li>
              ))}
            </ol>
          </Reveal>
        ))}
      </div>
    </div>
  );
}
