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

## Validation on additional recordings (third pass)

Six further recordings from the user's local collection, none used for tuning,
60-second windows, run end to end with the committed cleanup (cb7887f), then
replayed with the change below on identical raw notes and pitch tracks. There
is no reference score for any of them: the checks use evidence independent of
pYIN (constant-Q harmonic salience and peak-to-floor harmonicity of the lead
and backing stems) plus listening clips. "Agree" means a kept note's pitch is
the lead stem's dominant CQT pitch (or its octave, a known CQT-comb ambiguity).

| Recording (window) | Material | Kept notes before -> after | Agree before -> after | Audible dominant-pitch notes dropped before -> after |
| --- | --- | --- | --- | --- |
| Galliyan, unplugged (0:40-1:40) | male vocal, guitar | 142 -> 143 | 97 % -> 97 % | 0 -> 0 |
| Babam Bam (0:30-1:30) | male vocal, fast melisma | 110 -> 151 | 94 % -> 93 % | 39 -> 7 |
| Candle in the Wind cover (0:20-1:20) | male vocal, piano | 104 -> 111 | 97 % -> 98 % | 9 -> 1 |
| Gayatri Mantra (0:30-1:30) | chant, repeated notes | 96 -> 101 | 86 % -> 88 % | 22 -> 15 |
| Tarararara trumpet (0:00-1:00), `--part lead`, 6 stems | trumpet, fast repeats | 135 -> 134 | 58 % -> 60 % | 17 -> 13 |
| Shiva fusion theme (0:30-1:30), `--part lead`, 6 stems | instrumental mix | 216 -> 216 | 64 % -> 64 % | 67 -> 63 |

**Demonstrated failure: the unpitched rule deleted real fast notes.** Almost
every audible dominant-pitch note that cleanup removed came from the
"unpitched" rule (<= 120 ms, < 20 % frames with voicing probability >= 0.5).
In Babam Bam those 38 notes were as harmonic as the notes Tono kept (median
peak-to-floor 34.1 dB vs 32.8 dB). Inside one of them (Basic Pitch MIDI 60,
confidence 0.72, attack 0.93) pYIN decoded f0 60.09 on almost every frame
while its voicing probability stayed 0.01-0.37. The rule now drops a short
note as unpitched only if pYIN's decoded f0 also fails to match the note's
pitch (within 0.5 semitone) on at least half its frames. Genuine consonants and
breath decode no matching pitch and are still removed.

Of the notes this restores, the share whose pitch is the lead's dominant CQT
pitch: Galliyan 1/1, Babam Bam 46/51 (90 %), Candle 10/10, Gayatri 10/10,
trumpet 7/9, Shiva 4/6. Replayed on the tuning songs: Poove gains 12 notes,
and all 15 changed segments match the dominant CQT pitch, including a 70 ms
ornament. The first pass above treated Poove's short unpitched drops as noise;
most were real notes. Ek Pyar gains one 81 ms E4 where pYIN shows the pitch
reaching E4 about 60 ms before the transcribed E4; because that transcribed
E4 has its own strong attack, the video now shows an extra E4 re-attack.

**Coverage and what the numbers do not show.**

- Vocals (4 recordings) and fast ornaments (Babam Bam, Poove) are covered.
  Cleanup keeps 93-98 % dominant-pitch agreement on three of them; the chant
  is lower (88 %).
- Instrumental leads (trumpet, fusion) are covered but poor: a third or more
  of kept notes are not the lead stem's dominant pitch, and the fusion
  melody is barely harmonic (22 dB). `other` holds several instruments;
  cleanup cannot recover a melody that separation did not isolate. The
  trumpet has 33 octave-or-larger jumps, only 10 with both notes spectrally
  supported: accompaniment and harmonics remain in instrumental melodies.
- Repeated notes occur in every recording (10-52 same-pitch neighbours), but
  without a score, whether each is a genuine re-attack needs listening,
  especially the trumpet's fast repeated tonguing.
- Genuine sung octave jumps: one example (Candle, song time ~54.6 s, MIDI
  49 -> 61), kept, both notes supported. Too little to claim coverage.
- The register-outlier rule removed 21 (trumpet) and 25 (fusion) events; they
  are less harmonic than kept notes (23.4 vs 30.0 dB; 19.8 vs 22.1 dB) and
  were not flagged as audible dominant pitches, so no real-note loss was
  demonstrated there.
- Agreement with a CQT dominant pitch is supporting evidence, not ground
  truth; it can follow a loud accompaniment in a mixed stem.

Listening material: `tono-practices/melody-validation-20260930/ab-unpitched-rescue/`
holds before/after pairs around every changed spot (left: separated lead;
right: notes as tones; file names carry the original song time).

## Repeated same-pitch notes: split or re-attack? (fourth pass)

Prompted by Ek Pyar's extra E4 re-attack, every pair of adjacent same-pitch
notes (gap <= 150 ms) in the current output of eight recordings was measured
at its boundary on the separated lead: level dip at 6 ms resolution, timbre
change (MFCC distance 60 ms either side, compared with the drift inside one
note of that recording), pYIN voicing through the boundary, and the second
event's transcriber attack.

| Evidence | Pairs (of 161) |
| --- | --- |
| Converging re-attack: dip >= 9 dB and timbre change above in-note drift | 27 |
| Converging continuous split: no dip, stable timbre, voiced throughout, no gap | 3 |
| Ambiguous: the measures disagree or are inconclusive | 131 |

The three continuous-looking pairs (Galliyan ~55.75 s and ~90.94 s, Poove
~29.36 s, song time) stay split because the second event's attack is 0.76-0.82,
just above the 0.75 repeat threshold. The 27 converging re-attacks have attacks
of 0.74-0.90 (median 0.83; 70 % below 0.85). Attack strength therefore cannot
separate the two behaviours: raising the threshold to join the three would
erase most genuine re-articulations. Level dips and timbre separate only the
converging cases; vocal syllable changes on one pitch often show no dip, and
wind/electronic re-tonguing often shows little timbre change.

The Ek Pyar case belongs to 20 pairs where a short first note (<= 120 ms)
precedes the same pitch. All 20 are ambiguous: one has a 17.7 dB dip (clearly
re-attacked), others show no dip with low timbre change, many show large
timbre changes typical of new syllables, and pYIN voicing collapses at almost
every such boundary. Ek Pyar itself shows no dip and timbre change close to its
in-note drift, which leans towards an early onset, but that is one ambiguous
case.

Decision: no merge rule was added. Tono keeps deciding repeats from the
transcriber's attack and measured silence only; the rescued short notes are
unchanged. Regression tests now pin a zero-gap re-attack (kept separate), a
zero-gap split without attack (joined), and the ambiguous short-fragment case
(kept, documented as a limit).

Listening: `tono-practices/melody-validation-20260930/repeated-notes/` has 32
clips (the 3 continuous-looking pairs, 9 converging re-attacks including trumpet
tonguing, and all 20 short-fragment pairs) with `INDEX.md` listing the
measurements and a verdict column. Human labels on these would be the first
real ground truth for this question.

## Remaining limitations

- Measured pitch overrides overlap confidence only in clear cases; short or
  ambiguous overlaps, and weakly voiced octave disagreements, still follow
  transcription confidence. One independently supported change is thin
  evidence; more recordings are needed.
- The register-outlier rule can drop a genuine, quiet, very short leap of an
  octave or more when the pitch tracker cannot confirm it.
- Same-pitch repeats: a continuous note split by the transcriber with a
  moderately strong attack stays two notes (an extra re-attack cue), and
  audio evidence cannot yet tell such splits from quick re-articulations.
- `--part lead` selects the model's whole other stem. It cannot reliably identify
  a solo instrument when several instruments remain in that stem, nor follow a
  lead that moves between instruments. With the 4-stem model an instrumental
  lead usually shares `other` with its accompaniment.
- Separation itself is unchanged. Removing faint false note events does not
  improve lead removal in `backing.wav`.
- The pitch tracker uses roughly 93 ms analysis windows. Timing and voicing
  around fast transitions are uncertain, even though its hop is about 23 ms.
- Thresholds still need more real recordings, a reference score for at least
  one song, and musician review. Fewer notes
  alone are not evidence of a better transcription.

MIDI, MusicXML and JSON imports retain their existing behaviour: imported notes
skip audio transcription and cleanup.
