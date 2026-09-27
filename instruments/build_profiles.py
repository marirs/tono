"""Deterministic expansion of sourced front-key tables; never guesses fingerings.
Run from any directory after reviewing the cited manufacturer chart pages.
"""
import json
from pathlib import Path
ROOT = Path(__file__).resolve().parent

# Visually transcribed independently from AE-05 p13, AE-10 p13, AE-20 p22.
# Use the lower, non-altissimo Bb3-C#5 chart segment, then documented octave controls.
SAX_BASE = {
 58: ["1","2","3","4","5","6","C","Bb"],
 59: ["1","2","3","4","5","6","C","B"],
 60: ["1","2","3","4","5","6","C"],
 61: ["1","2","3","4","5","6","Cs"],
 62: ["1","2","3","4","5","6"],
 63: ["1","2","3","4","5","6","Eb"],
 64: ["1","2","3","4","5"], 65: ["1","2","3","4"],
 66: ["1","2","3","5"], 67: ["1","2","3"],
 68: ["1","2","3","Gs"], 69: ["1","2"],
 70: ["1","2","Ta"], 71: ["1"], 72: ["2"], 73: []
}
SAX_ALTERNATIVES = {72: [["1", "Tc"]]}
# Relative control locations from each model's numbered panel diagram.
# Schematic spacing is expanded for legibility; control identity/order is retained.
SAX_KEYS = [
 ("X","X",290,120,"rect"),("1","1",290,190,"circle"),
 ("P","P",290,245,"circle"),("2","2",290,300,"circle"),("3","3",325,355,"circle"),
 ("C2","C2",370,155,"rect"),("C1","C1",415,185,"rect"),("C4","C4",395,270,"rect"),
 ("C3","C3",185,280,"rect"),("Tc","Tc",185,345,"rect"),("Ta","Ta",185,410,"rect"),
 ("Gs","G#",395,415,"rect"),("B","B",360,480,"rect"),("Cs","C#",420,480,"rect"),("Bb","Bb",405,535,"rect"),
 ("4","4",285,510,"circle"),("5","5",285,600,"circle"),("6","6",315,690,"circle"),
 ("Tf","Tf",210,640,"rect"),("Eb","Eb",280,760,"rect"),("C","C",250,815,"rect")
]

def write(name, data):
 (ROOT / (name + ".json")).write_text(json.dumps(data, indent=2) + "\n")

def make_sax(name, manual, chart_page, panel_page, octaves):
 shift_order = [0,1,-1] + ([2,-2] if octaves==2 else [])
 fingerings, alternatives = {}, {}
 for midi in range(58-12*octaves,74+12*octaves):
  variants=[]
  for shift in shift_order:
   base=midi-shift*12
   if base not in SAX_BASE: continue
   for keys in [SAX_BASE[base]]+SAX_ALTERNATIVES.get(base,[]):
    variants.append({"octave": {0:"normal",1:"up",-1:"down",2:"up2",-2:"down2"}[shift],"keys":keys})
  fingerings[str(midi)]=variants[0]
  if len(variants)>1: alternatives[str(midi)]=variants[1:]
 layout=[dict(id=k,label=l,x=x,y=y,shape=s) for k,l,x,y,s in SAX_KEYS]
 if name != "ae10":layout.append(dict(id="C5",label="C5",x=185,y=550,shape="rect"))
 if octaves==2:layout += [dict(id="octave_up2",label="+2",x=555,y=190,shape="octave"),dict(id="octave_down2",label="-2",x=555,y=440,shape="octave")]
 layout += [dict(id="octave_up",label="+1",x=555,y=260,shape="octave"),dict(id="octave_down",label="-1",x=555,y=370,shape="octave")]
 setup="Sax fingering; Transpose 0; tone octave shift 0" + ("; Octave Key OCT2" if name=="ae10" else "; Octave Key Oct2" if name=="ae20" else "")
 return dict(version=1,instrument="roland-"+name,display_name=name.upper().replace("AE","AE-"),verified=False,renderer="sax",required_settings=setup,
  coverage="Bb3-C#5 chart segment plus documented octave controls; altissimo/custom fingerings not included",
  sources=[dict(url=manual,chart_pages=[chart_page],panel_pages=[panel_page],method="Manual chart transcription, deterministic octave expansion")],
  base_fingerings={str(k):v for k,v in SAX_BASE.items()},layout_keys=layout,fingerings=fingerings,alternatives=alternatives)

# AE-BRISA official Fingering Chart multi01, pages 1-2. Key positions were
# read from the vector diagrams and visually checked; do not infer flute patterns
# from Brisa or sax patterns. Thumb left/right follow the chart's rear-key inset.
BRISA_BASE = {'60': ['1', '2', '3', '4', '5', '6', 'foot_c', 'foot_cs'], '61': ['1', '2', '3', '4', '5', '6', 'foot_c'], '62': ['1', '2', '3', '4', '5', '6'], '63': ['1', '2', '3', '4', '5', '6', 'foot_eb'], '64': ['1', '2', '3', '4', '5'], '65': ['1', '2', '3', '4'], '66': ['1', '2', '3', '5'], '67': ['1', '2', '3'], '68': ['1', '2', '3', 'gsharp'], '69': ['1', '2'], '70': ['1', '2', 'trill1'], '71': ['1'], '72': ['2'], '73': []}
BRISA_FLUTE = [{'midi': 60, 'keys': ['1', '2', '3', '4', '5', '6', 'foot_c', 'foot_cs', 'thumb_right'], 'breath': 'low'}, {'midi': 61, 'keys': ['1', '2', '3', '4', '5', '6', 'foot_c', 'thumb_right'], 'breath': 'low'}, {'midi': 62, 'keys': ['1', '2', '3', '4', '5', '6', 'thumb_right'], 'breath': 'low'}, {'midi': 63, 'keys': ['1', '2', '3', '4', '5', '6', 'foot_eb', 'thumb_right'], 'breath': 'low'}, {'midi': 64, 'keys': ['1', '2', '3', '4', '5', 'foot_eb', 'thumb_right'], 'breath': 'low'}, {'midi': 65, 'keys': ['1', '2', '3', '4', 'foot_eb', 'thumb_right'], 'breath': 'low'}, {'midi': 66, 'keys': ['1', '2', '3', '6', 'foot_eb', 'thumb_right'], 'breath': 'low'}, {'midi': 67, 'keys': ['1', '2', '3', 'foot_eb', 'thumb_right'], 'breath': 'low'}, {'midi': 68, 'keys': ['1', '2', '3', 'foot_eb', 'gsharp', 'thumb_right'], 'breath': 'low'}, {'midi': 69, 'keys': ['1', '2', 'foot_eb', 'thumb_right'], 'breath': 'low'}, {'midi': 70, 'keys': ['1', '4', 'foot_eb', 'thumb_right'], 'breath': 'low'}, {'midi': 71, 'keys': ['1', 'foot_eb', 'thumb_right'], 'breath': 'low'}, {'midi': 72, 'keys': ['1', 'foot_eb'], 'breath': 'low'}, {'midi': 73, 'keys': ['foot_eb'], 'breath': 'low'}, {'midi': 72, 'keys': ['1', '2', '3', '4', '5', '6', 'foot_c', 'foot_cs', 'thumb_right'], 'breath': 'upper'}, {'midi': 73, 'keys': ['1', '2', '3', '4', '5', '6', 'foot_c', 'thumb_right'], 'breath': 'upper'}, {'midi': 74, 'keys': ['1', '2', '3', '4', '5', '6', 'thumb_right'], 'breath': 'upper'}, {'midi': 75, 'keys': ['1', '2', '3', '4', '5', '6', 'foot_eb', 'thumb_right'], 'breath': 'upper'}, {'midi': 76, 'keys': ['1', '2', '3', '4', '5', 'foot_eb', 'thumb_right'], 'breath': 'upper'}, {'midi': 77, 'keys': ['1', '2', '3', '4', 'foot_eb', 'thumb_right'], 'breath': 'upper'}, {'midi': 78, 'keys': ['1', '2', '3', '6', 'foot_eb', 'thumb_right'], 'breath': 'upper'}, {'midi': 79, 'keys': ['1', '2', '3', 'foot_eb', 'thumb_right'], 'breath': 'upper'}, {'midi': 80, 'keys': ['1', '2', '3', 'foot_eb', 'gsharp', 'thumb_right'], 'breath': 'upper'}, {'midi': 81, 'keys': ['1', '2', 'foot_eb', 'thumb_right'], 'breath': 'upper'}, {'midi': 82, 'keys': ['1', '4', 'foot_eb', 'thumb_right'], 'breath': 'upper'}, {'midi': 83, 'keys': ['1', 'foot_eb', 'thumb_right'], 'breath': 'upper'}, {'midi': 84, 'keys': ['1', 'foot_eb'], 'breath': 'upper'}, {'midi': 85, 'keys': ['foot_eb'], 'breath': 'upper'}, {'midi': 86, 'keys': ['2', '3', 'foot_eb', 'thumb_right'], 'breath': 'upper'}, {'midi': 87, 'keys': ['1', '2', '3', '4', '5', '6', 'foot_eb', 'gsharp', 'thumb_right'], 'breath': 'upper'}, {'midi': 88, 'keys': ['1', '2', '4', '5', 'foot_eb', 'thumb_right'], 'breath': 'upper'}, {'midi': 89, 'keys': ['1', '3', '4', 'foot_eb', 'thumb_right'], 'breath': 'upper'}, {'midi': 90, 'keys': ['1', '3', '6', 'foot_eb', 'thumb_right'], 'breath': 'upper'}, {'midi': 91, 'keys': ['1', '2', '3', 'foot_eb'], 'breath': 'upper'}, {'midi': 92, 'keys': ['2', '3', 'foot_eb', 'gsharp'], 'breath': 'upper'}, {'midi': 93, 'keys': ['2', '4', 'foot_eb', 'thumb_right'], 'breath': 'upper'}, {'midi': 94, 'keys': ['4', 'thumb_right', 'trill2'], 'breath': 'upper'}, {'midi': 95, 'keys': ['1', '3', 'thumb_right', 'trill3'], 'breath': 'upper'}, {'midi': 96, 'keys': ['1', '2', '3', '4', 'gsharp'], 'breath': 'upper'}]

# Independently reviewed YDS-120 and YDS-150 manuals, printed pp. 20-21.
# These are WRITTEN pitches. Profiles require a zero-transposition voice so
# written pitch equals sounding MIDI; factory alto/tenor voices do NOT match.
# Oct is a physical charted key, never an extrapolated +/-12 range extender.
YDS_LOW = {
 57: ["1","2","3","4","5","6","C","Bb","low_a"],
 58: ["1","2","3","4","5","6","C","Bb"],
 59: ["1","2","3","4","5","6","C","B"],
 60: ["1","2","3","4","5","6","C"],
 61: ["1","2","3","4","5","6","Cs"],
 62: ["1","2","3","4","5","6"],
 63: ["1","2","3","4","5","6","Eb"],
 64: ["1","2","3","4","5"], 65: ["1","2","3","4"],
 66: ["1","2","3","5"], 67: ["1","2","3"],
 68: ["1","2","3","Gs"], 69: ["1","2"],
 70: ["1","2","Ta"], 71: ["1"], 72: ["2"], 73: []
}
YDS_ALTERNATIVES = {
 66: [["1","2","3","4","6","Tf"]],
 70: [["1","P"],["1","4"],["1","5"]],
 72: [["1","Tc"]]
}
YDS_HIGH = {
 86: [["C1"]], 87: [["C1","C2"]],
 88: [["C1","C2","C3"],["X","2","3"]],
 89: [["C1","C2","C3","C4"],["X","2"]],
 90: [["C1","C2","C3","C4","C5"]]
}

def make_yamaha(name, manual):
 variants={}
 for midi,keys in YDS_LOW.items():
  variants[midi]=[keys]+YDS_ALTERNATIVES.get(midi,[])
 # Chart explicitly repeats D4-C#5 patterns with Oct held for D5-C#6.
 for midi in range(62,74):
  variants[midi+12]=[keys+["oct"] for keys in variants[midi]]
 for midi,patterns in YDS_HIGH.items():
  variants[midi]=[keys+["oct"] for keys in patterns]
 # Yamaha panel pp. 8-9 has the same numbered front-control order as this
 # schematic, plus C5, one rear Oct key and a separate rear Low A key.
 layout=[dict(id=k,label="p" if k=="P" else l,x=x,y=y,shape=s) for k,l,x,y,s in SAX_KEYS]
 layout += [dict(id="C5",label="C5",x=185,y=550,shape="rect"),
  dict(id="oct",label="Oct",x=555,y=240,shape="octave"),
  dict(id="low_a",label="Low A",x=555,y=370,shape="octave")]
 def fingering(keys): return dict(octave="normal",keys=keys)
 return dict(version=1,instrument="yamaha-"+name,display_name="Yamaha "+name.upper().replace("YDS","YDS-"),
  verified=False,renderer="sax",layout_keys=layout,
  required_settings="Factory fingering; voice transposition 0 (e.g. C.01); no added pitch/octave shift",
  coverage="Charted A3-F#6, MIDI 57-90, with a concert-C/zero-transposition voice only",
  pitch_reference="Written chart pitch equals sounding pitch ONLY with total voice transposition 0. Factory A/T/S/B sax voices transpose and do not match this profile without reconfiguration.",
  sources=[dict(url=manual,chart_pages=[20,21],panel_pages=[8,9],voice_pages=[19],method="independently reviewed Yamaha chart patterns and alternatives; no uncharted octave expansion")],
  fingerings={str(m):fingering(v[0]) for m,v in sorted(variants.items())},
  alternatives={str(m):[fingering(k) for k in v[1:]] for m,v in sorted(variants.items()) if len(v)>1})

def make_brisa():
 controls=[("1","1",110,150,"circle"),("2","2",190,150,"circle"),("3","3",270,150,"circle"),
 ("gsharp","G#",300,85,"rect"),("4","4",420,150,"circle"),("5","5",500,150,"circle"),("6","6",580,150,"circle"),
 ("trill1","T1",380,225,"rect"),("trill2","T2",465,225,"rect"),("trill3","T3",545,225,"rect"),
 ("foot_eb","Eb",650,180,"rect"),("foot_cs","C#",710,155,"rect"),("foot_c","C",710,220,"rect"),
 ("thumb_left","L",100,285,"rect"),("thumb_right","R",175,285,"rect"),
 ("breath_both","BOTH",355,310,"rect"),("breath_upper","UPPER",520,310,"rect")]
 layout=[dict(id=k,label=l,x=x,y=y,shape=shape) for k,l,x,y,shape in controls]
 variants={}
 for shift in [0,1,2]:
  for note,keys in BRISA_BASE.items():
   midi=int(note)+12*shift
   thumb=[] if shift==0 else ["thumb_right"] if shift==1 else ["thumb_left","thumb_right"]
   variants.setdefault(str(midi),[]).append(dict(octave="normal",keys=keys+thumb))
 def table(mode,variants,coverage):
  return dict(version=1,instrument="roland-ae-brisa",fingering_mode=mode,verified=False,renderer="brisa",layout_keys=layout,
   required_settings=f"AE-BRISA: Fingering Mode {mode.title()}; transpose 0; tone octave 0; factory key/breath mapping",
   coverage=coverage,key_semantics="1-6 and named levers are physical keys; thumb_left/right are rear keys as viewed in Roland chart; breath_* are breath instructions, NOT keys",
   sources=[dict(url="https://static.roland.com/assets/media/pdf/AE-BRISA_Fingering_Chart_multi01_W.pdf",pages=[1 if mode=="brisa" else 2],method="reviewed chart diagrams; only Brisa octave-button expansion is derived"),dict(url="https://static.roland.com/manuals/ae-brisa_reference/en-US/334327947338651915.html",method="rear octave-key operations")],
   fingerings={k:v[0] for k,v in variants.items()},alternatives={k:v[1:] for k,v in variants.items() if len(v)>1})
 brisa=table("brisa",variants,"Chart C4-C#5 plus documented +1/+2 octave buttons: MIDI 60-97; extra trill-derived pitches not added")
 variants={}
 for entry in BRISA_FLUTE:
  keys=entry['keys']+["breath_both" if entry['breath']=="low" else "breath_upper"]
  variants.setdefault(str(entry['midi']),[]).append(dict(octave="normal",keys=keys))
 flute=table("flute",variants,"Charted C4-C7 (MIDI 60-96); lower-register C5/C#5 primary, upper-hole alternatives preserved; uncharted alternatives excluded")
 return dict(version=1,instrument="roland-ae-brisa",modes=dict(brisa=brisa,flute=flute))

if __name__ == "__main__":
 for name,filename,chart,panel,octaves in [
  ("ae05","AE-05_eng05_W.pdf",13,6,1),
  ("ae10","AE-10_eng03_W.pdf",13,4,2),
  ("ae20","AE-20_eng01_W.pdf",22,6,2)]:
  write(name,make_sax(name,"https://static.roland.com/assets/media/pdf/"+filename,chart,panel,octaves))
 tuning=[64,59,55,50,45,40] # string 1 high E through string 6 low E; sounding MIDI
 frets=19
 fingerings={};alternatives={}
 for midi in range(min(tuning),max(tuning)+frets+1):
  positions=sorted([(midi-open_midi,string) for string,open_midi in enumerate(tuning,1) if 0<=midi-open_midi<=frets])
  variants=[dict(octave="normal",keys=[f"s{string}_f{fret}"]) for fret,string in positions]
  fingerings[str(midi)]=variants[0]
  if len(variants)>1:alternatives[str(midi)]=variants[1:]
 write("guitar",dict(version=1,instrument="guitar",display_name="Guitar",verified=False,renderer="guitar",required_settings="Standard E A D G B E; no capo; frets 0-19; single-note melody",
  coverage="Sounding MIDI 40-83; all string/fret alternatives retained; lowest-fret primary",
  sources=[dict(url="https://www.yamaha.com/en/musical_instrument_guide/acoustic_guitar/mechanism/mechanism002.html",method="standard tuning plus one semitone per fret")],
  tuning_midi=tuning,fret_count=frets,fingerings=fingerings,alternatives=alternatives))

 # Bass profiles share the deterministic fret arithmetic, not guitar tuning.
 bass_source="https://hub.yamaha.com/guitars/bass/choosing-the-right-bass-part-1-four-string-or-five-string/"
 for name,tuning in [("guitar-bass",[43,38,33,28]),("guitar-bass-5string",[43,38,33,28,23])]:
  fingerings={};alternatives={}
  for midi in range(min(tuning),max(tuning)+20):
   positions=sorted((midi-note,string) for string,note in enumerate(tuning,1) if 0<=midi-note<=19)
   variants=[dict(octave="normal",keys=[f"s{string}_f{fret}"]) for fret,string in positions]
   fingerings[str(midi)]=variants[0]
   if len(variants)>1: alternatives[str(midi)]=variants[1:]
  write(name,dict(version=1,instrument=name,verified=False,renderer="guitar",required_settings=("Standard E1 A1 D2 G2" if len(tuning)==4 else "Standard B0 E1 A1 D2 G2")+"; no capo; frets 0-19; melody, not bass-part extraction",
   sources=[dict(url=bass_source,method="standard bass tuning plus one semitone per fret; sounding MIDI")],tuning_midi=tuning,fret_count=19,fingerings=fingerings,alternatives=alternatives))
 # MIDI naming uses middle C = C4, unlike Yamaha manuals' C3 convention.
 keyboard_source="https://uk.yamaha.com/files/download/other_assets/3/2291233/PSR-E383_reference_manual_En_B0_web.pdf"
 piano_source="https://europe.yamaha.com/files/download/other_assets/6/328146/p105_en_om_a0.pdf"
 for name,low,high in [("piano",21,108),("keyboard-76",28,103),("keyboard-61",36,96)]:
  write(name,dict(version=1,instrument=name,verified=False,renderer="piano",required_settings=f"{high-low+1} keys; transpose 0; middle C = C4; single-note melody; no finger numbers",
   sources=[dict(url=piano_source if name=="piano" else keyboard_source,method="documented keyboard endpoints mapped to sounding MIDI; Yamaha C3 = MIDI 60 / scientific C4")],
   fingerings={str(m):dict(octave="normal",keys=[f"key_{m}"]) for m in range(low,high+1)},alternatives={}))

 write("ae-brisa",make_brisa())
 for name,url in [
  ("yds120","https://usa.yamaha.com/files/download/other_assets/8/1628388/yds-120_en_om_c0-w.pdf"),
  ("yds150","https://usa.yamaha.com/files/download/other_assets/2/1361052/yds-150_en_om_i0w.pdf")]:
  write(name,make_yamaha(name,url))
