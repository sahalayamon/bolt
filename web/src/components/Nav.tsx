import { NAV_LINKS } from '../data';
import { Brand } from './Logo';

export function Nav({ stuck }: { stuck: boolean }) {
  return (
    <header className={`nav${stuck ? ' stuck' : ''}`}>
      <Brand />
      <nav className="links" aria-label="Primary">
        {NAV_LINKS.map((l) => (
          <a key={l.href} href={l.href}>
            {l.label}
          </a>
        ))}
      </nav>
      <a className="btn btn-lime btn-sm" href="#download">
        Download
      </a>
    </header>
  );
}
