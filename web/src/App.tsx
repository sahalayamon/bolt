import { DownloadSection } from './components/DownloadSection';
import { Faq } from './components/Faq';
import { Features } from './components/Features';
import { Footer } from './components/Footer';
import { Hero } from './components/Hero';
import { How } from './components/How';
import { MadeBy } from './components/MadeBy';
import { Marquee } from './components/Marquee';
import { MoreFrom } from './components/MoreFrom';
import { Nav } from './components/Nav';
import { Security } from './components/Security';
import { Showcase } from './components/Showcase';
import { Status } from './components/Status';
import { useLenis } from './hooks/useLenis';
import { useScrollState } from './hooks/useScrollState';

export function App() {
  useLenis();
  const { barRef, stuck } = useScrollState();

  return (
    <>
      <a className="skip" href="#main">
        Skip to content
      </a>
      <div className="bar" ref={barRef} aria-hidden="true" />
      <Nav stuck={stuck} />
      <main id="main">
        <Hero />
        <Marquee />
        <Showcase />
        <Features />
        <How />
        <Security />
        <Status />
        <Faq />
        <DownloadSection />
        <MadeBy />
        <MoreFrom />
      </main>
      <Footer />
    </>
  );
}
