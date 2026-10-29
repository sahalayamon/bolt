# Bolt website

Vite + React 19 + TypeScript (strict) + Lenis smooth scrolling. Same lime accent (#D7F62F) as the apps.

```bash
npm install
npm run dev        # http://localhost:5230
npm run build      # typecheck + static build into dist/
npm run preview    # serve dist/
```

Structure: `src/components/*` (one per section), `src/hooks/*` (Lenis, scroll state, reveal, pointer glow),
`src/data.ts` (copy lists, version, download sizes and SHA-256), `src/styles.css` (design tokens + styles).

`public/downloads/` holds the lime-themed installers (`target/release/bundle`) and the APK
(`apps/android/app/build/outputs/apk/debug`). When you publish a new version, re-copy them and update
`src/data.ts` (size + SHA-256). The binaries are git-ignored.
