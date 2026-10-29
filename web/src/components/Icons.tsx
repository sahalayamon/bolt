// Platform glyphs (brand shapes are not in lucide).
export function WindowsIcon({ size = 30 }: { size?: number }) {
  return (
    <svg viewBox="0 0 24 24" width={size} height={size} fill="currentColor" aria-hidden="true">
      <path d="M3 5.5 11 4.4v7H3zM12 4.2 21 3v8.4h-9zM3 12.6h8v7l-8-1.1zM12 12.6h9V21l-9-1.2z" />
    </svg>
  );
}

export function AndroidIcon({ size = 30 }: { size?: number }) {
  return (
    <svg viewBox="0 0 24 24" width={size} height={size} fill="currentColor" aria-hidden="true">
      <path d="M7 18a1 1 0 0 0 1 1h1v3a1.5 1.5 0 0 0 3 0v-3h0v3a1.5 1.5 0 0 0 3 0v-3h1a1 1 0 0 0 1-1V9H7zM4.5 9A1.5 1.5 0 0 0 3 10.5v5a1.5 1.5 0 0 0 3 0v-5A1.5 1.5 0 0 0 4.5 9zm15 0a1.5 1.5 0 0 0-1.5 1.5v5a1.5 1.5 0 0 0 3 0v-5A1.5 1.5 0 0 0 19.5 9zM16.8 3.3l1-1.7-.4-.3-1 1.7A6 6 0 0 0 12 2.2c-1.5 0-2.8.3-4.4 1.2l-1-1.7-.4.3 1 1.7A6 6 0 0 0 6 8h12a6 6 0 0 0-1.2-4.7zM9.5 6.2a.8.8 0 1 1 0-1.6.8.8 0 0 1 0 1.6zm5 0a.8.8 0 1 1 0-1.6.8.8 0 0 1 0 1.6z" />
    </svg>
  );
}
