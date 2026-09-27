"""Deterministic expansion of sourced front-key tables; never guesses fingerings.
Run from any directory after reviewing the cited Roland chart pages.
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
