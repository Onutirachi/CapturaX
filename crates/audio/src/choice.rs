//! The user's audio choices (data-model.md "Audio choice per video device", "Output choice"):
//! their settings texts and what they resolve to with the devices present now.

use std::fmt;

/// What a video device plays as its sound (`[devices."<id>"] audio`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AudioChoice {
    /// The paired input; nothing when there is none.
    #[default]
    Auto,
    /// Nothing, with no state.
    None,
    /// That input, by endpoint id; kept while the input is missing (it may return).
    Input(String),
}

impl AudioChoice {
    /// The choice for a settings text: `"auto"`, `"none"` or an endpoint id; empty → `Auto`.
    pub fn parse(text: &str) -> Self {
        match text {
            "" | "auto" => Self::Auto,
            "none" => Self::None,
            id => Self::Input(id.to_string()),
        }
    }
}

impl fmt::Display for AudioChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Auto => f.write_str("auto"),
            Self::None => f.write_str("none"),
            Self::Input(id) => f.write_str(id),
        }
    }
}

/// Where the preview's sound plays (`[audio] output`), one for the app.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum OutputChoice {
    /// The system's current default, following its changes.
    #[default]
    Default,
    /// That output, by endpoint id; while missing, the default with the fallback shown.
    Device(String),
}

impl OutputChoice {
    /// The choice for a settings text: `"default"` or an endpoint id; empty → `Default`.
    pub fn parse(text: &str) -> Self {
        match text {
            "" | "default" => Self::Default,
            id => Self::Device(id.to_string()),
        }
    }
}

impl fmt::Display for OutputChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default => f.write_str("default"),
            Self::Device(id) => f.write_str(id),
        }
    }
}

/// What a video device's audio choice gives with the inputs present now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved {
    /// Open this input.
    Play(String),
    /// `Auto` and no paired input (state No paired input).
    NoPair,
    /// The chosen input is not present (state Disconnected until it returns).
    Missing(String),
    /// `None`: nothing to play, no state.
    Off,
}

/// Resolves `choice` given the video device's `pair` (from `pairing::pair` over the present
/// inputs) and the ids of the present `inputs`.
pub fn resolve<S: AsRef<str>>(choice: &AudioChoice, pair: Option<&str>, inputs: &[S]) -> Resolved {
    match choice {
        AudioChoice::Auto => pair.map_or(Resolved::NoPair, |id| Resolved::Play(id.to_string())),
        AudioChoice::None => Resolved::Off,
        AudioChoice::Input(id) if inputs.iter().any(|input| input.as_ref() == id) => Resolved::Play(id.clone()),
        AudioChoice::Input(id) => Resolved::Missing(id.clone()),
    }
}

/// The output to open for `choice` given the ids of the present `outputs`: `None` opens the
/// system default. The flag is the fallback: a chosen output that is not present plays on the
/// default.
pub fn output_for<S: AsRef<str>>(choice: &OutputChoice, outputs: &[S]) -> (Option<String>, bool) {
    match choice {
        OutputChoice::Default => (None, false),
        OutputChoice::Device(id) if outputs.iter().any(|output| output.as_ref() == id) => (Some(id.clone()), false),
        OutputChoice::Device(_) => (None, true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CARD: &str = "{0.0.1.00000000}.{6c1b7c3a-0d4e-4a7f-9b1e-2f3c4d5e6f70}";
    const HEADSET: &str = "{0.0.0.00000000}.{11111111-2222-3333-4444-555555555555}";

    #[test]
    fn audio_choice_texts_round_trip() {
        for choice in [AudioChoice::Auto, AudioChoice::None, AudioChoice::Input(CARD.into())] {
            assert_eq!(AudioChoice::parse(&choice.to_string()), choice);
        }
        assert_eq!(AudioChoice::Auto.to_string(), "auto");
        assert_eq!(AudioChoice::None.to_string(), "none");
        assert_eq!(AudioChoice::Input(CARD.into()).to_string(), CARD);
    }

    #[test]
    fn empty_audio_text_is_auto() {
        assert_eq!(AudioChoice::parse(""), AudioChoice::Auto);
        assert_eq!(AudioChoice::default(), AudioChoice::Auto);
    }

    #[test]
    fn unknown_audio_text_is_kept_as_an_input() {
        let id = "alsa_input.usb-gone-device";
        assert_eq!(AudioChoice::parse(id), AudioChoice::Input(id.into()));
    }

    #[test]
    fn output_choice_texts_round_trip() {
        for choice in [OutputChoice::Default, OutputChoice::Device(HEADSET.into())] {
            assert_eq!(OutputChoice::parse(&choice.to_string()), choice);
        }
        assert_eq!(OutputChoice::Default.to_string(), "default");
    }

    #[test]
    fn empty_output_text_is_the_default() {
        assert_eq!(OutputChoice::parse(""), OutputChoice::Default);
        assert_eq!(OutputChoice::default(), OutputChoice::Default);
    }

    #[test]
    fn auto_plays_the_pair() {
        let inputs = [CARD];
        assert_eq!(
            resolve(&AudioChoice::Auto, Some(CARD), &inputs),
            Resolved::Play(CARD.into())
        );
    }

    #[test]
    fn auto_without_a_pair_is_no_pair() {
        let inputs = [CARD];
        assert_eq!(resolve(&AudioChoice::Auto, None, &inputs), Resolved::NoPair);
    }

    #[test]
    fn a_present_input_plays() {
        let inputs = [HEADSET, CARD];
        let choice = AudioChoice::Input(CARD.into());
        assert_eq!(resolve(&choice, None, &inputs), Resolved::Play(CARD.into()));
    }

    #[test]
    fn a_missing_input_is_missing() {
        let inputs = [HEADSET];
        let choice = AudioChoice::Input(CARD.into());
        assert_eq!(resolve(&choice, Some(HEADSET), &inputs), Resolved::Missing(CARD.into()));
        let none: [&str; 0] = [];
        assert_eq!(resolve(&choice, None, &none), Resolved::Missing(CARD.into()));
    }

    #[test]
    fn none_is_off_even_with_a_pair() {
        let inputs = [CARD];
        assert_eq!(resolve(&AudioChoice::None, Some(CARD), &inputs), Resolved::Off);
    }

    #[test]
    fn default_output_opens_the_default() {
        let outputs = [HEADSET];
        assert_eq!(output_for(&OutputChoice::Default, &outputs), (None, false));
        let none: [&str; 0] = [];
        assert_eq!(output_for(&OutputChoice::Default, &none), (None, false));
    }

    #[test]
    fn a_present_output_opens_itself() {
        let outputs = vec![CARD.to_string(), HEADSET.to_string()];
        let choice = OutputChoice::Device(HEADSET.into());
        assert_eq!(output_for(&choice, &outputs), (Some(HEADSET.into()), false));
    }

    #[test]
    fn a_missing_output_falls_back_to_the_default() {
        let outputs = [CARD];
        let choice = OutputChoice::Device(HEADSET.into());
        assert_eq!(output_for(&choice, &outputs), (None, true));
    }
}
