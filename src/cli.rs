//! CLI arguments and command dispatch.
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand};

use tono::commands::{demo, doctor};
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
    after_help = "Example: tono song.mp3 --instrument ae01\nThe default preserves melodic intervals using a whole-melody octave shift, or reports that it cannot fit.\nIndividual octave folding is disabled. Outputs default to ./tono-practices/<song>_<instrument>_<YYYYMMDD>.mp4; reruns replace matching outputs."
)]
struct Cli {
    /// Local audio/video file (mp3/m4a/wav/flac/mov/mp4).
    #[arg(value_name = "INPUT", required = true)]
    input: Option<PathBuf>,
    /// Optional explicit MP4 filename (overrides automatic naming).
    #[arg(value_name = "OUTPUT.mp4")]
    output: Option<PathBuf>,
    /// Directory for automatically named practice videos.
    #[arg(long, short = 'd', value_name = "DIRECTORY", conflicts_with = "output")]
    practice_dir: Option<PathBuf>,
    #[command(flatten)]
    options: PreparationArgs,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Args)]
struct PreparationArgs {
    /// Instrument profile, fingering table and diagram.
    #[arg(long, required = true, value_enum)]
    instrument: Option<Instrument>,
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
    /// Hand used on the upper three keys; changes labels only.
    #[arg(long, value_enum)]
    upper_hand: Option<UpperHand>,
    #[arg(long, default_value_t = 1.0)]
    tempo_scale: f64,
    #[arg(long, value_enum, default_value_t = MetronomeMode::Visual)]
    metronome: MetronomeMode,
    /// Octave fitting: strict preserves intervals (default); legacy fold is rejected.
    #[arg(long, value_enum)]
    range_policy: Option<RangePolicy>,
    /// Transcribed notes below this confidence are discarded.
    #[arg(long, default_value_t = 0.3)]
    min_note_confidence: f64,
    /// Preserve intermediate files and diagnostic logs.
    #[arg(long)]
    keep_work: bool,
}

impl PreparationArgs {
    fn into_options(
        self,
        input: PathBuf,
        out: PathBuf,
        video: Option<PathBuf>,
    ) -> Result<prep::PrepOptions> {
        if self.range_policy == Some(RangePolicy::Fold) {
            bail!("individual octave folding is disabled; use --range-policy strict to preserve the melody");
        }
        let instrument = self.instrument.context("--instrument is required")?;
        if !(0.0..=1.0).contains(&self.min_note_confidence) {
            bail!("--min-note-confidence must be between 0 and 1");
        }
        Ok(prep::PrepOptions {
            input,
            instrument,
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
        })
    }
}

#[derive(Subcommand)]
enum Commands {
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

/// Separate durable project data for each requested MP4, without using its
/// parent as a work directory or overwriting the source.
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
    let project = output.with_extension("tono");
    output::validate_destination(&input, &output)?;
    options.into_options(input, project, Some(output))
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Commands::Doctor { ml }) => doctor::run_doctor(ml),
        Some(Commands::Demo {
            instrument,
            out,
            bpm,
            backing,
            metronome,
            keep_work,
        }) => demo::run_demo(&demo::DemoOptions {
            instrument,
            output_directory: out,
            beats_per_minute: bpm,
            backing_audio: backing,
            metronome,
            keep_work,
        }),
        Some(Commands::Prep {
            input,
            out,
            options,
        }) => prep::run_prep(&options.into_options(input, out, None)?),
        None => {
            let input = cli.input.expect("required by clap");
            let destination = match cli.output {
                Some(path) => path,
                None => output::automatic_path(
                    &input,
                    cli.options.instrument.expect("required by clap"),
                    cli.practice_dir.as_deref(),
                    &output::local_date()?,
                )?,
            };
            let options = short_form_options(input, destination, cli.options)?;
            output::run(&options)
        }
    }
}

#[cfg(test)]
mod cli_tests {
    use super::*;

    #[test]
    fn every_instrument_keeps_the_shared_cli_controls() {
        for name in ["ae01", "ae05", "ae10", "ae20", "guitar"] {
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
