/** The Bolt "B" mark (same artwork as the desktop and Android app icons). */
export function LogoMark({ size = 28 }: { size?: number }) {
  return <img className="logo-mark" src="logo.png" width={size} height={size} alt="" aria-hidden="true" />;
}

export function Brand({ href = '#top', size }: { href?: string; size?: number }) {
  return (
    <a className="brand" href={href} aria-label="Bolt home">
      <LogoMark size={size} />
      <span>Bolt</span>
    </a>
  );
}
