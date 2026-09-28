//! Offline fingering overview and deliberately non-autoplaying practice player.
use crate::{
    instruments::{
        self,
        diagram::{DiagramState, PressedStyle},
    },
    music::timeline::PracticeTimeline,
    render::RenderSettings,
};
use anyhow::Result;
use std::path::Path;

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn url_segment(value: &str) -> String {
    value
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// Publication changes the staged video's name, but keeps this relative link portable.
pub fn set_video_link(directory: &Path, video: &Path) -> Result<()> {
    let page = directory.join("practice.html");
    let name = video
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("video needs a filename"))?
        .to_string_lossy();
    let html = std::fs::read_to_string(&page)?;
    std::fs::write(
        page,
        html.replace(
            "\"../practice.mp4\"",
            &format!("\"{}\"", url_segment(&name)),
        ),
    )?;
    Ok(())
}

pub fn write_sheet(
    directory: &Path,
    timeline: &PracticeTimeline,
    settings: &RenderSettings,
    video: &str,
) -> Result<()> {
    let title = escape(settings.title.as_deref().unwrap_or("Your practice"));
    let mut html = format!(
        r##"<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>{title} · Tono practice</title><style>
body{{margin:0;background:#12151c;color:#eef1f7;font:17px/1.6 system-ui,sans-serif}}main{{max-width:1200px;margin:auto;padding:32px 24px}}h1{{line-height:1.15}}p{{max-width:75ch;color:#c8cfdb}}a{{color:#ffcf65}}.cards{{display:grid;grid-template-columns:repeat(auto-fit,minmax(250px,1fr));gap:20px;margin:28px 0}}article{{background:#1c2330;border:1px solid #414b5c;border-radius:16px;padding:20px;text-align:center}}article svg{{width:100%;max-height:420px}}h3{{margin:0}}video{{width:100%;background:#000;border-radius:12px}}.warning{{color:#ffcf65}}.rest{{grid-column:1/-1;border-left:3px solid #ffcf65;padding:10px 20px;color:#c8cfdb}}.timing{{font-size:14px;color:#c8cfdb;margin:6px 0}}@media print{{video,.play{{display:none}}article{{break-inside:avoid}}}}
</style><main><p>TONO · {}</p><h1>{title}</h1><h2>Practice the whole melody</h2><p>Follow every numbered note from left to right, then continue on the next row. Repeated notes stay in the sequence. Filled keys are held down; unfilled keys stay released. Practice slowly at your own pace, repeat any section, then try the video below. Times and hold lengths match the practice video, including its count-in.</p><p>{}</p><a class="play" href="#player">Ready? Go to the player ↓</a>"##,
        escape(settings.instrument.name()),
        escape(&settings.required_settings)
    );
    if settings.fingering_is_unverified {
        html.push_str(
            "<p class=\"warning\">Fingerings have not yet been checked on the instrument.</p>",
        );
    }
    for warning in &settings.video_warnings {
        html.push_str(&format!("<p class=\"warning\">{}</p>", escape(warning)));
    }
    html.push_str("<div class=\"cards\">");
    let mut previous_end = timeline.count_in.map_or(0.0, |count| count.duration());
    if previous_end > 0.0 {
        html.push_str(&format!(
            "<div class=\"rest\">Preparation count · {previous_end:.2} s before the music</div>"
        ));
    }
    for (position, entry) in timeline.entries.iter().enumerate() {
        let keys = entry.fingering.pressed_key_ids();
        let index = position + 1;
        let gap = entry.start - previous_end;
        let rest = if gap > 0.001 {
            format!("<p class=\"timing\">Rest · {gap:.2} s · then note {index}</p>")
        } else {
            String::new()
        };
        previous_end = entry.end;
        let state = DiagramState {
            upper_hand: settings.upper_hand,
            pressed_keys: &keys,
            pressed_style: PressedStyle::Sounding,
            transition_hint: None,
        };
        let prefix = format!("sheet-{index}-");
        let (view, drawing) = if settings.instrument.is_horizontal() {
            (
                "0 -50 850 470",
                instruments::horizontal_svg(
                    settings.instrument,
                    &settings.layout_keys,
                    &state,
                    0.0,
                    0.0,
                    1.0,
                    &prefix,
                ),
            )
        } else {
            (
                "0 0 640 900",
                instruments::wind_svg(
                    settings.instrument,
                    &settings.layout_keys,
                    &state,
                    "",
                    &prefix,
                ),
            )
        };
        let name = [
            "C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B",
        ][entry.midi as usize % 12];
        let octave = entry.midi as i32 / 12 - 1;
        html.push_str(&format!("<article>{rest}<h3>{index}. {name}{octave}</h3><p class=\"timing\">Video {start:.2} s · hold {duration:.2} s</p><svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{view}\" role=\"img\" aria-label=\"Fingering for {name}{octave}\">{drawing}</svg></article>", start = entry.start, duration = entry.end - entry.start));
    }
    let tail = timeline.total_duration_seconds - previous_end;
    if tail > 0.001 {
        html.push_str(&format!(
            "<div class=\"rest\">Rest · {tail:.2} s · end of practice</div>"
        ));
    }
    html.push_str(&format!("</div><section id=\"player\"><h2>Play when you’re ready</h2><p>Playback starts only when you press Play. Pause or return to the sheet whenever you need. The video includes your selected count-in.</p><video controls preload=\"none\" playsinline src=\"{}\">Open the MP4 in your video player.</video><p><a href=\"{}\">Open the MP4 directly</a></p></section></main></html>", escape(video), escape(video)));
    std::fs::write(directory.join("practice.html"), html)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sheet_preserves_note_order_repeats_and_rests_and_waits_for_play() {
        use crate::instruments::{
            fingering::{map_notes_to_fingerings, FingeringTable},
            Instrument,
        };
        use crate::music::notes::NoteEvent;
        for instrument in [
            Instrument::Ae01,
            Instrument::Ae20,
            Instrument::Guitar,
            Instrument::Piano,
        ] {
            let table = FingeringTable::load_instrument(instrument).unwrap();
            let notes = [60, 62, 60, 60]
                .iter()
                .enumerate()
                .map(|(i, &midi)| NoteEvent {
                    start: i as f64,
                    end: i as f64 + 0.9,
                    midi,
                    confidence: 1.0,
                })
                .collect::<Vec<_>>();
            let timeline =
                PracticeTimeline::new(map_notes_to_fingerings(&notes, &table).unwrap(), 100.0, 0.0);
            let settings = RenderSettings {
                instrument,
                layout_keys: table.layout_keys,
                title: Some("<unsafe> & song".into()),
                ..RenderSettings::default()
            };
            let dir = tempfile::tempdir().unwrap();
            write_sheet(dir.path(), &timeline, &settings, "../practice.mp4").unwrap();
            set_video_link(dir.path(), Path::new("/tmp/a #é.mp4")).unwrap();
            let html = std::fs::read_to_string(dir.path().join("practice.html")).unwrap();
            assert_eq!(html.matches("<article>").count(), 4);
            assert!(html.contains("4. C4"));
            assert!(html.contains("hold 0.90 s"));
            assert_eq!(html.matches("Rest · 0.10 s · then note").count(), 3);
            assert!(!html.contains("<unsafe>"));
            assert!(html.contains("<video controls preload=\"none\""));
            assert_eq!(html.matches("a%20%23%C3%A9.mp4").count(), 2);
            assert!(!html.contains("../practice.mp4"));
            for piece in html.split("<svg ").skip(1) {
                let svg = format!(
                    "<svg {}",
                    piece.split("</svg>").next().unwrap().to_owned() + "</svg>"
                );
                roxmltree::Document::parse(&svg).unwrap();
            }
        }
    }

    #[test]
    fn publication_encodes_filename_and_escapes_text() {
        assert_eq!(url_segment("a #é.mp4"), "a%20%23%C3%A9.mp4");
        assert_eq!(escape("<x>\"&"), "&lt;x&gt;&quot;&amp;");
    }
}
