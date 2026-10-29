import { Brand } from './Logo';

export function Footer() {
  return (
    <footer className="foot">
      <div className="wrap foot-in">
        <Brand size={24} />
        <small>Offline file sharing. No cloud, no accounts, no telemetry.</small>
        <a className="totop" href="#top">
          Back to top ↑
        </a>
      </div>
    </footer>
  );
}
