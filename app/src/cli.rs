//! Command line (contracts/cli.md): start options for users, developer options for measurements.

use clap::{Parser, ValueEnum};

/// Exit code for an invalid command line.
pub const EXIT_USAGE: i32 = 64;

#[derive(Parser, Debug)]
#[command(
    name = "capturax",
    version,
    about = "Watch and hear capture cards with the lowest possible latency"
)]
struct Args {
    /// Start in fullscreen for this start only
    #[arg(long, conflicts_with = "windowed")]
    fullscreen: bool,

    /// Start windowed for this start only
    #[arg(long)]
    windowed: bool,

    /// Started by the system's autostart entry
    #[arg(long)]
    autostart: bool,

    /// Developer: play a moving test video (1080p60, 1440p60 or WxH@FPS)
    #[arg(long, value_name = "PRESET|WxH@FPS", value_parser = parse_video_size)]
    test_video: Option<VideoSize>,

    /// Developer: print presentation statistics once per second
    #[arg(long)]
    stats: bool,

    /// Developer: list the capture devices with their modes, then exit
    #[arg(long)]
    list_devices: bool,

    /// Developer: force the graphics backend
    #[arg(long, value_enum)]
    backend: Option<Backend>,

    /// Developer: force the present mode
    #[arg(long, value_enum)]
    present: Option<PresentMode>,

    /// Use this language for this start, without saving it
    #[arg(long, value_name = "CODE")]
    lang: Option<String>,
}

/// Window mode asked for this start, overriding the saved setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartMode {
    Fullscreen,
    Windowed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Backend {
    Dx12,
    Vulkan,
    Gl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PresentMode {
    Mailbox,
    Immediate,
    Fifo,
}

/// Size and frame rate of the test video.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoSize {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

/// What the command line asks for this start (data-model.md, Startup options); never saved.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StartupOptions {
    pub start_mode: Option<StartMode>,
    pub autostart: bool,
    pub test_video: Option<VideoSize>,
    pub stats: bool,
    pub list_devices: bool,
    pub backend: Option<Backend>,
    pub present: Option<PresentMode>,
    pub lang: Option<String>,
}

impl StartupOptions {
    /// Whether this start opens in fullscreen: `--fullscreen` or `--windowed` win over the saved
    /// Start in fullscreen setting, for this start only (FR-012a).
    pub fn starts_fullscreen(&self, saved: bool) -> bool {
        match self.start_mode {
            Some(StartMode::Fullscreen) => true,
            Some(StartMode::Windowed) => false,
            None => saved,
        }
    }
}

/// The command line could not be used: show `message` and exit with `code`.
#[derive(Debug)]
pub struct CliExit {
    pub code: i32,
    pub message: String,
    /// The message is help or version text for stdout, not an error.
    pub info: bool,
}

/// Parses the arguments (the first is the program name).
pub fn parse<I, T>(args: I) -> Result<StartupOptions, CliExit>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let args = Args::try_parse_from(args).map_err(|err| {
        use clap::error::ErrorKind;
        let info = matches!(err.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion);
        CliExit {
            code: if info { 0 } else { EXIT_USAGE },
            message: err.render().to_string(),
            info,
        }
    })?;
    let start_mode = if args.fullscreen {
        Some(StartMode::Fullscreen)
    } else if args.windowed {
        Some(StartMode::Windowed)
    } else {
        None
    };
    Ok(StartupOptions {
        start_mode,
        autostart: args.autostart,
        test_video: args.test_video,
        stats: args.stats,
        list_devices: args.list_devices,
        backend: args.backend,
        present: args.present,
        lang: args.lang,
    })
}

/// `1080p60`, `1440p60` or `WxH@FPS`.
fn parse_video_size(text: &str) -> Result<VideoSize, String> {
    match text {
        "1080p60" => {
            return Ok(VideoSize {
                width: 1920,
                height: 1080,
                fps: 60,
            });
        }
        "1440p60" => {
            return Ok(VideoSize {
                width: 2560,
                height: 1440,
                fps: 60,
            });
        }
        _ => {}
    }
    let bad = || format!("{text:?} is not 1080p60, 1440p60 or WxH@FPS (for example 1920x1080@60)");
    let (size, fps) = text.split_once('@').ok_or_else(bad)?;
    let (width, height) = size.split_once(['x', 'X']).ok_or_else(bad)?;
    let number = |s: &str| s.parse::<u32>().ok().filter(|n| *n > 0).ok_or_else(bad);
    Ok(VideoSize {
        width: number(width)?,
        height: number(height)?,
        fps: number(fps)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> Result<StartupOptions, CliExit> {
        parse(std::iter::once("capturax").chain(args.iter().copied()))
    }

    #[test]
    fn no_options() {
        assert_eq!(run(&[]).unwrap(), StartupOptions::default());
    }

    #[test]
    fn start_options() {
        assert_eq!(run(&["--fullscreen"]).unwrap().start_mode, Some(StartMode::Fullscreen));
        assert_eq!(run(&["--windowed"]).unwrap().start_mode, Some(StartMode::Windowed));
        assert!(run(&["--autostart"]).unwrap().autostart);
    }

    #[test]
    fn start_options_override_the_saved_setting() {
        for saved in [false, true] {
            assert_eq!(run(&[]).unwrap().starts_fullscreen(saved), saved);
            assert!(run(&["--fullscreen"]).unwrap().starts_fullscreen(saved));
            assert!(!run(&["--windowed"]).unwrap().starts_fullscreen(saved));
        }
    }

    #[test]
    fn fullscreen_and_windowed_conflict() {
        let err = run(&["--fullscreen", "--windowed"]).unwrap_err();
        assert_eq!(err.code, EXIT_USAGE);
        assert!(!err.info);
    }

    #[test]
    fn developer_options() {
        let o = run(&[
            "--stats",
            "--backend",
            "dx12",
            "--present",
            "mailbox",
            "--lang",
            "pt-BR",
        ])
        .unwrap();
        assert!(o.stats);
        assert!(!o.list_devices);
        assert!(run(&["--list-devices"]).unwrap().list_devices);
        assert_eq!(o.backend, Some(Backend::Dx12));
        assert_eq!(o.present, Some(PresentMode::Mailbox));
        assert_eq!(o.lang.as_deref(), Some("pt-BR"));
        assert_eq!(run(&["--backend", "vulkan"]).unwrap().backend, Some(Backend::Vulkan));
        assert_eq!(run(&["--backend", "gl"]).unwrap().backend, Some(Backend::Gl));
        assert_eq!(
            run(&["--present", "immediate"]).unwrap().present,
            Some(PresentMode::Immediate)
        );
        assert_eq!(run(&["--present", "fifo"]).unwrap().present, Some(PresentMode::Fifo));
        assert_eq!(run(&["--backend", "metal"]).unwrap_err().code, EXIT_USAGE);
        assert_eq!(run(&["--present", "vsync"]).unwrap_err().code, EXIT_USAGE);
    }

    #[test]
    fn test_video_presets() {
        let size = |t: &str| run(&["--test-video", t]).unwrap().test_video.unwrap();
        assert_eq!(
            size("1080p60"),
            VideoSize {
                width: 1920,
                height: 1080,
                fps: 60
            }
        );
        assert_eq!(
            size("1440p60"),
            VideoSize {
                width: 2560,
                height: 1440,
                fps: 60
            }
        );
        assert_eq!(
            size("1280x720@30"),
            VideoSize {
                width: 1280,
                height: 720,
                fps: 30
            }
        );
        assert_eq!(
            size("3840X2160@120"),
            VideoSize {
                width: 3840,
                height: 2160,
                fps: 120
            }
        );
    }

    #[test]
    fn bad_test_video() {
        for bad in [
            "720p",
            "1920x1080",
            "1920@60",
            "0x1080@60",
            "1920x1080@0",
            "axb@c",
            "1920x1080@60x",
            "",
        ] {
            assert_eq!(run(&["--test-video", bad]).unwrap_err().code, EXIT_USAGE, "{bad:?}");
        }
    }

    #[test]
    fn unknown_option() {
        assert_eq!(run(&["--nope"]).unwrap_err().code, EXIT_USAGE);
    }

    #[test]
    fn help_is_not_an_error() {
        let help = run(&["--help"]).unwrap_err();
        assert_eq!(help.code, 0);
        assert!(help.info);
    }
}
