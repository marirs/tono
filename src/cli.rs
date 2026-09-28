//! CLI arguments and command dispatch.
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand};

use tono::commands::{demo, doctor};
use tono::instruments::brisa::{validate_mode, FingeringMode};
use tono::instruments::diagram::UpperHand;
use tono::instruments::Instrument;
use tono::music::range::RangePolicy;
use tono::pipeline::output;
use tono::pipeline::prep::{self, Part};
use tono::render::MetronomeMode;

#[derive(Parser)]
#[command(
    name = "tono",
    version,
    about = "Turn a song into an instrument practice video",
    args_conflicts_with_subcommands = true,
    subcommand_negates_reqs = true,
    after_help = "Example: tono song.mp3 --instrument ae01\nPiano: --piano or --instrument piano (alias piano-88). Guitar: guitar-6string; bass: guitar-bass or guitar-bass-5string.\nStrict fitting preserves melodic intervals: whole-octave shifts for most instruments, whole-semitone shifts for pitched pans; otherwise generation stops.\nMood Pan requires --pan-style; its output folder also includes the style.\nIndividual octave folding is disabled. Outputs default to ./tono-practices/<song>_<instrument>_<YYYYMMDD>/practice.mp4; reruns replace matching outputs."
)]
struct Cli {
    /// Local audio/video or melody file (.mid, .midi, .musicxml, .xml, .json).
    #[arg(value_name = "INPUT", required = true)]
    input: Option<PathBuf>,
    /// Optional output name: custom.mp4 creates custom/practice.mp4 and practice.html.
    #[arg(value_name = "OUTPUT.mp4")]
    output: Option<PathBuf>,
    /// Parent directory for automatically named practice folders.
    #[arg(long, short = 'd', value_name = "DIRECTORY", conflicts_with = "output")]
    practice_dir: Option<PathBuf>,
    #[command(flatten)]
    options: PreparationArgs,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Args)]
struct PreparationArgs {
    /// Ready-made backing audio for a MIDI/MusicXML/JSON melody (no separation).
    #[arg(long, conflicts_with_all = ["audio", "make_bgm"])]
    backing: Option<PathBuf>,
    /// Original recording to separate when importing a melody file.
    #[arg(long, requires = "make_bgm")]
    audio: Option<PathBuf>,
    /// Create backing from --audio; lead isolation is best-effort.
    #[arg(long, requires = "audio")]
    make_bgm: bool,
    /// Audio-file seconds corresponding to melody time zero (trim positive, pad negative).
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    audio_offset: f64,
    /// One-based MIDI track or MusicXML part index; required if multiple contain notes.
    #[arg(long, value_parser = clap::value_parser!(u16).range(1..))]
    melody_track: Option<u16>,
    /// Demucs model; htdemucs-6s also retains guitar/piano when excluding other.
    #[arg(long, value_enum, default_value_t = prep::SeparationModel::Htdemucs)]
    separation_model: prep::SeparationModel,

    /// Video heading; defaults to the input filename without its extension.
    #[arg(long, value_name = "TEXT", value_parser = parse_title)]
    title: Option<String>,
    /// Instrument profile, fingering table and diagram.
    #[arg(
        long,
        required_unless_present = "piano",
        conflicts_with = "piano",
        value_enum
    )]
    instrument: Option<Instrument>,
    /// Required for AE-BRISA; must match the instrument's Fingering Mode.
    #[arg(long, value_enum, required_if_eq("instrument", "ae-brisa"))]
    fingering_mode: Option<FingeringMode>,
    /// Required for Mood Pan; match its Style knob, with Handpan tone and factory tuning.
    #[arg(long, value_enum)]
    pan_style: Option<tono::instruments::pan::PanStyle>,
    /// Full-size 88-key piano (shorthand for --instrument piano).
    #[arg(long, conflicts_with = "instrument")]
    piano: bool,
    /// Manual start (HH:MM:SS); overrides auto detection.
    #[arg(long = "from")]
    from_time: Option<String>,
    /// Manual end (HH:MM:SS); overrides auto detection.
    #[arg(long = "to")]
    to_time: Option<String>,
    #[arg(long, value_enum, default_value_t = Part::Vocal)]
    part: Part,
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    auto_song_region: bool,
    /// Preparation beats before playback (0 disables the count-in).
    #[arg(long, default_value_t = 4, value_parser = clap::value_parser!(u8).range(0..=4))]
    count_in: u8,
    /// Override vertical wind hand labels (default: left above right); does not remap keys.
    #[arg(long, value_enum)]
    upper_hand: Option<UpperHand>,
    #[arg(long, default_value_t = 1.0)]
    tempo_scale: f64,
    #[arg(long, value_enum, default_value_t = MetronomeMode::Visual)]
    metronome: MetronomeMode,
    /// Strict preserves intervals: octave fitting, or whole-semitone fitting for pans; fold is rejected.
    #[arg(long, value_enum)]
    range_policy: Option<RangePolicy>,
    /// Use beginner controls; transpose melody and backing together if needed.
    /// Unsupported instruments warn and continue normally.
    #[arg(long)]
    easy_fingering: bool,
    /// Transcribed notes below this confidence are discarded.
    #[arg(long, default_value_t = 0.3)]
    min_note_confidence: f64,
    /// Preserve intermediate files and diagnostic logs.
    #[arg(long)]
    keep_work: bool,
}

fn parse_title(value: &str) -> std::result::Result<String, String> {
    let title = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if title.is_empty() || title.chars().any(char::is_control) {
        Err("title must contain visible text without control characters".into())
    } else {
        Ok(title)
    }
}

impl PreparationArgs {
    fn selected_instrument(&self) -> Result<Instrument> {
        if self.piano {
            Ok(Instrument::Piano)
        } else {
            self.instrument
                .context("--instrument or --piano is required")
        }
    }
    fn into_options(
        self,
        input: PathBuf,
        out: PathBuf,
        video: Option<PathBuf>,
    ) -> Result<prep::PrepOptions> {
        if self.range_policy == Some(RangePolicy::Fold) {
            bail!(tono::music::range::FOLDING_DISABLED_MESSAGE);
        }
        let instrument = self.selected_instrument()?;
        validate_mode(instrument, self.fingering_mode)?;
        tono::instruments::pan::validate_style(instrument, self.pan_style)?;
        if !(0.0..=1.0).contains(&self.min_note_confidence) {
            bail!("--min-note-confidence must be between 0 and 1");
        }
        Ok(prep::PrepOptions {
            title: self.title,
            import: tono::pipeline::imported::ImportOptions {
                backing: self.backing,
                audio: self.audio,
                make_bgm: self.make_bgm,
                audio_offset: self.audio_offset,
                melody_track: self.melody_track,
            },
            separation_model: self.separation_model,
            input,
            instrument,
            pan_style: self.pan_style,
            fingering_mode: self.fingering_mode,
            from_timecode: self.from_time,
            to_timecode: self.to_time,
            part: self.part,
            auto_song_region: self.auto_song_region,
            output_directory: out,
            output_video: video,
            keep_work: self.keep_work,
            min_note_confidence: self.min_note_confidence,
            tempo_scale: self.tempo_scale,
            count_in_beats: self.count_in,
            upper_hand: self
                .upper_hand
                .unwrap_or_else(|| instrument.default_upper_hand()),
            metronome: self.metronome,
            range_policy: self.range_policy.unwrap_or(RangePolicy::Strict),
            easy_fingering: self.easy_fingering,
        })
    }
}

#[derive(Subcommand)]
enum Commands {
    /// Prepare the private audio/ML runtime (also automatic on first render).
    Setup,
    /// Check ffmpeg, python and the ML worker.
    Doctor {
        /// Treat missing ML packages/models as failures.
        #[arg(long)]
        ml: bool,
    },
    /// Render the built-in melody without ML.
    Demo {
        #[arg(long, value_enum, default_value_t = Instrument::Ae01)]
        instrument: Instrument,
        /// Required for AE-BRISA: brisa or flute.
        #[arg(long, value_enum, required_if_eq("instrument", "ae-brisa"))]
        fingering_mode: Option<FingeringMode>,
        /// Required for Mood Pan; select the same style on the instrument.
        #[arg(long, value_enum)]
        pan_style: Option<tono::instruments::pan::PanStyle>,
        #[arg(long, default_value = "./tono-demo")]
        out: PathBuf,
        #[arg(long, default_value_t = 100.0)]
        bpm: f64,
        /// Mux this audio instead of the synthesized guide track.
        #[arg(long)]
        backing: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = MetronomeMode::Visual)]
        metronome: MetronomeMode,
        #[arg(long)]
        keep_work: bool,
    },
    /// Advanced form: write practice.mp4 and supporting files in one directory.
    Prep {
        input: PathBuf,
        #[arg(long, default_value = "./tono-output")]
        out: PathBuf,
        #[command(flatten)]
        options: PreparationArgs,
    },
}

/// Each output name selects a self-contained practice folder.
fn short_form_options(
    input: PathBuf,
    output: PathBuf,
    options: PreparationArgs,
) -> Result<prep::PrepOptions> {
    if !output
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("mp4"))
    {
        bail!("output must be an .mp4 filename");
    }
    if input == output
        || (input.exists() && output.exists() && same_file::is_same_file(&input, &output)?)
    {
        bail!("output must not overwrite the input file");
    }
    let project = output.with_extension("");
    let video = project.join("practice.mp4");
    output::validate_destination(&input, &video)?;
    options.into_options(input, project, Some(video))
}

fn print_header(input: Option<&Path>, output: Option<&Path>) -> Result<()> {
    let color = std::io::stdout().is_terminal()
        && std::env::var_os("NO_COLOR").is_none()
        && std::env::var("TERM").as_deref() != Ok("dumb");
    if color {
        println!("\x1b[1;36mTono - Play what you love.\x1b[0m");
    } else {
        println!("Tono - Play what you love.");
    }
    println!("v{}", env!("CARGO_PKG_VERSION"));
    for (label, path) in [("Input", input), ("Output", output)] {
        if let Some(path) = path {
            println!("{label}: {}", std::path::absolute(path)?.display());
        }
    }
    println!();
    Ok(())
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Commands::Setup) => {
            print_header(None, None)?;
            tono::runtime::ensure_ready_cli()?;
            doctor::run_doctor(true)
        }
        Some(Commands::Doctor { ml }) => {
            print_header(None, None)?;
            doctor::run_doctor(ml)
        }
        Some(Commands::Demo {
            instrument,
            fingering_mode,
            pan_style,
            out,
            bpm,
            backing,
            metronome,
            keep_work,
        }) => {
            print_header(backing.as_deref(), Some(&out.join("practice.mp4")))?;
            demo::run_demo(&demo::DemoOptions {
                instrument,
                fingering_mode,
                pan_style,
                output_directory: out,
                beats_per_minute: bpm,
                backing_audio: backing,
                metronome,
                keep_work,
            })
        }
        Some(Commands::Prep {
            input,
            out,
            options,
        }) => {
            let options = options.into_options(input, out, None)?;
            print_header(
                Some(&options.input),
                Some(&options.output_directory.join("practice.mp4")),
            )?;
            prep::run_prep(&options)
        }
        None => {
            let input = cli.input.expect("required by clap");
            let automatic = cli.output.is_none();
            let mut destination = match cli.output {
                Some(path) => path,
                None => output::automatic_path_for_mode(
                    &input,
                    cli.options.selected_instrument()?,
                    cli.options.fingering_mode,
                    cli.practice_dir.as_deref(),
                    &output::local_date()?,
                )?,
            };
            if automatic {
                if let Some(style) = cli.options.pan_style {
                    let stem = destination.file_stem().unwrap().to_string_lossy();
                    destination.set_file_name(format!("{stem}_{}.mp4", style.id()));
                }
            }
            let options = short_form_options(input, destination, cli.options)?;
            print_header(Some(&options.input), options.output_video.as_deref())?;
            output::run(&options)
        }
    }
}

#[cfg(test)]
mod cli_tests {
    use super::*;

    #[test]
    fn pan_setup_is_required_and_percussion_fails_before_processing() {
        for name in ["moodpan", "mn-10", "taiko-1", "octapad", "spd-20-pro"] {
            let cli = Cli::try_parse_from(["tono", "missing.mp3", "--instrument", name]).unwrap();
            let error = cli
                .options
                .into_options(PathBuf::from("missing.mp3"), PathBuf::from("out"), None)
                .err()
                .unwrap()
                .to_string();
            assert!(
                error.contains("--pan-style") || error.contains("does not support lead-melody"),
                "{error}"
            );
        }
        let cli = Cli::try_parse_from([
            "tono",
            "song.mp3",
            "--instrument",
            "moodpan",
            "--pan-style",
            "minor",
            "--tempo-scale",
            "0.5",
        ])
        .unwrap();
        let o = cli
            .options
            .into_options(PathBuf::from("song.mp3"), PathBuf::from("out"), None)
            .unwrap();
        assert_eq!(o.pan_style, Some(tono::instruments::pan::PanStyle::Minor));
        assert_eq!(o.tempo_scale, 0.5);
        let cli = Cli::try_parse_from([
            "tono",
            "song.mp3",
            "--instrument",
            "ae01",
            "--pan-style",
            "minor",
        ])
        .unwrap();
        assert!(cli
            .options
            .into_options(PathBuf::from("song.mp3"), PathBuf::from("out"), None)
            .is_err());
    }

    #[test]
    fn import_flags_are_explicit_and_keep_existing_controls() {
        let cli = Cli::try_parse_from([
            "tono",
            "melody.mid",
            "--instrument",
            "ae01",
            "--audio",
            "source.mp4",
            "--make-bgm",
            "--part",
            "lead",
            "--separation-model",
            "htdemucs-6s",
            "--audio-offset",
            "7.68",
            "--tempo-scale",
            "0.75",
        ])
        .unwrap();
        let o = cli
            .options
            .into_options(cli.input.unwrap(), PathBuf::from("out"), None)
            .unwrap();
        assert!(o.import.make_bgm);
        assert_eq!(o.import.audio_offset, 7.68);
        assert_eq!(o.tempo_scale, 0.75);
        assert_eq!(o.separation_model, prep::SeparationModel::Htdemucs6s);
        for flags in [
            vec!["--make-bgm"],
            vec!["--audio", "source.mp3"],
            vec![
                "--backing",
                "bgm.mp3",
                "--audio",
                "source.mp3",
                "--make-bgm",
            ],
        ] {
            let mut args = vec!["tono", "melody.mid", "--instrument", "ae01"];
            args.extend(flags);
            assert!(Cli::try_parse_from(args).is_err());
        }
        assert!(Cli::try_parse_from([
            "tono",
            "melody.mid",
            "--instrument",
            "ae01",
            "--backing",
            "bgm.mp3",
            "--audio-offset",
            "-2"
        ])
        .is_ok());
    }

    #[test]
    fn setup_needs_no_song_or_instrument() {
        let cli = Cli::try_parse_from(["tono", "setup"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Setup)));
    }

    #[test]
    fn title_override_reaches_direct_and_prep_options() {
        for command in [None, Some("prep")] {
            let mut args = vec!["tono"];
            args.extend(command);
            args.extend([
                "original.mp3",
                "--instrument",
                "ae20",
                "--title",
                "  Love & Music  ",
            ]);
            let cli = Cli::try_parse_from(args).unwrap();
            let options = match cli.command {
                Some(Commands::Prep {
                    input,
                    out,
                    options,
                }) => options.into_options(input, out, None),
                None => cli
                    .options
                    .into_options(cli.input.unwrap(), PathBuf::from("out"), None),
                _ => unreachable!(),
            }
            .unwrap();
            assert_eq!(
                prep::practice_title(&options.input, options.title.as_deref()),
                "Love & Music"
            );
        }
        assert!(
            Cli::try_parse_from(["tono", "song.mp3", "--instrument", "ae20", "--title", "  "])
                .is_err()
        );
    }

    #[test]
    fn acoustic_profiles_and_aliases_select_explicit_setups() {
        for (id, expected) in [
            ("ukulele", Instrument::Ukulele),
            ("ukulele-high-g", Instrument::Ukulele),
            ("ukulele-low-g", Instrument::UkuleleLowG),
            ("ukulele-baritone", Instrument::UkuleleBaritone),
            ("recorder-baroque", Instrument::RecorderBaroque),
            ("recorder-german", Instrument::RecorderGerman),
            ("yvs120", Instrument::Yvs120),
            ("yvs-120", Instrument::Yvs120),
            ("alto-venova", Instrument::Yvs120),
            ("flute", Instrument::Flute),
            ("flute-cfoot", Instrument::Flute),
            ("flute-bfoot", Instrument::FluteBFoot),
            ("violin", Instrument::Violin),
        ] {
            let cli = Cli::try_parse_from(["tono", "song.mp3", "--instrument", id]).unwrap();
            assert_eq!(cli.options.selected_instrument().unwrap(), expected);
            assert!(cli.options.fingering_mode.is_none());
        }
        assert!(Cli::try_parse_from(["tono", "song.mp3", "--instrument", "recorder"]).is_err());
    }

    #[test]
    fn brisa_requires_a_mode_in_direct_prep_and_demo_commands() {
        for args in [
            vec!["tono", "song.mp3", "--instrument", "ae-brisa"],
            vec!["tono", "prep", "song.mp3", "--instrument", "ae-brisa"],
            vec!["tono", "demo", "--instrument", "ae-brisa"],
        ] {
            let error = Cli::try_parse_from(args.clone()).err().unwrap().to_string();
            assert!(error.contains("--fingering-mode"), "{error}");
            for mode in ["brisa", "flute"] {
                let mut with_mode = args.clone();
                with_mode.extend(["--fingering-mode", mode]);
                assert!(Cli::try_parse_from(with_mode).is_ok());
            }
        }
        let cli = Cli::try_parse_from([
            "tono",
            "song.mp3",
            "--instrument",
            "ae20",
            "--fingering-mode",
            "flute",
        ])
        .unwrap();
        assert!(cli
            .options
            .into_options(PathBuf::from("song.mp3"), PathBuf::from("out"), None)
            .is_err());
        assert!(Cli::try_parse_from([
            "tono",
            "song.mp3",
            "--instrument",
            "ae-brisa",
            "--fingering-mode",
            "trumpet"
        ])
        .is_err());
        let input = std::path::Path::new("song.mp3");
        let brisa = output::automatic_path_for_mode(
            input,
            Instrument::AeBrisa,
            Some(FingeringMode::Brisa),
            None,
            "20260927",
        )
        .unwrap();
        let flute = output::automatic_path_for_mode(
            input,
            Instrument::AeBrisa,
            Some(FingeringMode::Flute),
            None,
            "20260927",
        )
        .unwrap();
        assert_ne!(brisa, flute);
        assert!(flute.ends_with("song_ae-brisa_flute_20260927.mp4"));
    }

    #[test]
    fn piano_shorthand_and_profile_aliases_are_unambiguous() {
        for (name, expected) in [
            ("piano", Instrument::Piano),
            ("piano-88", Instrument::Piano),
            ("piano-76", Instrument::Keyboard76),
            ("piano-61", Instrument::Keyboard61),
            ("guitar-6string", Instrument::Guitar),
            ("guitar-electric", Instrument::Guitar),
            ("guitar-acoustic", Instrument::Guitar),
            ("guitar-classical", Instrument::Guitar),
            ("guitar-4string", Instrument::Bass),
            ("bass", Instrument::Bass),
            ("bass-5string", Instrument::Bass5),
        ] {
            let cli = Cli::try_parse_from(["tono", "song.mp3", "--instrument", name]).unwrap();
            assert_eq!(cli.options.selected_instrument().unwrap(), expected);
        }
        let cli =
            Cli::try_parse_from(["tono", "song.mp3", "--piano", "--tempo-scale", "0.75"]).unwrap();
        assert_eq!(
            cli.options.selected_instrument().unwrap(),
            Instrument::Piano
        );
        let options = cli
            .options
            .into_options(cli.input.unwrap(), PathBuf::from("out.tono"), None)
            .unwrap();
        assert_eq!(options.instrument, Instrument::Piano);
        assert_eq!(options.tempo_scale, 0.75);
        assert!(
            Cli::try_parse_from(["tono", "song.mp3", "--piano", "--instrument", "ae01"]).is_err()
        );
        assert!(Cli::try_parse_from(["tono", "prep", "song.mp3", "--piano"]).is_ok());
    }

    #[test]
    fn every_instrument_keeps_the_shared_cli_controls() {
        for name in [
            "ae01",
            "ae05",
            "ae10",
            "ae20",
            "guitar",
            "guitar-bass",
            "guitar-bass-5string",
            "piano",
            "keyboard-76",
            "keyboard-61",
        ] {
            let cli = Cli::try_parse_from([
                "tono",
                "song.mp3",
                "practice.mp4",
                "--instrument",
                name,
                "--tempo-scale",
                "0.5",
                "--metronome",
                "both",
            ])
            .unwrap();
            let options = cli
                .options
                .into_options(
                    cli.input.unwrap(),
                    PathBuf::from("practice.tono"),
                    cli.output,
                )
                .unwrap();
            assert_eq!(options.instrument.id(), name);
            assert_eq!(options.tempo_scale, 0.5);
            assert_eq!(options.metronome, MetronomeMode::Both);
        }
        assert!(
            Cli::try_parse_from(["tono", "song.mp3", "out.mp4", "--instrument", "ae99"]).is_err()
        );
    }

    #[test]
    fn short_form_accepts_spaces_and_preserves_intervals_by_default() {
        let cli = Cli::try_parse_from([
            "tono",
            "my song.mp3",
            "my practice.mp4",
            "--instrument",
            "ae01",
        ])
        .unwrap();
        assert!(cli.command.is_none());
        let options = cli
            .options
            .into_options(
                cli.input.unwrap(),
                PathBuf::from("my practice.tono"),
                cli.output,
            )
            .unwrap();
        assert_eq!(options.input, PathBuf::from("my song.mp3"));
        assert_eq!(options.range_policy, RangePolicy::Strict);
        assert_eq!(options.part, Part::Vocal);
        assert!(options.auto_song_region);
    }

    #[test]
    fn short_form_allows_explicit_overrides() {
        let cli = Cli::try_parse_from([
            "tono",
            "song.mp4",
            "practice.mp4",
            "--instrument",
            "ae01",
            "--range-policy",
            "strict",
            "--tempo-scale",
            "0.75",
            "--part",
            "lead",
        ])
        .unwrap();
        let options = cli
            .options
            .into_options(
                cli.input.unwrap(),
                PathBuf::from("practice.tono"),
                cli.output,
            )
            .unwrap();
        assert_eq!(options.range_policy, RangePolicy::Strict);
        assert_eq!(options.tempo_scale, 0.75);
        assert_eq!(options.part, Part::Lead);
    }

    #[test]
    fn direct_form_retains_controls_and_requires_instrument() {
        let cli = Cli::try_parse_from([
            "tono",
            "song.mp4",
            "out.mp4",
            "--instrument",
            "ae01",
            "--from",
            "00:12",
            "--to",
            "00:40",
            "--auto-song-region",
            "false",
            "--count-in",
            "0",
            "--upper-hand",
            "left",
            "--metronome",
            "both",
            "--keep-work",
        ])
        .unwrap();
        assert_eq!(cli.options.count_in, 0);
        assert_eq!(cli.options.upper_hand, Some(UpperHand::Left));
        assert_eq!(cli.options.metronome, MetronomeMode::Both);
        assert_eq!(cli.options.from_time.as_deref(), Some("00:12"));
        assert_eq!(cli.options.to_time.as_deref(), Some("00:40"));
        assert!(!cli.options.auto_song_region);
        assert!(cli.options.keep_work);
        assert!(Cli::try_parse_from([
            "tono",
            "song.mp4",
            "out.mp4",
            "--instrument",
            "ae01",
            "--count-in",
            "5"
        ])
        .is_err());
    }

    #[test]
    fn legacy_commands_keep_working() {
        assert!(matches!(
            Cli::try_parse_from(["tono", "doctor", "--ml"])
                .unwrap()
                .command,
            Some(Commands::Doctor { ml: true })
        ));
        assert!(matches!(
            Cli::try_parse_from(["tono", "demo"]).unwrap().command,
            Some(Commands::Demo { .. })
        ));
        let cli = Cli::try_parse_from([
            "tono",
            "prep",
            "song.mp3",
            "--out",
            "out",
            "--instrument",
            "ae01",
        ])
        .unwrap();
        let Some(Commands::Prep {
            input,
            out,
            options,
        }) = cli.command
        else {
            panic!("prep expected")
        };
        assert_eq!(
            options.into_options(input, out, None).unwrap().range_policy,
            RangePolicy::Strict
        );
    }

    #[test]
    fn legacy_fold_cannot_change_individual_notes() {
        let cli = Cli::try_parse_from([
            "tono",
            "song.mp3",
            "out.mp4",
            "--instrument",
            "ae01",
            "--range-policy",
            "fold",
        ])
        .unwrap();
        assert!(cli
            .options
            .into_options(cli.input.unwrap(), PathBuf::from("out.tono"), cli.output)
            .err()
            .unwrap()
            .to_string()
            .contains("folding is disabled"));
    }

    #[test]
    fn easy_fingering_is_opt_in_and_reaches_pipeline_for_all_instruments() {
        for instrument in ["ae01", "guitar"] {
            let cli = Cli::try_parse_from([
                "tono",
                "song.mp3",
                "--instrument",
                instrument,
                "--easy-fingering",
            ])
            .unwrap();
            let options = cli
                .options
                .into_options(cli.input.unwrap(), PathBuf::from("out.tono"), None)
                .unwrap();
            assert!(options.easy_fingering);
        }
        let cli = Cli::try_parse_from(["tono", "song.mp3", "--instrument", "ae01"]).unwrap();
        assert!(!cli.options.easy_fingering);
    }

    #[test]
    fn missing_instrument_and_unknown_options_fail() {
        assert!(
            Cli::try_parse_from(["tono", "song.mp3", "practice.mp4"]).is_err(),
            "instrument must be explicit"
        );
        assert!(Cli::try_parse_from(["tono", "song.mp3"]).is_err());
        assert!(Cli::try_parse_from(["tono", "song.mp3", "practice.mp4", "--typo"]).is_err());
    }

    #[test]
    fn automatic_output_and_directory_flags_keep_instrument_required() {
        let cli = Cli::try_parse_from(["tono", "song.mp3", "--instrument", "ae20"]).unwrap();
        assert!(cli.output.is_none());
        assert!(cli.practice_dir.is_none());
        for flag in ["-d", "--practice-dir"] {
            let cli = Cli::try_parse_from([
                "tono",
                "song.mp3",
                "--instrument",
                "guitar",
                flag,
                "my practices",
                "--tempo-scale",
                "0.75",
            ])
            .unwrap();
            assert_eq!(cli.practice_dir, Some(PathBuf::from("my practices")));
            assert_eq!(cli.options.tempo_scale, 0.75);
        }
        assert!(Cli::try_parse_from([
            "tono",
            "song.mp3",
            "out.mp4",
            "--instrument",
            "ae20",
            "-d",
            "elsewhere"
        ])
        .is_err());
    }

    #[test]
    fn direct_output_keeps_html_video_and_data_together() {
        let cli = Cli::try_parse_from(["tono", "song.mp3", "custom.mp4", "--instrument", "ae01"])
            .unwrap();
        let options =
            short_form_options(cli.input.unwrap(), cli.output.unwrap(), cli.options).unwrap();
        assert_eq!(options.output_directory, PathBuf::from("custom"));
        assert_eq!(
            options.output_video,
            Some(PathBuf::from("custom/practice.mp4"))
        );
    }

    #[test]
    fn refuses_invalid_output_and_existing_input_as_destination() {
        let cli =
            Cli::try_parse_from(["tono", "song.mp3", "out.wav", "--instrument", "ae01"]).unwrap();
        assert!(short_form_options(cli.input.unwrap(), cli.output.unwrap(), cli.options).is_err());
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("tono-cli-test-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("original.mp4");
        std::fs::write(&path, b"original media").unwrap();
        let cli =
            Cli::try_parse_from(["tono", "song.mp3", "out.mp4", "--instrument", "ae01"]).unwrap();
        assert!(short_form_options(path.clone(), path.clone(), cli.options).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"original media");
        let output = directory.join("existing.mp4");
        std::fs::create_dir(output.with_extension("tono")).unwrap();
        let cli =
            Cli::try_parse_from(["tono", "song.mp3", "out.mp4", "--instrument", "ae01"]).unwrap();
        assert!(short_form_options(path, output, cli.options).is_ok());
        std::fs::remove_dir_all(directory).unwrap();
    }
}
