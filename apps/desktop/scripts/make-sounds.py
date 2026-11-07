"""Generates Bolt's placeholder sound effects (short soft blips) as 16-bit mono WAV files.

Run:  python apps/desktop/scripts/make-sounds.py
Writes the same five files for the desktop app (apps/desktop/src/assets/sounds) and the Android app
(apps/android/app/src/main/res/raw). To use your own sounds, just replace those files (keep the names).
"""
import math
import struct
import wave
from pathlib import Path

RATE = 44100
ROOT = Path(__file__).resolve().parents[3]
OUT = [ROOT / "apps/desktop/src/assets/sounds", ROOT / "apps/android/app/src/main/res/raw"]


def tone(freq, ms, gain=0.5, harmonic=0.18):
    n = int(RATE * ms / 1000)
    out = []
    for i in range(n):
        t = i / RATE
        attack = min(1.0, i / (RATE * 0.006))
        decay = math.exp(-5.0 * i / n)
        s = math.sin(2 * math.pi * freq * t) + harmonic * math.sin(2 * math.pi * freq * 2 * t)
        out.append(gain * attack * decay * s / (1 + harmonic))
    return out


def silence(ms):
    return [0.0] * int(RATE * ms / 1000)


def seq(*parts):
    out = []
    for p in parts:
        out += p
    return out


SOUNDS = {
    # someone wants to send you something: two rising notes
    "offer": seq(tone(660, 90), silence(15), tone(880, 130)),
    # transfer finished: bright rising triad
    "done": seq(tone(523, 70), silence(10), tone(659, 70), silence(10), tone(784, 70), silence(10), tone(1047, 190)),
    # failed or declined: two falling low notes
    "fail": seq(tone(330, 130, 0.55), silence(20), tone(247, 220, 0.55)),
    # text or link received: one short blip and a soft echo
    "text": seq(tone(1047, 70), silence(25), tone(1319, 55, 0.25)),
    # pairing request / paired: two equal notes, then a higher one
    "pair": seq(tone(784, 75), silence(40), tone(784, 75), silence(40), tone(1175, 180)),
}


def write(path: Path, samples):
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(b"".join(struct.pack("<h", int(max(-1.0, min(1.0, s)) * 32767)) for s in samples))


for d in OUT:
    d.mkdir(parents=True, exist_ok=True)
    for name, samples in SOUNDS.items():
        write(d / f"{name}.wav", samples)
print("wrote", ", ".join(SOUNDS), "to", ", ".join(str(d) for d in OUT))
