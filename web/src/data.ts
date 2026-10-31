/** Where releases live. Files are attached by .github/workflows/release.yml when a `v*` tag is pushed. */
export const REPO = 'https://github.com/sahalayamon/bolt';
const LATEST = `${REPO}/releases/latest/download`;

/** Shown in the download heading. */
export const VERSION = 'v5';

export interface DownloadFile {
  href: string;
  /** Approximate size, so the label never goes stale between releases. */
  size: string;
}

export const WINDOWS_EXE: DownloadFile = { href: `${LATEST}/Bolt-windows-x64-setup.exe`, size: '~5 MB' };
export const WINDOWS_MSI: DownloadFile = { href: `${LATEST}/Bolt-windows-x64.msi`, size: '~7 MB' };
export const ANDROID_APK: DownloadFile = { href: `${LATEST}/Bolt-android.apk`, size: '~45 MB' };
export const CHECKSUMS_URL = `${LATEST}/SHA256SUMS.txt`;
export const RELEASES_URL = `${REPO}/releases/latest`;

export const NAV_LINKS = [
  { href: '#app', label: 'App' },
  { href: '#features', label: 'Features' },
  { href: '#how', label: 'How it works' },
  { href: '#security', label: 'Security' },
  { href: '#status', label: 'Status' },
  { href: '#faq', label: 'FAQ' },
  { href: '#about', label: 'About' },
] as const;

export const MARQUEE = [
  'No internet needed',
  'QUIC + TLS 1.3',
  'BLAKE3 verified',
  'Folders & huge files',
  'No accounts',
  'No telemetry',
  'QR pairing',
] as const;

export const STEPS = [
  { n: '01', title: 'Open Bolt on both', body: 'On the same Wi-Fi, a hotspot, or a USB tether. Nearby devices appear automatically.' },
  { n: '02', title: 'Pair once', body: 'Scan the QR code and confirm the same 6-digit code shows on both screens. Pairing codes are single-use and expire fast.' },
  { n: '03', title: 'Send, accept, done', body: 'Drop files on a device, tap Accept on the other side. Bolt shows real progress and only says “done” after verification.' },
] as const;

export const SECURITY = [
  { title: 'Encrypted in transit', body: 'QUIC with TLS 1.3 and mutual device certificates.' },
  { title: 'Identity you can verify', body: 'Devices are pinned by key. A QR plus a shared 6-digit code defeats impostors.' },
  { title: 'Hostile-file safe', body: 'Path traversal, reserved names, symlink escapes and oversize messages are rejected.' },
  { title: 'Nothing runs by itself', body: 'Received files are never executed or auto-opened.' },
] as const;

export const VERIFIED = [
  'Engine tests and a real two-process run',
  'Windows app transfers with the command-line client',
  'Android app on an emulator',
] as const;

export const PENDING = [
  'Long Wi-Fi transfers between a real phone and PC',
  'Wi-Fi Direct (not built yet)',
  'Signed installers (current builds are unsigned)',
] as const;

/** The "Made by" card. Edit these lines; the photo is public/sahal.jpg (swap the file for a sharper one). */
export const CREATOR = {
  name: 'Sahal Ayamon',
  role: 'Developer & designer',
  photo: 'sahal.jpg',
  bio: 'Designed and built Bolt end to end: the Windows and Android apps, the transfer engine and this site.',
  quote: 'Moving a file from your PC to your phone should be instant, private and work without the internet.',
} as const;

/** Left column of the About section. */
export const ABOUT = {
  title: ['Built for the way you', 'actually share.'] as const,
  paragraphs: [
    'Bolt is a simple file sharing app with one idea behind it: your files should reach the other device directly, without a cloud, an account or an internet connection in between.',
    'So it finds your devices on its own, encrypts everything, checks every file before it says done, and asks before accepting anything from a device you have not paired.',
  ],
} as const;

/** The "More from" link above the footer. */
export const MORE_FROM = { brand: 'AYMN', host: 'aymn.in', href: 'https://aymn.in' } as const;

/** "See it" section next to the phone screenshot (public/phone-home.jpg). */
export const SHOWCASE = {
  title: ['Open it.', 'Pick a device.'] as const,
  lead: 'This is the whole Android home screen. Nearby devices appear on their own, and what you want to send is one tap away.',
  points: [
    { title: 'Devices show up by themselves', body: 'Your PC appears as soon as both are on the same network. Nothing to type, nothing to sign in to.' },
    { title: 'Files, Photos or Folder', body: 'Choose what to send with the system pickers. Whole folders keep their structure.' },
    { title: 'Or use an address', body: 'If a network blocks discovery, tap Address and enter the PC’s IP. It still works.' },
  ],
} as const;

export const FAQ = [
  {
    q: 'Does Bolt need the internet?',
    a: 'No. Bolt never uses the internet or a cloud. Both devices only need a local connection between them: the same Wi-Fi, a phone hotspot, or USB tethering.',
  },
  {
    q: 'Do I need an account?',
    a: 'No. There is no sign-up, no sign-in and no telemetry. Devices recognise each other by pairing once with a QR code and a 6-digit check.',
  },
  {
    q: 'Are my files safe in transit?',
    a: 'Transfers are encrypted with QUIC and TLS 1.3, and the receiver verifies every file with BLAKE3 before it is saved. Unknown devices cannot send you anything without you accepting.',
  },
  {
    q: 'My devices can’t see each other. What now?',
    a: 'Some networks, like guest Wi-Fi or routers with client isolation, block discovery. Use “Send to an IP address”, pair with the QR code, or put both devices on one phone’s hotspot.',
  },
  {
    q: 'Does it work with big files and folders?',
    a: 'Yes. Files are streamed instead of loaded into memory, folders keep their structure, and an interrupted transfer can resume from the verified partial.',
  },
  {
    q: 'Why does Windows show a warning when I install it?',
    a: 'The current builds are not code-signed yet, so SmartScreen warns about an unknown publisher. Choose More info, then Run anyway. On Android, allow “install unknown apps” for the app you download it with.',
  },
  {
    q: 'Does it support Wi-Fi Direct, iPhone, Mac or Linux?',
    a: 'Not yet. Today Bolt is for Windows and Android, and both devices need to be on the same network, hotspot or USB tether.',
  },
] as const;
