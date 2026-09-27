"""Reviewed acoustic wind charts and deterministic ukulele positions.
Imported chart vectors were checked against rendered Yamaha PDFs. Runtime needs
only the generated JSON; regeneration uses the standard library, not PDF tools.
"""

def make_profile(name, variants, layout, settings, sources, **metadata):
    return dict(version=1, instrument=name, verified=False,
                required_settings=settings, sources=sources, layout_keys=layout,
                fingerings={str(m):dict(octave="normal",keys=v[0]) for m,v in sorted(variants.items())},
                alternatives={str(m):[dict(octave="normal",keys=k) for k in v[1:]]
                              for m,v in sorted(variants.items()) if len(v)>1}, **metadata)

def layout_keys(rows):
    return [dict(id=k,label=label,x=x,y=y,shape=shape) for k,label,x,y,shape in rows]

def build(write):
    # Facts transcribed from Violin Online's first-position chart, cross-checked
    # against Yamaha's natural-note finger diagram. No extensions or shifts.
    violin_tuning = [76, 69, 62, 55]  # E5 A4 D4 G3, string 1 first
    violin_fingers = [[0,1,1,2,2,3,4,4], [0,1,1,2,2,3,4,4],
                      [0,1,1,2,2,3,3,4], [0,1,1,2,2,3,3,4]]
    variants = {}
    for midi in range(55,84):
        choices = sorted((midi-note,string,violin_fingers[string-1][midi-note])
                         for string,note in enumerate(violin_tuning,1) if 0<=midi-note<=7)
        variants[midi] = [[f"s{string}_p{offset}_n{finger}"] for offset,string,finger in choices]
    write("violin",make_profile("violin",variants,[],
          "G3 D4 A4 E5 tuning; first position; single melody; bowing not prescribed",
          [dict(url="https://www.violinonline.com/fingeringchart.html",
                method="first-position chart: string, sounding pitch and finger number; alternatives retained"),
           dict(url="https://www.yamaha.com/en/musical_instrument_guide/violin/play/play003.html",
                method="natural-note first-position finger diagram and fingers 1-4")],
          renderer="violin",tuning_midi=violin_tuning,fret_count=0,
          coverage="G3-B5, MIDI 55-83; first position only, not the full violin range",
          key_semantics="s1=E, s2=A, s3=D, s4=G; p is semitones above open, NOT a fret; n is finger 0-4 (0=open). Only the sounding finger is prescribed; supporting fingers are not specified.",
          selection="Prefer the lowest semitone offset (open strings before fourth-finger alternatives); deterministic, not phrase-optimized."))

    for name,tuning in [("ukulele",[69,64,60,67]),("ukulele-low-g",[69,64,60,55]),
                        ("ukulele-baritone",[64,59,55,50])]:
        variants={}
        for midi in range(min(tuning),max(tuning)+13):
            positions=sorted((midi-note,string) for string,note in enumerate(tuning,1)
                             if 0<=midi-note<=12)
            variants[midi]=[[f"s{string}_f{fret}"] for fret,string in positions]
        tuning_label={"ukulele":"G4 C4 E4 A4 (high G)","ukulele-low-g":"G3 C4 E4 A4 (low G)",
                      "ukulele-baritone":"D3 G3 B3 E4"}[name]
        write(name,make_profile(name,variants,[],f"{tuning_label}; no capo; frets 0-12; single-note melody",
              [dict(url="https://kalabrand.com/blogs/home/ukulele-tuning-decoded",
                    method="documented tuning plus one semitone per fret; string 1 is A4/E4")],
              renderer="guitar",tuning_midi=tuning,fret_count=12,
              coverage="Conservative first 12 frets; all positions retained, lowest-fret primary"))
    layout=layout_keys([
        ("0","0",530,250,"circle"),("0_vent","0 vent",530,250,"circle"),
        ("1","1",300,210,"circle"),("2","2",300,285,"circle"),("3","3",300,360,"circle"),
        ("4","4",300,490,"circle"),("5","5",300,565,"circle"),
        ("6a","6a",281,650,"circle"),("6b","6b",322,665,"circle"),
        ("7a","7a",271,745,"circle"),("7b","7b",312,760,"circle")])
    for mode,variants in [("baroque",BAROQUE),("german",GERMAN)]:
        name="recorder-"+mode
        write(name,make_profile(name,variants,layout,f"Soprano in C; {mode.title()} fingering; double holes 6/7",
              [dict(url=f"https://www.yamaha.com/en/musical_instrument_guide/common/images/recorder/fingering_{mode}.pdf",
                    pages=[1],method="reviewed red chart dots and alternatives; treble clef 8 means sounding one octave higher")],
              renderer="recorder",coverage="Sounding C5-D7, MIDI 72-98",
              key_semantics="0 is rear thumb; 0_vent leaves about 1/4 of thumb hole open; 6a/7a are larger double holes, 6b/7b smaller. A vent is not an extra physical key."))
    controls=[("1","1",140,150,"circle"),("2","2",220,150,"circle"),("3","3",300,150,"circle"),
              ("Gs","G#",340,80,"rect"),("4","4",440,150,"circle"),("5","5",520,150,"circle"),
              ("6","6",600,150,"circle"),("Eb","Eb",680,105,"rect"),("Cs","C#",680,185,"rect"),
              ("C","C",750,185,"rect"),("thumb_b","B",140,280,"rect"),
              ("thumb_bb","Bb",220,280,"rect"),("trill1","T1",455,225,"rect"),
              ("trill2","T2",540,225,"rect")]
    cues=[(f"register_{r}",r,0,0,"rect") for r in ["low","middle","high"]]
    for name in ["flute","flute-bfoot"]:
        raw={m:[list(k) for k in v] for m,v in FLUTE.items()}
        extra=[]; sources=[dict(url="https://www.yamaha.com/en/musical_instrument_guide/common/images/flute/fingering.pdf",
                              pages=[1],method="reviewed red chart keys and all charted alternatives; concert pitch")]
        if name=="flute-bfoot":
            raw[59]=[["1","2","3","4","5","6","C","Cs","B","thumb_b"]]
            extra=[("B","B foot",750,255,"rect")]
            sources.append(dict(url="https://usa.yamaha.com/files/download/other_assets/3/335903/piccolo_flute_en_om_a0.pdf",
                                pages=[16],method="starred low B fingering, only for B footjoint"))
        variants={m:[keys+["register_"+("low" if m<72 else "middle" if m<86 else "high")]
                      for keys in patterns] for m,patterns in raw.items()}
        write(name,make_profile(name,variants,layout_keys(controls+extra+cues),
              f"Concert C flute; {'B' if name=='flute-bfoot' else 'C'} footjoint; standard Boehm keys; seal open holes fully",
              sources,renderer="flute",coverage=f"Sounding MIDI {59 if name=='flute-bfoot' else 60}-96; charted fingerings only",
              key_semantics="1-3 left fingers, 4-6 right fingers; rear B/Bb thumb levers; foot C#/C/B and Eb. T1/T2 are trill levers. register_* are air/embouchure cues, NOT physical keys."))

BAROQUE = {72: [['0', '1', '2', '3', '4', '5', '6a', '6b', '7a', '7b']],
 73: [['0', '1', '2', '3', '4', '5', '6a', '6b', '7a']],
 74: [['0', '1', '2', '3', '4', '5', '6a', '6b']],
 75: [['0', '1', '2', '3', '4', '5', '6a']],
 76: [['0', '1', '2', '3', '4', '5']],
 77: [['0', '1', '2', '3', '4', '6a', '6b', '7a', '7b']],
 78: [['0', '1', '2', '3', '5', '6a', '6b']],
 79: [['0', '1', '2', '3']],
 80: [['0', '1', '2', '4', '5', '6a']],
 81: [['0', '1', '2']],
 82: [['0', '1', '3', '4']],
 83: [['0', '1'], ['0', '2', '3']],
 84: [['0', '2']],
 85: [['1', '2'], ['0']],
 86: [['2']],
 87: [['2', '3', '4', '5', '6a', '6b']],
 88: [['0_vent', '1', '2', '3', '4', '5']],
 89: [['0_vent', '1', '2', '3', '4', '6a', '6b']],
 90: [['0_vent', '1', '2', '3', '5']],
 91: [['0_vent', '1', '2', '3']],
 92: [['0_vent', '1', '2', '4']],
 93: [['0_vent', '1', '2']],
 94: [['0_vent', '1', '2', '4', '5', '6a', '6b']],
 95: [['0_vent', '1', '2', '4', '5'], ['0_vent', '1', '5', '6a', '6b', '7a', '7b']],
 96: [['0_vent', '1', '4', '5']],
 97: [['0_vent', '1', '3', '4', '5', '7a', '7b']],
 98: [['0_vent', '1', '3', '4', '6a', '6b']]}

GERMAN = {72: [['0', '1', '2', '3', '4', '5', '6a', '6b', '7a', '7b']],
 73: [['0', '1', '2', '3', '4', '5', '6a', '6b', '7a']],
 74: [['0', '1', '2', '3', '4', '5', '6a', '6b']],
 75: [['0', '1', '2', '3', '4', '5', '6a']],
 76: [['0', '1', '2', '3', '4', '5']],
 77: [['0', '1', '2', '3', '4']],
 78: [['0', '1', '2', '3', '5', '6a', '6b', '7a', '7b']],
 79: [['0', '1', '2', '3']],
 80: [['0', '1', '2', '4', '5', '6a']],
 81: [['0', '1', '2']],
 82: [['0', '1', '3', '4']],
 83: [['0', '1'], ['0', '2', '3']],
 84: [['0', '2']],
 85: [['1', '2'], ['0']],
 86: [['2']],
 87: [['2', '3', '4', '5', '6a', '6b']],
 88: [['0_vent', '1', '2', '3', '4', '5']],
 89: [['0_vent', '1', '2', '3', '4']],
 90: [['0_vent', '1', '2', '3', '5', '7a', '7b'], ['0_vent', '1', '2', '3', '5', '6a']],
 91: [['0_vent', '1', '2', '3']],
 92: [['0_vent', '1', '2', '3', '5', '6a', '6b', '7a', '7b']],
 93: [['0_vent', '1', '2']],
 94: [['0_vent', '1', '2', '4', '5', '6a', '6b']],
 95: [['0_vent', '1', '2', '4', '5']],
 96: [['0_vent', '1', '4', '5']],
 97: [['0_vent', '1', '3', '4', '6a', '6b', '7a', '7b']],
 98: [['0_vent', '1', '3', '4', '6a', '6b']]}

FLUTE = {60: [['1', '2', '3', '4', '5', '6', 'C', 'Cs', 'thumb_b']],
 61: [['1', '2', '3', '4', '5', '6', 'Cs', 'thumb_b']],
 62: [['1', '2', '3', '4', '5', '6', 'thumb_b']],
 63: [['1', '2', '3', '4', '5', '6', 'Eb', 'thumb_b']],
 64: [['1', '2', '3', '4', '5', 'Eb', 'thumb_b']],
 65: [['1', '2', '3', '4', 'Eb', 'thumb_b']],
 66: [['1', '2', '3', '6', 'Eb', 'thumb_b'], ['1', '2', '3', '5', 'Eb', 'thumb_b']],
 67: [['1', '2', '3', 'Eb', 'thumb_b']],
 68: [['1', '2', '3', 'Eb', 'Gs', 'thumb_b']],
 69: [['1', '2', 'Eb', 'thumb_b'], ['1', '2', '4', '6', 'Eb', 'Gs', 'thumb_b']],
 70: [['1', '4', 'Eb', 'thumb_b'], ['1', 'Eb', 'thumb_bb']],
 71: [['1', 'Eb', 'thumb_b']],
 72: [['1', 'Eb'], ['1', '2', '3', '4', '5', '6', 'C', 'Cs']],
 73: [['Eb'], ['2', '3', '4', '5', '6', 'Cs']],
 74: [['2', '3', '4', '5', '6', 'thumb_b']],
 75: [['2', '3', '4', '5', '6', 'Eb', 'thumb_b']],
 76: [['1', '2', '3', '4', '5', 'Eb', 'thumb_b']],
 77: [['1', '2', '3', '4', 'Eb', 'thumb_b']],
 78: [['1', '2', '3', '6', 'Eb', 'thumb_b'], ['1', '2', '3', '5', 'Eb', 'thumb_b']],
 79: [['1', '2', '3', 'Eb', 'thumb_b']],
 80: [['1', '2', '3', 'Eb', 'Gs', 'thumb_b']],
 81: [['1', '2', 'Eb', 'thumb_b'], ['1', '2', '4', '6', 'Eb', 'Gs', 'thumb_b']],
 82: [['1', '4', 'Eb', 'thumb_b'], ['1', 'Eb', 'thumb_bb']],
 83: [['1', 'Eb', 'thumb_b']],
 84: [['1', 'Eb'], ['1', '2', '3', '4', '5', '6', 'C', 'Cs']],
 85: [['Eb'], ['2', '3', '4', '5', '6', 'Cs']],
 86: [['2', '3', 'Eb', 'thumb_b']],
 87: [['1', '2', '3', '4', '5', '6', 'Eb', 'Gs', 'thumb_b']],
 88: [['1', '2', '4', '5', 'Eb', 'thumb_b']],
 89: [['1', '3', '4', 'Eb', 'thumb_b']],
 90: [['1', '3', '6', 'Eb', 'thumb_b'], ['1', '3', '6', 'Cs', 'thumb_b']],
 91: [['1', '2', '3', 'Eb']],
 92: [['2', '3', 'Eb', 'Gs'], ['2', '3', '5', '6', 'Eb', 'Gs']],
 93: [['2', '4', 'Eb', 'thumb_b'], ['2', '4', 'C', 'Cs', 'thumb_b']],
 94: [['4', 'thumb_b', 'trill1'], ['1', '3', '6', 'thumb_bb', 'trill1']],
 95: [['1', '3', 'thumb_b', 'trill2'], ['1', '3', '6', 'Eb', 'thumb_b', 'trill1', 'trill2']],
 96: [['1', '2', '3', '4', 'Gs'], ['1', '2', '3', '4', '5', 'Gs', 'trill2']]}
