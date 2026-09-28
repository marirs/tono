//! Known melody + supplied/separated backing, or synthesized reference tones.
use super::{
    practice::{build_practice_video, PracticeRequest},
    prep::{assess_separation, practice_title, report_practice, PrepOptions},
};
use crate::{
    analysis::worker::{analyze_region, WorkerPaths},
    media::{
        source::{normalize_source, SampleRange, ANALYSIS_SAMPLE_RATE},
        validation::probe_audio_duration_seconds,
    },
    music::{import, notes::NoteEvent, timecode::parse_clip_window},
};
use anyhow::{bail, Result};
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Clone, Default)]
pub struct ImportOptions {
    pub backing: Option<PathBuf>,
    pub audio: Option<PathBuf>,
    pub make_bgm: bool,
    pub audio_offset: f64,
    pub melody_track: Option<u16>,
}

pub fn validate_options(o: &PrepOptions) -> Result<()> {
    let i = &o.import;
    if !i.audio_offset.is_finite() {
        bail!("--audio-offset must be finite");
    }
    if i.backing.is_some() && (i.audio.is_some() || i.make_bgm) {
        bail!("use either --backing or --audio with --make-bgm");
    }
    if i.audio.is_some() != i.make_bgm {
        bail!("--audio and --make-bgm must be used together");
    }
    let has_options = i.backing.is_some()
        || i.audio.is_some()
        || i.make_bgm
        || i.audio_offset != 0.0
        || i.melody_track.is_some();
    if !import::is_melody_file(&o.input) {
        if has_options {
            bail!("--backing, --audio, --make-bgm, --audio-offset and --melody-track require a MIDI/MusicXML/JSON input");
        }
        return Ok(());
    }
    if i.audio_offset != 0.0 && i.backing.is_none() && i.audio.is_none() {
        bail!("--audio-offset requires --backing or --audio");
    }
    for source in i.backing.iter().chain(i.audio.iter()) {
        if !source.is_file() {
            bail!("audio input not found: {}", source.display());
        }
    }
    let song = import::load(&o.input, i.melody_track)?;
    let (notes, _) = clip(&song.notes, song.duration, o)?;
    let mut table = crate::instruments::fingering::FingeringTable::load_for_setup(
        o.instrument,
        o.fingering_mode,
        o.pan_style,
    )?;
    if o.easy_fingering && table.easy_fingering.is_some() {
        crate::music::range::fit_easy_melody(&notes, &mut table, o.range_policy)?;
    } else {
        crate::music::range::fit_melody_to_table(&notes, &table, o.range_policy)?;
    }
    // The advanced prep form writes in-place; refuse sources inside that directory.
    if o.output_video.is_none() {
        if let Ok(out) = o.output_directory.canonicalize() {
            for source in std::iter::once(&o.input)
                .chain(i.backing.iter())
                .chain(i.audio.iter())
            {
                if source.canonicalize()?.starts_with(&out) {
                    bail!("input files cannot be inside the output directory");
                }
            }
        }
    }
    Ok(())
}

fn clip(
    notes: &[NoteEvent],
    duration: f64,
    o: &PrepOptions,
) -> Result<(Vec<NoteEvent>, SampleRange)> {
    let (from, to) = parse_clip_window(o.from_timecode.as_deref(), o.to_timecode.as_deref())?;
    let start = from;
    let end = to.unwrap_or(duration);
    if start < 0.0 || end <= start || end > duration + 0.001 {
        bail!("melody region must lie within 0..{duration:.3}s; imported timing is not stretched to fit audio");
    }
    let range = SampleRange::from_seconds(start, end.min(duration));
    let ns: Vec<_> = notes
        .iter()
        .filter_map(|n| {
            let a = n.start.max(range.start_seconds());
            let b = n.end.min(range.end_seconds());
            (b > a).then_some(NoteEvent {
                start: (a - range.start_seconds()).max(0.0),
                end: b - range.start_seconds(),
                ..*n
            })
        })
        .collect();
    if ns.is_empty() {
        bail!("selected melody region contains no notes");
    }
    Ok((ns, range))
}

fn ffmpeg_step(ffmpeg: &Path, input: &Path, output: &Path, args: &[String]) -> Result<()> {
    let r = Command::new(ffmpeg)
        .args(["-v", "error", "-nostdin", "-y", "-i"])
        .arg(input)
        .args(args)
        .arg(output)
        .output()?;
    if !r.status.success() {
        bail!(
            "audio preparation failed: {}",
            String::from_utf8_lossy(&r.stderr)
        );
    }
    Ok(())
}

/// Positive offset discards audio before melody t=0; negative offset inserts silence.
fn align_audio(
    ffmpeg: &Path,
    ffprobe: &Path,
    input: &Path,
    output: &Path,
    range: SampleRange,
    offset: f64,
    work: &Path,
) -> Result<()> {
    let normalized = work.join("import-audio.wav");
    normalize_source(ffmpeg, input, &normalized)?;
    let available = probe_audio_duration_seconds(ffprobe, &normalized)?;
    let start = range.start_seconds() + offset;
    let end = range.end_seconds() + offset;
    if end > available + 0.001 || end <= 0.0 {
        bail!("audio alignment needs seconds {start:.3}..{end:.3}, but audio is {available:.3}s; supply matching audio or adjust --audio-offset/--from/--to");
    }
    let sr = ANALYSIS_SAMPLE_RATE as f64;
    let trim = (start.max(0.0) * sr).round() as u64;
    let delay = (-start.min(0.0) * sr).round() as u64;
    let len = range.end_sample - range.start_sample;
    let filter=format!("atrim=start_sample={trim},asetpts=PTS-STARTPTS,adelay={delay}S:all=1,apad=whole_len={len},atrim=end_sample={len}");
    ffmpeg_step(
        ffmpeg,
        &normalized,
        output,
        &["-af".into(), filter, "-c:a".into(), "pcm_s24le".into()],
    )?;
    let actual = probe_audio_duration_seconds(ffprobe, output)?;
    if (actual - range.duration_seconds()).abs() > 0.001 {
        bail!("aligned audio duration mismatch");
    }
    Ok(())
}

pub fn prepare(o: &PrepOptions, ffmpeg: &Path, ffprobe: &Path, work: &Path) -> Result<()> {
    let song = import::load(&o.input, o.import.melody_track)?;
    let (notes, range) = clip(&song.notes, song.duration, o)?;
    let out = &o.output_directory;
    let duration = range.duration_seconds();
    let mut warnings = song.warnings.clone();
    let mut beats: Vec<_> = song
        .beat_times
        .iter()
        .filter(|t| **t >= range.start_seconds() && **t < range.end_seconds())
        .map(|t| t - range.start_seconds())
        .collect();
    println!(
        "✓ imported {} notes from {} ({duration:.3}s); transcription and cleanup skipped",
        notes.len(),
        song.format
    );
    let mut separation = serde_json::Value::Null;
    let kind;
    let beat_source;
    if let Some(audio) = &o.import.audio {
        let region = work.join("region.wav");
        align_audio(
            ffmpeg,
            ffprobe,
            audio,
            &region,
            range,
            o.import.audio_offset,
            work,
        )?;
        let a = analyze_region(
            &region,
            o.part.worker_name(),
            o.separation_model.worker_name(),
            true,
            &WorkerPaths {
                work_dir: work,
                out_dir: out,
            },
            duration,
        )?;
        let quality = assess_separation(&a)?;
        warnings.extend(quality.warnings);
        warnings.push("Backing separation is best-effort: the excluded stem may contain accompaniment, and retained stems may contain lead leakage. Listen before practicing.".into());
        beats = a.beat_times;
        beat_source = "detected backing beats";
        separation = json!({"model":a.separation.model,"excluded_stem":a.separation.lead_stem,"retained_stems":a.separation.backing_stems,"lead_selection":a.separation.lead_selection,"backing_to_mix_db":a.separation.backing_to_mix_db,"gain_db":a.separation.backing_gain_db,"gain_limited":a.separation.backing_gain_limited,"quality":"unverified","verified_by_listening":false});
        kind = "separated";
    } else if let Some(backing) = &o.import.backing {
        align_audio(
            ffmpeg,
            ffprobe,
            backing,
            &out.join("backing.wav"),
            range,
            o.import.audio_offset,
            work,
        )?;
        warnings.push("Supplied backing alignment uses the requested offset; matching duration does not verify musical synchronization.".into());
        kind = "supplied";
        beat_source = "imported tempo map (not verified against backing)";
    } else {
        crate::media::audio::write_note_guide(&out.join("guide.wav"), &notes, duration)?;
        normalize_source(ffmpeg, &out.join("guide.wav"), &out.join("backing.wav"))?;
        kind = "synthesized_reference";
        beat_source = "imported tempo map";
        println!("· no backing supplied: using synthesized reference melody");
    }
    if kind != "synthesized_reference" {
        ffmpeg_step(
            ffmpeg,
            &out.join("backing.wav"),
            &out.join("backing.mp3"),
            &[
                "-c:a".into(),
                "libmp3lame".into(),
                "-b:a".into(),
                "192k".into(),
            ],
        )?;
    }
    let notes_doc = json!({"version":1,"stage":"imported_unmodified","timebase":"selected_region","region_start_in_source":range.start_seconds(),"duration_seconds":duration,"notes":notes});
    std::fs::write(
        out.join("notes.json"),
        serde_json::to_vec_pretty(&notes_doc)?,
    )?;
    for w in &warnings {
        println!("! {w}");
    }
    let title = practice_title(&o.input, o.title.as_deref());
    let practice = build_practice_video(&PracticeRequest {
        title: &title,
        instrument: o.instrument,
        pan_style: o.pan_style,
        fingering_mode: o.fingering_mode,
        cleaned_notes: &notes,
        beat_times: &beats,
        region_duration_seconds: duration,
        tempo_scale: o.tempo_scale,
        count_in_beats: o.count_in_beats,
        upper_hand: o.upper_hand,
        metronome: o.metronome,
        range_policy: o.range_policy,
        easy_fingering: o.easy_fingering,
        low_confidence: false,
        out_dir: out,
        output_video: o.output_video.as_deref(),
        work_dir: work,
        ffmpeg,
        ffprobe,
    })?;
    report_practice(&practice);
    warnings.extend(practice.warnings.clone());
    let p = json!({
        "version":1,"tono":env!("CARGO_PKG_VERSION"),"title":title,"instrument":o.instrument,"pan_style":o.pan_style,"fingering_mode":o.fingering_mode,
        "source":{"path":o.input.canonicalize()?,"format":song.format,"duration":song.duration},
        "selected_region":{"start":range.start_seconds(),"end":range.end_seconds(),"method":"imported_timeline"},
        "transcription":{"source":"import","model":null,"cleanup_applied":false,"note_count":notes.len(),"verified_by_listening":false,"melody_track":o.import.melody_track},
        "backing":{"source":kind,"input":o.import.backing.as_ref().or(o.import.audio.as_ref()).map(|p|p.canonicalize()).transpose()?,"file":"backing.wav","mp3":if kind=="synthesized_reference"{None}else{Some("backing.mp3")},"audio_offset_seconds":o.import.audio_offset,"duration":duration},
        "separation":separation,
        "fingering":{"file":"fingering.json","table_verified":practice.table_verified,"range_policy":practice.fitted.policy,"octave_shift":practice.fitted.octave_shift,"semitone_offset":practice.fitted.semitone_offset,"backing_transpose_semitones":practice.fitted.transpose_semitones(),"easy_fingering":practice.fitted.easy_fingering,"melody_shape_changed":false},
        "practice":{"file":o.output_video.as_ref().map(|p|p.display().to_string()).unwrap_or("practice.mp4".into()),"duration":practice.practice_duration_seconds,"tempo_scale":o.tempo_scale,"count_in_seconds":practice.count_in.map_or(0.0,crate::music::timeline::CountIn::duration),"beat_source":beat_source,"beat_count":practice.beat_count,"bpm":practice.practice_bpm,"metronome":format!("{:?}",practice.metronome).to_lowercase(),"validation":{"tool":"ffprobe","passed":true,"video_duration":practice.validated.video_duration_seconds,"audio_duration":practice.validated.audio_duration_seconds}},
        "warnings":warnings,
    });
    std::fs::write(out.join("project.json"), serde_json::to_vec_pretty(&p)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn audio_offsets_trim_pad_and_reject_short_sources() {
        let (Some(ffmpeg), Some(ffprobe)) = (
            crate::paths::ffmpeg_executable(),
            crate::paths::ffprobe_executable(),
        ) else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("input.wav");
        // Distinct constant regions make a one-sample alignment error observable.
        let mut samples = vec![0.1; 44100];
        samples[22050..].fill(0.2);
        crate::media::audio::write_mono_wav(&input, &samples, 44100).unwrap();
        let output = dir.path().join("aligned.wav");
        for (offset, expected) in [(0.5, 0.2), (-0.25, 0.0)] {
            align_audio(
                &ffmpeg,
                &ffprobe,
                &input,
                &output,
                SampleRange::from_seconds(0.0, 0.5),
                offset,
                dir.path(),
            )
            .unwrap();
            let bytes = Command::new(&ffmpeg)
                .args(["-v", "error", "-i"])
                .arg(&output)
                .args(["-ac", "1", "-f", "f32le", "-"])
                .output()
                .unwrap()
                .stdout;
            assert_eq!(bytes.len(), 22050 * 4);
            let first = f32::from_le_bytes(bytes[0..4].try_into().unwrap());
            assert!((first - expected).abs() < 0.001);
            if offset < 0.0 {
                let at = 11025 * 4;
                let after = f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
                assert!((after - 0.1).abs() < 0.001);
            }
        }
        assert!(align_audio(
            &ffmpeg,
            &ffprobe,
            &input,
            &output,
            SampleRange::from_seconds(0.0, 0.5),
            0.75,
            dir.path()
        )
        .is_err());
    }
}
