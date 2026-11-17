//! The preview's sound: which audio session is open and what the Audio dropdown shows
//! (data-model.md "Audio session", "Audio state"). Pure, as `preview.rs`: monitor events, session
//! events and the video session's open and close go in, commands for the app come out. No
//! platform call, no settings write.

use ::audio::choice::{self, AudioChoice, Resolved};
use ::audio::pairing::{self, VideoKey};
use platform::audio::{AudioEvent, Endpoint, SessionEvent, SessionRequest, Side};
use settings::Settings;

/// What the Audio dropdown shows (data-model.md "Audio state"); never shown while no video device
/// is open (FR-018).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioState {
    /// No video device open, or nothing to play.
    Off,
    /// The session was asked for (or waits for the audio list); no sound yet.
    Opening,
    /// Both sides run.
    Playing,
    /// Auto and no input pairs with the video device.
    NoPair,
}

/// The video device whose sound is wanted, as the app read it when its session opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoDevice {
    pub id: String,
    /// The system's name, without the app's " (2)" numbering.
    pub base_name: String,
    /// `platform::audio::video_link` of the device; None when the system gives none.
    pub link: Option<String>,
}

/// What goes into [`AudioController::handle`].
#[derive(Debug, Clone)]
pub enum Event {
    Monitor(AudioEvent),
    Session(SessionEvent),
    /// A video session started (preview states Opening, Playing, No signal follow it).
    VideoOpened {
        session: u64,
        device: VideoDevice,
    },
    /// The video session ended.
    VideoClosed {
        session: u64,
    },
}

impl Event {
    /// A short description for the log line of a state change (the event's reason).
    pub fn reason(&self) -> String {
        match self {
            Self::Monitor(AudioEvent::Listed { inputs, outputs, .. }) => {
                format!("audio list, {} inputs, {} outputs", inputs.len(), outputs.len())
            }
            Self::Monitor(AudioEvent::Added(endpoint)) => format!("{} added", endpoint.id),
            Self::Monitor(AudioEvent::Removed(id)) => format!("{id} removed"),
            Self::Monitor(AudioEvent::DefaultOutput(id)) => format!("default output {id:?}"),
            Self::Session(event) => format!("audio session: {event:?}"),
            Self::VideoOpened { session, device } => format!("video session {session} opened on {}", device.id),
            Self::VideoClosed { session } => format!("video session {session} closed"),
        }
    }
}

/// What the app does for the preview's sound. Every command returns at once (research R9).
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Open this session; a session still open is closed first by [`Command::Close`].
    Open(SessionRequest),
    /// Reopen the open session's input on this endpoint (the bridge restarts empty).
    #[expect(dead_code, reason = "emitted by the input choice (T034)")]
    ReopenInput { input: String, exclusive: bool },
    /// Reopen the open session's output (None: the system default).
    #[expect(dead_code, reason = "emitted by the output choice (T034)")]
    ReopenOutput { output: Option<String>, exclusive: bool },
    /// Close the open session.
    Close,
    /// Set the open bridge's gain (`audio::volume::gain`).
    #[expect(dead_code, reason = "emitted by the volume control (T032)")]
    SetGain(f32),
}

/// The audio session asked for.
#[derive(Debug, Clone)]
struct Open {
    number: u64,
    /// The sides that reported `Opened`.
    input: bool,
    output: bool,
}

#[derive(Debug, Default)]
pub struct AudioController {
    state: Option<AudioState>,
    /// The monitor's first list arrived.
    listed: bool,
    inputs: Vec<Endpoint>,
    outputs: Vec<Endpoint>,
    default_output: Option<String>,
    /// The open video session and its device.
    video: Option<(u64, VideoDevice)>,
    open: Option<Open>,
    /// Audio session numbers handed out so far.
    sessions: u64,
}

impl AudioController {
    pub fn new() -> Self {
        Self::default()
    }

    /// What the Audio dropdown shows.
    pub fn state(&self) -> AudioState {
        self.state.clone().unwrap_or(AudioState::Off)
    }

    /// The number of the audio session open or opening; events and samples of others are dropped.
    #[cfg_attr(not(test), expect(dead_code, reason = "read by the stats lines (T025)"))]
    pub fn session(&self) -> Option<u64> {
        self.open.as_ref().map(|o| o.number)
    }

    /// The system default output's id, as the monitor last said.
    #[cfg_attr(not(test), expect(dead_code, reason = "shown in the Output menu (T034)"))]
    pub fn default_output(&self) -> Option<&str> {
        self.default_output.as_deref()
    }

    pub fn handle(&mut self, event: Event, settings: &Settings) -> Vec<Command> {
        let mut out = Vec::new();
        match event {
            Event::Monitor(AudioEvent::Listed {
                inputs,
                outputs,
                default_output,
            }) => {
                self.inputs = inputs;
                self.outputs = outputs;
                self.default_output = default_output;
                self.listed = true;
                if self.open.is_none() {
                    self.resolve(settings, &mut out);
                }
            }
            Event::Monitor(AudioEvent::Added(endpoint)) => {
                let list = match endpoint.flow {
                    platform::audio::Flow::Input => &mut self.inputs,
                    platform::audio::Flow::Output => &mut self.outputs,
                };
                list.retain(|e| e.id != endpoint.id);
                list.push(endpoint);
                platform::audio::numbered(list);
                // An input that pairs with the video device may have come.
                if self.state == Some(AudioState::NoPair) {
                    self.resolve(settings, &mut out);
                }
            }
            Event::Monitor(AudioEvent::Removed(id)) => {
                self.inputs.retain(|e| e.id != id);
                self.outputs.retain(|e| e.id != id);
                platform::audio::numbered(&mut self.inputs);
                platform::audio::numbered(&mut self.outputs);
            }
            Event::Monitor(AudioEvent::DefaultOutput(id)) => self.default_output = id,
            Event::Session(event) => self.session_event(event),
            Event::VideoOpened { session, device } => {
                self.close(&mut out);
                self.video = Some((session, device));
                self.resolve(settings, &mut out);
            }
            Event::VideoClosed { session } => {
                if self.video.as_ref().is_some_and(|(open, _)| *open == session) {
                    self.video = None;
                    self.close(&mut out);
                    self.state = Some(AudioState::Off);
                }
            }
        }
        out
    }

    /// Opens the video device's sound, or shows why there is none.
    fn resolve(&mut self, _settings: &Settings, out: &mut Vec<Command>) {
        let Some((_, video)) = &self.video else {
            self.state = Some(AudioState::Off);
            return;
        };
        if !self.listed {
            // The first list resolves it.
            self.state = Some(AudioState::Opening);
            return;
        }
        let endpoints: Vec<pairing::Endpoint> = self
            .inputs
            .iter()
            .map(|e| pairing::Endpoint {
                id: e.id.clone(),
                name: e.base_name.clone(),
                link: e.link.clone(),
            })
            .collect();
        let key = VideoKey {
            link: video.link.clone(),
            name: video.base_name.clone(),
        };
        let pair = pairing::pair(&key, &endpoints);
        let ids: Vec<&str> = self.inputs.iter().map(|e| e.id.as_str()).collect();
        match choice::resolve(&AudioChoice::Auto, pair, &ids) {
            Resolved::Play(input) => {
                self.sessions += 1;
                let number = self.sessions;
                self.open = Some(Open {
                    number,
                    input: false,
                    output: false,
                });
                self.state = Some(AudioState::Opening);
                out.push(Command::Open(SessionRequest {
                    number,
                    input,
                    output: None,
                    input_exclusive: false,
                    output_exclusive: false,
                }));
            }
            Resolved::NoPair => self.state = Some(AudioState::NoPair),
            Resolved::Missing(_) | Resolved::Off => self.state = Some(AudioState::Off),
        }
    }

    /// Closes the open audio session, if any.
    fn close(&mut self, out: &mut Vec<Command>) {
        if self.open.take().is_some() {
            out.push(Command::Close);
        }
    }

    fn session_event(&mut self, event: SessionEvent) {
        let number = match &event {
            SessionEvent::Opened { number, .. }
            | SessionEvent::Error { number, .. }
            | SessionEvent::Closed { number } => *number,
        };
        let Some(open) = self.open.as_mut().filter(|o| o.number == number) else {
            // An older session's: no sound plays after its video is gone (research R9).
            return;
        };
        match event {
            SessionEvent::Opened { side, .. } => {
                match side {
                    Side::Input => open.input = true,
                    Side::Output => open.output = true,
                }
                if open.input && open.output {
                    self.state = Some(AudioState::Playing);
                }
            }
            SessionEvent::Error { side, error, .. } => {
                // The error states come with the input and output choices (T034, T038, T041).
                log::warn!("audio session {number}: {side:?} side: {error}");
            }
            SessionEvent::Closed { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use platform::audio::{AudioError, Flow, Format, SampleFormat, StreamMode};

    use super::*;

    const CARD_VIDEO: &str = "mf:ezcap";
    const CARD_INPUT: &str = "{0.0.1.00000000}.{card}";
    const MIC_INPUT: &str = "{0.0.1.00000000}.{mic}";
    const SPEAKERS: &str = "{0.0.0.00000000}.{speakers}";
    const CARD_LINK: &str = "6c1b7c3a-0d4e-4a7f-9b1e-2f3c4d5e6f70";

    fn endpoint(id: &str, name: &str, flow: Flow, link: Option<&str>) -> Endpoint {
        Endpoint {
            id: id.into(),
            name: name.into(),
            base_name: name.into(),
            flow,
            link: link.map(Into::into),
            format: None,
        }
    }

    fn listed() -> Event {
        Event::Monitor(AudioEvent::Listed {
            inputs: vec![
                endpoint(MIC_INPUT, "Microphone (Realtek(R) Audio)", Flow::Input, None),
                endpoint(
                    CARD_INPUT,
                    "Digital Audio Interface (ezcap)",
                    Flow::Input,
                    Some(CARD_LINK),
                ),
            ],
            outputs: vec![endpoint(SPEAKERS, "Speakers (Realtek(R) Audio)", Flow::Output, None)],
            default_output: Some(SPEAKERS.into()),
        })
    }

    fn card() -> VideoDevice {
        VideoDevice {
            id: CARD_VIDEO.into(),
            base_name: "ezcap Game Link".into(),
            link: Some(CARD_LINK.into()),
        }
    }

    /// A webcam with no link and no input of its name.
    fn lonely_camera() -> VideoDevice {
        VideoDevice {
            id: "mf:camera".into(),
            base_name: "NVIDIA Broadcast".into(),
            link: None,
        }
    }

    fn opened(number: u64, side: Side) -> Event {
        Event::Session(SessionEvent::Opened {
            number,
            side,
            mode: StreamMode::Shared,
            format: Format {
                rate: 48000,
                channels: 2,
                sample: SampleFormat::I16,
            },
            period_ms: 10.0,
            reported_latency_ms: None,
        })
    }

    fn request(number: u64, input: &str) -> Command {
        Command::Open(SessionRequest {
            number,
            input: input.into(),
            output: None,
            input_exclusive: false,
            output_exclusive: false,
        })
    }

    struct Run {
        audio: AudioController,
        settings: Settings,
    }

    impl Run {
        fn new() -> Self {
            Self {
                audio: AudioController::new(),
                settings: Settings::default(),
            }
        }

        fn send(&mut self, event: Event) -> Vec<Command> {
            self.audio.handle(event, &self.settings)
        }
    }

    #[test]
    fn opens_with_the_video_and_closes_with_it() {
        let mut run = Run::new();
        assert!(run.send(listed()).is_empty());
        assert_eq!(run.audio.state(), AudioState::Off);

        let commands = run.send(Event::VideoOpened {
            session: 1,
            device: card(),
        });
        assert_eq!(commands, [request(1, CARD_INPUT)]);
        assert_eq!(run.audio.state(), AudioState::Opening);
        assert_eq!(run.audio.session(), Some(1));

        assert!(run.send(opened(1, Side::Output)).is_empty());
        assert_eq!(run.audio.state(), AudioState::Opening, "one side is not enough");
        run.send(opened(1, Side::Input));
        assert_eq!(run.audio.state(), AudioState::Playing);

        assert_eq!(run.send(Event::VideoClosed { session: 1 }), [Command::Close]);
        assert_eq!(run.audio.state(), AudioState::Off);
        assert_eq!(run.audio.session(), None);
        // Its acknowledgment changes nothing.
        assert!(run.send(Event::Session(SessionEvent::Closed { number: 1 })).is_empty());
        assert_eq!(run.audio.state(), AudioState::Off);
    }

    #[test]
    fn a_video_opened_before_the_audio_list_waits_for_it() {
        let mut run = Run::new();
        assert!(
            run.send(Event::VideoOpened {
                session: 1,
                device: card(),
            })
            .is_empty()
        );
        assert_eq!(run.audio.state(), AudioState::Opening);
        assert_eq!(run.send(listed()), [request(1, CARD_INPUT)]);
    }

    #[test]
    fn a_video_switch_while_the_audio_opens_closes_it_and_opens_the_next() {
        let mut run = Run::new();
        run.send(listed());
        run.send(Event::VideoOpened {
            session: 1,
            device: card(),
        });
        // The preview closes the old video session and opens the next one.
        assert_eq!(run.send(Event::VideoClosed { session: 1 }), [Command::Close]);
        assert_eq!(
            run.send(Event::VideoOpened {
                session: 2,
                device: card(),
            }),
            [request(2, CARD_INPUT)]
        );
        // The first session's late events are ignored.
        run.send(opened(1, Side::Input));
        run.send(opened(1, Side::Output));
        assert_eq!(run.audio.state(), AudioState::Opening);
        run.send(opened(2, Side::Input));
        run.send(opened(2, Side::Output));
        assert_eq!(run.audio.state(), AudioState::Playing);
    }

    #[test]
    fn a_video_opened_without_a_close_replaces_the_session() {
        let mut run = Run::new();
        run.send(listed());
        run.send(Event::VideoOpened {
            session: 1,
            device: card(),
        });
        let commands = run.send(Event::VideoOpened {
            session: 2,
            device: card(),
        });
        assert_eq!(commands, [Command::Close, request(2, CARD_INPUT)]);
        // The old video session's close comes late: the new one stays.
        assert!(run.send(Event::VideoClosed { session: 1 }).is_empty());
        assert_eq!(run.audio.session(), Some(2));
    }

    #[test]
    fn no_paired_input_opens_nothing_until_one_comes() {
        let mut run = Run::new();
        run.send(listed());
        let camera = lonely_camera();
        assert!(
            run.send(Event::VideoOpened {
                session: 1,
                device: camera.clone(),
            })
            .is_empty()
        );
        assert_eq!(run.audio.state(), AudioState::NoPair);
        // An unrelated output changes nothing; an input with the camera's name pairs.
        assert!(
            run.send(Event::Monitor(AudioEvent::Added(endpoint(
                "{0.0.0.00000000}.{hdmi}",
                "NVIDIA Broadcast",
                Flow::Output,
                None,
            ))))
            .is_empty()
        );
        let mic = "{0.0.1.00000000}.{broadcast}";
        let commands = run.send(Event::Monitor(AudioEvent::Added(endpoint(
            mic,
            "Microphone (NVIDIA Broadcast)",
            Flow::Input,
            None,
        ))));
        assert_eq!(commands, [request(1, mic)]);
        assert_eq!(run.audio.state(), AudioState::Opening);
        // No pair after the video closes is Off.
        run.send(Event::VideoClosed { session: 1 });
        assert_eq!(run.audio.state(), AudioState::Off);
    }

    #[test]
    fn a_late_opened_of_an_old_session_is_ignored() {
        let mut run = Run::new();
        run.send(listed());
        run.send(Event::VideoOpened {
            session: 1,
            device: card(),
        });
        run.send(Event::VideoClosed { session: 1 });
        run.send(Event::VideoOpened {
            session: 2,
            device: lonely_camera(),
        });
        assert_eq!(run.audio.state(), AudioState::NoPair);
        run.send(opened(1, Side::Input));
        run.send(opened(1, Side::Output));
        run.send(Event::Session(SessionEvent::Error {
            number: 1,
            side: Side::Input,
            error: AudioError::Gone,
        }));
        assert_eq!(run.audio.state(), AudioState::NoPair);
        assert_eq!(run.audio.session(), None);
    }

    #[test]
    fn the_monitor_keeps_its_lists_and_the_default_output() {
        let mut run = Run::new();
        run.send(listed());
        assert_eq!(run.audio.default_output(), Some(SPEAKERS));
        run.send(Event::Monitor(AudioEvent::DefaultOutput(None)));
        assert_eq!(run.audio.default_output(), None);
        // The card's input leaves before its video opens: nothing pairs by link.
        run.send(Event::Monitor(AudioEvent::Removed(CARD_INPUT.into())));
        run.send(Event::VideoOpened {
            session: 1,
            device: card(),
        });
        assert_eq!(run.audio.state(), AudioState::NoPair);
    }
}
