import offer from '../assets/sounds/offer.wav';
import done from '../assets/sounds/done.wav';
import fail from '../assets/sounds/fail.wav';
import text from '../assets/sounds/text.wav';
import pair from '../assets/sounds/pair.wav';

/** The moments Bolt makes a sound for. Replace the files in `assets/sounds` to change how they sound. */
export type SoundName = 'offer' | 'done' | 'fail' | 'text' | 'pair';

const FILES: Record<SoundName, string> = { offer, done, fail, text, pair };
const KEY = 'bolt.sounds';
const VOLUME = 0.6;

/** Sound effects are on unless the user turned them off (a per-computer preference, not part of the saved settings). */
export function soundsOn(): boolean {
  try {
    return localStorage.getItem(KEY) !== 'off';
  } catch {
    return true;
  }
}

export function setSoundsOn(on: boolean): void {
  try {
    localStorage.setItem(KEY, on ? 'on' : 'off');
  } catch {
    /* storage unavailable: the choice just won't be remembered */
  }
}

const cache = new Map<SoundName, HTMLAudioElement>();

export function playSound(name: SoundName): void {
  if (!soundsOn() || typeof Audio === 'undefined') return;
  // jsdom (the unit tests) has no audio output
  if (typeof navigator !== 'undefined' && /jsdom/i.test(navigator.userAgent)) return;
  try {
    let a = cache.get(name);
    if (!a) {
      a = new Audio(FILES[name]);
      a.volume = VOLUME;
      cache.set(name, a);
    }
    a.currentTime = 0;
    void a.play().catch(() => {});
  } catch {
    /* no audio device: stay silent */
  }
}
