# Melody quality checks

## September 2026 cleanup checks

Two saved, separated real recordings were replayed through the Rust cleanup
pipeline using Basic Pitch events and a newly measured pYIN pitch track. These
are regression comparisons, not independently scored transcription accuracy.
No source recording, timestamps or song-specific rules are embedded in Tono.

| Recording | Input to measurement | Before | After |
| --- | --- | --- | --- |
| Ek Pyar, first 54 seconds | Saved `htdemucs_6s` other stem; fresh Basic Pitch events | 74 notes | 74 identical notes |
| Poove Sempoove, selected 119-second region | Saved vocal stem and 513 raw Basic Pitch events | 214 notes | 218 notes |

The existing bleed filter already removed Ek Pyar's false intro event, leaving
the first note at 6.378 seconds, MIDI 62 (D4). In Poove it removed 143 events
from near-silent sections of the vocal stem. Those results depend on separation
quality; this is not a general detector of which instrument is playing the lead.

The fixes address later rules that could undo good pitch evidence:

- A short, measured semitone ornament survives the vibrato rule.
- An ornament's own strong attack prevents duration-only vibrato absorption.
- A measured silent gap keeps repeated notes separate, even with a weak attack.
- A short note with steady matching pitch survives the duration cutoff.
- Uncertain pitch evidence does not authorize collapsing a semitone excursion.

Four regression tests reproduced failures before the fix. Additional checks
cover uncertain evidence and continued merging across a transcription gap where
the measured tone remains continuous. Existing randomized tests still check
that cleanup produces a non-overlapping melody.

Poove's differences occur around 48.77, 57.84 and 67.02 seconds on the selected
region's timeline. The first two contain strong attacks; the last has sparse
voicing evidence and is conservatively retained. These are candidates for
listening review, not four independently confirmed correct notes.

## Overlaps, harmonics and register outliers (second pass)

Replays used the same saved inputs as above, plus a fresh `htdemucs_6s` run of
the first 54 s of Ek Pyar. "Before" is the cleanup described in the previous
section; "after" adds the rules below.

**Overlapping candidates.** When transcription candidates overlap, the one
whose pitch the lead stem measurably carries now wins over a more confident
one, but only if: the overlap spans at least three pitch-track hops (~70 ms);
pitch evidence is voiced, steady and audible; a candidate lies within 0.25
semitone of the measured centre; and, for candidates an octave apart, at least
85 % of frames are voiced. Only existing candidates can win, so no pitch is
invented. On these recordings the problem was small: Ek Pyar had no conflicting
overlap; Poove had five slices where the confidence winner contradicted
measured pitch while another candidate matched.

With a 0.35-semitone tolerance three Poove segments changed. A CQT harmonic-
salience check (independent of pYIN) supported only one of them:

| Poove region time | Change | CQT salience, after vs before | Kept? |
| --- | --- | --- | --- |
| 49.40-49.63 s | 62 -> 58 | +18.2 dB | yes |
| 42.81-42.99 s | 62 -> 61 | -0.2 dB | no: tolerance tightened to 0.25 |
| 48.77-48.86 s | 60 -> 61 | -2.5 dB | no: tolerance tightened to 0.25 |

Final result: one Poove note segment changes (62 -> 58 at 49.40 s); Ek Pyar is
unchanged by this rule.

Testing exposed the same risk in older rules, now guarded consistently: pitch
evidence no longer re-pitches events shorter than three hops, no longer joins
an attacked fragment too short to measure into its neighbour, and joins
transcribed pitches an octave apart only with the 85 % voicing requirement.

**Register outliers.** In the 6-stem Ek Pyar run a single 116 ms MIDI 86 (D6)
among D4-C#5 notes forced the whole melody and backing down an octave. It was
quiet (~30 dB below loud passages), mostly unvoiced, had a weak attack (0.26)
and its spectrum peaked at D6/D7 rather than the lead's register. A note is now
dropped only when it is at least an octave from both neighbours, at most
150 ms, without a strong attack and unconfirmed by trusted pitch evidence. The
Ek Pyar run then needs no transposition. Poove is unaffected.

**Instrumental lead isolation.** Measured on Ek Pyar (first 54 s):

| Stem (`htdemucs_6s`) | Level (p95) | Loud and voiced before 6.3 s | Loud and voiced 6.4-54 s | Pitch at 6.40-6.60 s |
| --- | --- | --- | --- | --- |
| other | 0 dB | 0 % | 68 % | D4 |
| guitar | -9 dB | 17 % | 2 % | D3 (accompaniment) |
| drums | -15 dB | 1 % | 1 % | - |
| bass, piano, vocals | about -62 to -65 dB | 0 % | 0 % | bleed only |

With 6 stems the pre-lead music sits mostly in `guitar`, which stays in the
backing; the first lead note is 6.378 s, D4, and the only earlier transcribed
event (1.45 s) is dropped as bleed. With the default 4-stem model `other`
contains both lead and guitar, so removing it leaves a backing 19.2 dB below the
mix and Tono refuses the run. That error now suggests
`--separation-model htdemucs-6s`. No automatic stem selection or in-stem lead
masking was added: on the only instrumental test song the chosen stem was
already right and no overlap inside `other` conflicted, so such a change could
not be validated here.

## Remaining limitations

- Measured pitch overrides overlap confidence only in clear cases; short or
  ambiguous overlaps, and weakly voiced octave disagreements, still follow
  transcription confidence. One independently supported change is thin
  evidence; more recordings are needed.
- The register-outlier rule can drop a genuine, quiet, very short leap of an
  octave or more when the pitch tracker cannot confirm it.
- `--part lead` selects the model's whole other stem. It cannot reliably identify
  a solo instrument when several instruments remain in that stem, nor follow a
  lead that moves between instruments. With the 4-stem model an instrumental
  lead usually shares `other` with its accompaniment.
- Separation itself is unchanged. Removing faint false note events does not
  improve lead removal in `backing.wav`.
- The pitch tracker uses roughly 93 ms analysis windows. Timing and voicing
  around fast transitions are uncertain, even though its hop is about 23 ms.
- Thresholds still need more real recordings and musician review. Fewer notes
  alone are not evidence of a better transcription.

MIDI, MusicXML and JSON imports retain their existing behaviour: imported notes
skip audio transcription and cleanup.
