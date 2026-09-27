# Instrument profiles

These JSON files are Tono runtime data, not prompts or agent skills. Select one
with `--instrument ae01|ae05|ae10|ae20|guitar`. No ML work is needed to load and
validate a profile; invalid or missing data fails before processing the source.

| Profile | Diagram | Sounding MIDI coverage | Required setup |
| --- | --- | --- | --- |
| ae01 | Vertical recorder controls | 47–85 | Recorder, transpose 0 |
| ae05 | Vertical sax controls | 46–85 | Sax, transpose 0, tone octave shift 0 |
| ae10 | Vertical sax controls | 34–97 | Sax, transpose 0, tone octave shift 0, Oct Key OCT2 |
| ae20 | Vertical sax controls | 34–97 | Sax, transpose 0, tone octave shift 0, Octave Key Oct2 |
| guitar | Horizontal fretboard | 40–83 | Six strings, E2 A2 D3 G3 B3 E4, no capo, frets 0–19 |

AE-01 keeps the user's right-hand-upper label preference. Other winds default
to the manuals' left-hand-upper grip. `--upper-hand right|left` changes labels
only; it never mirrors controls or alters fingerings. It does not affect guitar.

## Data and source checks

Each file contains primary `fingerings`, preserved `alternatives`, verification
status, required settings, and source URLs. Wind profiles also contain physical
control IDs and schematic coordinates under `layout_keys` (AE-01 retains its
existing dedicated drawing). Every referenced key must exist in that model's
diagram. Unknown instruments, key IDs and malformed guitar positions are errors.

The new sax tables were visually transcribed from the **Bb3–C#5 segment** in:

- [AE-05 manual](https://static.roland.com/assets/media/pdf/AE-05_eng05_W.pdf): chart p13, key layout and octave behavior p6.
- [AE-10 manual](https://static.roland.com/assets/media/pdf/AE-10_eng03_W.pdf): chart PDF p13, layout p4, octave settings p8.
- [AE-20 manual](https://static.roland.com/assets/media/pdf/AE-20_eng01_W.pdf): chart p22, layout and octave controls p6.

Each model's chart was checked independently. Their chosen base patterns match,
but their controls and octave modes differ. The C5 alternative (1 + Tc) is retained.
`build_profiles.py` deterministically expands this documented subset using ±1
(AE-05) or ±2 (AE-10/20) octave controls. Neutral fingerings are preferred, then
smaller octave displacements. Altissimo, customized fingerings and other modes
are not included; these ranges are **profile coverage, not the instrument's
absolute limits**. No AE-01 patterns are reused as sax fingerings.

Guitar mappings follow [Yamaha's tuning/fret guide](https://www.yamaha.com/en/musical_instrument_guide/acoustic_guitar/mechanism/mechanism002.html):
`pitch = open-string MIDI + fret`. All positions through fret 19 are retained.
The primary uses the lowest fret, breaking ties by string number. This is a
simple deterministic position choice, not an optimized concert arrangement.
The renderer shows a five-fret window, string/fret, open strings and plucking
instructions. It plays the transcribed **single-note melody**, not chords or
strumming. Staff pitches are sounding pitches; extreme registers carry octave
marks so ledger lines stay inside the panel.

`verified` remains false until checked on the corresponding physical instrument.
The JSON `required_settings` are shown in the video and terminal; Tono cannot
read or change settings on a connected instrument.

## Editing and regenerating

- `ae01.json` is the canonical AE-01 profile, including chart corrections and
  octave-key alternatives.
- To update new sax data, check the manual first, edit `SAX_BASE` or the
  model-specific layout in `build_profiles.py`, then run that script.
- The generator also builds `guitar.json` from tuning and fret arithmetic.
- Run `cargo test`; never change key mappings by guesswork or force `verified`.
- Strict range fitting remains the default. A whole-octave shift also shifts
  the backing, and individual folding is disabled.

Examples:

```sh
tono song.mp3 sax.mp4 --instrument ae20 --tempo-scale 0.75 --metronome both
tono song.mp3 guitar.mp4 --instrument guitar --tempo-scale 0.5
tono demo --instrument ae05 --out /tmp/tono-ae05-demo
tono --help
```
