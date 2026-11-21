//! The audio input that belongs to a video device (research R5, data-model.md "Pairing"): by
//! the system's link first, by name only where the system gives no link.

/// An audio input as pairing sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    pub id: String,
    /// The system's name, without the app's " (2)" numbering.
    pub name: String,
    /// The physical device key; `None` when the system gives none.
    pub link: Option<String>,
}

/// The video device's side of a pairing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoKey {
    /// The physical device key, as the inputs'; `None` when the system gives none.
    pub link: Option<String>,
    pub name: String,
}

/// PipeWire's profile descriptions, which follow the device's name in a node's description.
const PROFILE_WORDS: [&str; 7] = [
    "analog stereo",
    "digital stereo",
    "estéreo analógico",
    "estéreo digital",
    "mono",
    "analog surround",
    "multichannel",
];

/// The id of the input paired with `video`, if any.
///
/// Inputs with the video's link: one is the pair; of several, the one whose name matches, else
/// the first by id. With no such input, the name decides among the inputs without a link (every
/// input when the video has none). An input with another link is never paired.
pub fn pair<'a>(video: &VideoKey, inputs: &'a [Endpoint]) -> Option<&'a str> {
    let name = clean_name(&video.name);
    let named = |input: &&Endpoint| names_match(&clean_name(&input.name), &name);
    let linked = || {
        inputs
            .iter()
            .filter(|input| video.link.is_some() && input.link == video.link)
    };

    if linked().next().is_some() {
        return first_by_id(linked().filter(named)).or_else(|| first_by_id(linked()));
    }
    let unlinked = inputs
        .iter()
        .filter(|input| input.link.is_none() || video.link.is_none());
    first_by_id(unlinked.filter(named))
}

/// A device name without the system's decorations, in lowercase.
///
/// A name that ends with `(…)` is a Windows endpoint, `Endpoint (Device)`: the device part is
/// taken. Otherwise PipeWire's profile words and what follows them are removed, then V4L2's
/// suffix after `: `.
pub fn clean_name(name: &str) -> String {
    let name = name.trim().to_lowercase();
    if let Some((before, device)) = device_part(&name) {
        // `… Digital Stereo (IEC958)` is a profile with its own parentheses, not a device part.
        if profile_start(before).is_none() {
            return device.trim().to_string();
        }
    }
    let name = profile_start(&name).map_or(name.as_str(), |at| &name[..at]);
    let name = name.split(": ").next().unwrap_or(name);
    name.trim().to_string()
}

/// Splits `Endpoint (Device)` into the text before the last parentheses and the text inside
/// them; inner parentheses (`Realtek(R) Audio`) stay in the device part.
fn device_part(name: &str) -> Option<(&str, &str)> {
    if !name.ends_with(')') {
        return None;
    }
    let mut depth = 0;
    for (at, character) in name.char_indices().rev() {
        match character {
            ')' => depth += 1,
            '(' => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            let (before, device) = (&name[..at], &name[at + 1..name.len() - 1]);
            return (!before.trim().is_empty() && !device.trim().is_empty()).then_some((before, device));
        }
    }
    None
}

/// Where the first profile word starts in a lowercase name: a whole word, after the device's
/// name.
fn profile_start(name: &str) -> Option<usize> {
    PROFILE_WORDS
        .iter()
        .flat_map(|word| name.match_indices(word))
        .filter(|(at, word)| {
            let (before, after) = (&name[..*at], &name[at + word.len()..]);
            !before.trim().is_empty()
                && before.ends_with(char::is_whitespace)
                && !after.starts_with(char::is_alphanumeric)
        })
        .map(|(at, _)| at)
        .min()
}

/// Whether two cleaned names are one device's: equal, or one a prefix of the other at a word
/// boundary.
fn names_match(a: &str, b: &str) -> bool {
    let prefix =
        |long: &str, short: &str| long.starts_with(short) && !long[short.len()..].starts_with(char::is_alphanumeric);
    !a.is_empty() && !b.is_empty() && (prefix(a, b) || prefix(b, a))
}

fn first_by_id<'a>(inputs: impl Iterator<Item = &'a Endpoint>) -> Option<&'a str> {
    inputs.map(|input| input.id.as_str()).min()
}

#[cfg(test)]
mod tests {
    use super::*;

    // measurements/pairing.md, Windows 11.
    const EZCAP_CONTAINER: &str = "b6f921b6-5905-5b63-b1a1-34d4a41a75c2";
    const C920_CONTAINER: &str = "ef06585b-1df1-51ae-b61b-8b660c165769";
    // measurements/pairing.md, CachyOS.
    const EZCAP_PATH: &str = "pci-0000:03:00.0-usb-0:3";
    const C920_PATH: &str = "pci-0000:00:14.0-usb-0:1";

    fn input(id: &str, name: &str, link: Option<&str>) -> Endpoint {
        Endpoint {
            id: id.into(),
            name: name.into(),
            link: link.map(Into::into),
        }
    }

    fn video(name: &str, link: Option<&str>) -> VideoKey {
        VideoKey {
            link: link.map(Into::into),
            name: name.into(),
        }
    }

    fn windows_inputs() -> Vec<Endpoint> {
        vec![
            input("voicemeeter", "Voicemeeter Out B1 (VB-Audio Voicemeeter VAIO)", None),
            input(
                "g533",
                "Microfone (G533 Gaming Headset)",
                Some("a717caec-0000-0000-0000-000000000000"),
            ),
            input("c920", "Microfone (HD Pro Webcam C920)", Some(C920_CONTAINER)),
            input("ezcap", "HDMI (ezcap Live GAMER ULTRA)", Some(EZCAP_CONTAINER)),
        ]
    }

    fn linux_inputs() -> Vec<Endpoint> {
        vec![
            input(
                "alsa_input.pci-0000_00_1f.3.analog-stereo",
                "Áudio interno Estéreo analógico",
                Some("pci-0000:00:1f.3"),
            ),
            input(
                "alsa_input.usb-046d_C920.analog-stereo",
                "HD Pro Webcam C920 Estéreo analógico",
                Some(C920_PATH),
            ),
            input(
                "alsa_input.usb-ezcap.analog-stereo",
                "ezcap Live GAMER ULTRA Estéreo analógico",
                Some(EZCAP_PATH),
            ),
        ]
    }

    #[test]
    fn windows_devices_pair_by_container_id() {
        let inputs = windows_inputs();
        assert_eq!(
            pair(&video("ezcap Live GAMER ULTRA", Some(EZCAP_CONTAINER)), &inputs),
            Some("ezcap")
        );
        assert_eq!(
            pair(&video("HD Pro Webcam C920", Some(C920_CONTAINER)), &inputs),
            Some("c920")
        );
    }

    #[test]
    fn linux_devices_pair_by_bus_path() {
        let inputs = linux_inputs();
        let ezcap = video("ezcap Live GAMER ULTRA: ezcap L", Some(EZCAP_PATH));
        assert_eq!(pair(&ezcap, &inputs), Some("alsa_input.usb-ezcap.analog-stereo"));
        let c920 = video("HD Pro Webcam C920", Some(C920_PATH));
        assert_eq!(pair(&c920, &inputs), Some("alsa_input.usb-046d_C920.analog-stereo"));
    }

    #[test]
    fn the_link_pairs_whatever_the_names() {
        let inputs = [input("a", "Line", Some("k1")), input("b", "Camera", Some("k2"))];
        assert_eq!(pair(&video("Camera", Some("k1")), &inputs), Some("a"));
    }

    #[test]
    fn several_inputs_with_the_link_take_the_name_match() {
        let inputs = [
            input("a", "Line In (Codec)", Some("k")),
            input("b", "HDMI (Card)", Some("k")),
            input("c", "HDMI (Card)", Some("other")),
        ];
        assert_eq!(pair(&video("Card", Some("k")), &inputs), Some("b"));
    }

    #[test]
    fn several_inputs_with_the_link_and_no_name_match_take_the_first_by_id() {
        let inputs = [
            input("b", "HDMI (Codec)", Some("k")),
            input("a", "Line In (Codec)", Some("k")),
        ];
        assert_eq!(pair(&video("Card", Some("k")), &inputs), Some("a"));
    }

    #[test]
    fn linux_names_pair_without_a_link() {
        let inputs = [
            input("mic", "Áudio interno Estéreo analógico", None),
            input("card", "ezcap Live GAMER ULTRA Estéreo analógico", None),
        ];
        assert_eq!(
            pair(&video("ezcap Live GAMER ULTRA: ezcap L", None), &inputs),
            Some("card")
        );
    }

    #[test]
    fn windows_names_pair_without_a_link() {
        let inputs = [
            input("mic", "Microfone (G533 Gaming Headset)", None),
            input("card", "Digital Audio Interface (ezcap Live Gamer Ultra)", None),
        ];
        assert_eq!(pair(&video("ezcap Live Gamer Ultra", None), &inputs), Some("card"));
    }

    #[test]
    fn a_video_without_a_link_pairs_by_name_with_a_linked_input() {
        let inputs = windows_inputs();
        assert_eq!(pair(&video("HD Pro Webcam C920", None), &inputs), Some("c920"));
    }

    #[test]
    fn a_linked_video_pairs_by_name_only_with_inputs_without_a_link() {
        let inputs = [input("virtual", "Line (Virtual Camera)", None)];
        assert_eq!(pair(&video("Virtual Camera", Some("k")), &inputs), Some("virtual"));
    }

    #[test]
    fn an_input_with_another_link_is_never_paired() {
        let inputs = [input("other", "HDMI (ezcap Live GAMER ULTRA)", Some("other"))];
        assert_eq!(
            pair(&video("ezcap Live GAMER ULTRA", Some(EZCAP_CONTAINER)), &inputs),
            None
        );
    }

    #[test]
    fn two_identical_cards_pair_each_with_its_own_input() {
        let inputs = [
            input(
                "first",
                "ezcap Live GAMER ULTRA Estéreo analógico",
                Some("pci-0000:03:00.0-usb-0:3"),
            ),
            input(
                "second",
                "ezcap Live GAMER ULTRA Estéreo analógico",
                Some("pci-0000:03:00.0-usb-0:4"),
            ),
        ];
        let name = "ezcap Live GAMER ULTRA: ezcap L";
        assert_eq!(
            pair(&video(name, Some("pci-0000:03:00.0-usb-0:3")), &inputs),
            Some("first")
        );
        assert_eq!(
            pair(&video(name, Some("pci-0000:03:00.0-usb-0:4")), &inputs),
            Some("second")
        );
        // A third card whose audio is missing takes neither.
        assert_eq!(pair(&video(name, Some("pci-0000:03:00.0-usb-0:5")), &inputs), None);
    }

    #[test]
    fn nothing_pairs_without_a_link_or_a_name() {
        let inputs = windows_inputs();
        assert_eq!(pair(&video("OBS Virtual Camera", None), &inputs), None);
        assert_eq!(pair(&video("", None), &[input("a", "", None)]), None);
        assert_eq!(pair(&video("Card", Some("k")), &[]), None);
    }

    #[test]
    fn clean_name_removes_linux_decorations() {
        assert_eq!(clean_name("ezcap Live GAMER ULTRA: ezcap L"), "ezcap live gamer ultra");
        for profile in [
            "Analog Stereo",
            "Digital Stereo (IEC958)",
            "Estéreo analógico",
            "Estéreo digital (IEC958)",
            "Mono",
            "Analog Surround 5.1",
            "Multichannel",
        ] {
            assert_eq!(
                clean_name(&format!("ezcap Live GAMER ULTRA {profile}")),
                "ezcap live gamer ultra",
                "{profile}"
            );
        }
    }

    #[test]
    fn clean_name_takes_the_windows_device_part() {
        assert_eq!(
            clean_name("Digital Audio Interface (ezcap Live Gamer Ultra)"),
            "ezcap live gamer ultra"
        );
        assert_eq!(clean_name("Microphone (Realtek(R) Audio)"), "realtek(r) audio");
        assert_eq!(clean_name("Microphone (USB Mono Device)"), "usb mono device");
    }

    #[test]
    fn clean_name_keeps_a_plain_name() {
        assert_eq!(clean_name("  HD Pro Webcam C920 "), "hd pro webcam c920");
        assert_eq!(clean_name("Monoprice Capture"), "monoprice capture");
        assert_eq!(clean_name("Mono Capture"), "mono capture");
        assert_eq!(clean_name("(odd)"), "(odd)");
    }

    #[test]
    fn names_match_at_a_word_boundary_only() {
        assert!(names_match("usb camera", "usb camera"));
        assert!(names_match("usb camera pro", "usb camera"));
        assert!(names_match("usb camera", "usb camera-2"));
        assert!(!names_match("usb camera2", "usb camera"));
        assert!(!names_match("usb", ""));
    }
}
