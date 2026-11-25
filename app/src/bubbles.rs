//! The info bubbles' arrangement (preview extras data-model.md "Bubble arrangement"): moving a
//! bubble to a corner, pinning, resetting, and the arrays the Slint layout reads. Pure; every
//! operation keeps the invariant that each kind is in exactly one corner list.

use settings::{Arrangement, BubbleKind, Overlay};

/// The number of bubble kinds; the Slint arrays have one entry per kind.
const KINDS: usize = BubbleKind::ALL.len();

/// The arrangement as the Slint layout reads it, indexed by kind (Source 0, Mode 1, Fps 2,
/// Latency 3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlintArrays {
    /// The kind's corner: 0 top left, 1 top right, 2 bottom left, 3 bottom right.
    pub corner: [i32; KINDS],
    /// The kind's place in its corner's list, from 0.
    pub rank: [i32; KINDS],
    /// Whether the kind shows also while the controls are hidden.
    pub pinned: [bool; KINDS],
}

/// The kind's index in the Slint arrays.
fn index(kind: BubbleKind) -> usize {
    BubbleKind::ALL
        .iter()
        .position(|k| *k == kind)
        .expect("every kind is in ALL")
}

/// Removes `kind` from its list and inserts it into `corner` at `index`, clamped to the list's
/// length. Within the same corner, an index past the old place is reduced by one (the drop
/// bar's index counts the dragged bubble's old slot). A corner past the last is ignored.
pub fn move_to(arrangement: &mut Arrangement, kind: BubbleKind, corner: usize, index: usize) {
    if corner >= arrangement.corners.len() {
        log::warn!("bubble moved to corner {corner}, which does not exist; ignored");
        return;
    }
    let mut index = index;
    for (from, list) in arrangement.corners.iter_mut().enumerate() {
        if let Some(old) = list.iter().position(|k| *k == kind) {
            list.remove(old);
            if from == corner && index > old {
                index -= 1;
            }
        }
    }
    let list = &mut arrangement.corners[corner];
    list.insert(index.min(list.len()), kind);
}

/// The kind at `index` in the Slint arrays, `None` past the last (logged).
pub fn kind_at(index: i32) -> Option<BubbleKind> {
    let kind = usize::try_from(index)
        .ok()
        .and_then(|i| BubbleKind::ALL.get(i).copied());
    if kind.is_none() {
        log::warn!("bubble kind {index} does not exist; ignored");
    }
    kind
}

/// Adds `kind` to the pinned kinds or removes it; the list stays in kind order.
pub fn toggle_pin(arrangement: &mut Arrangement, kind: BubbleKind) {
    if let Some(at) = arrangement.pinned.iter().position(|k| *k == kind) {
        arrangement.pinned.remove(at);
    } else {
        arrangement.pinned.push(kind);
        arrangement.pinned.sort();
    }
}

/// Returns the bubbles to the defaults; the display choices (`latency`, `fps`, `mode`) stay.
// T027 (reset) calls it.
#[cfg_attr(not(test), expect(dead_code))]
pub fn reset(overlay: &mut Overlay) {
    overlay.bubbles = Arrangement::default();
}

/// The arrays the Slint layout reads.
pub fn slint_arrays(arrangement: &Arrangement) -> SlintArrays {
    let mut arrays = SlintArrays {
        corner: [settings::BOTTOM_RIGHT as i32; KINDS],
        rank: [0; KINDS],
        pinned: [false; KINDS],
    };
    for (corner, list) in arrangement.corners.iter().enumerate() {
        for (rank, kind) in list.iter().enumerate() {
            arrays.corner[index(*kind)] = corner as i32;
            arrays.rank[index(*kind)] = rank as i32;
        }
    }
    for kind in &arrangement.pinned {
        arrays.pinned[index(*kind)] = true;
    }
    arrays
}

#[cfg(test)]
mod tests {
    use super::*;
    use BubbleKind::{Fps, Latency, Mode, Source};
    use settings::{FpsDisplay, LatencyDisplay, ModeDisplay};

    const TOP_LEFT: usize = 0;
    const TOP_RIGHT: usize = 1;
    const BOTTOM_RIGHT: usize = settings::BOTTOM_RIGHT;

    /// Every kind is in exactly one corner list, and `pinned` is in kind order without duplicates.
    fn assert_invariant(a: &Arrangement) {
        for kind in BubbleKind::ALL {
            let count: usize = a.corners.iter().map(|l| l.iter().filter(|k| **k == kind).count()).sum();
            assert_eq!(count, 1, "{kind:?} in {a:?}");
        }
        assert!(a.pinned.windows(2).all(|w| w[0] < w[1]), "{a:?}");
    }

    fn moved(a: &Arrangement, kind: BubbleKind, corner: usize, index: usize) -> Arrangement {
        let mut a = a.clone();
        move_to(&mut a, kind, corner, index);
        assert_invariant(&a);
        a
    }

    #[test]
    fn moves_to_another_corner() {
        let a = moved(&Arrangement::default(), Fps, TOP_LEFT, 0);
        assert_eq!(a.corners[TOP_LEFT], [Fps]);
        assert_eq!(a.corners[BOTTOM_RIGHT], [Source, Mode, Latency]);

        let b = moved(&a, Latency, TOP_LEFT, 0);
        assert_eq!(b.corners[TOP_LEFT], [Latency, Fps], "first place");
        let c = moved(&a, Latency, TOP_LEFT, 1);
        assert_eq!(c.corners[TOP_LEFT], [Fps, Latency], "last place");
        let d = moved(&a, Latency, TOP_LEFT, 9);
        assert_eq!(d.corners[TOP_LEFT], [Fps, Latency], "clamped to the length");
        assert_eq!(d.corners[BOTTOM_RIGHT], [Source, Mode]);
    }

    #[test]
    fn moves_within_a_corner() {
        let a = Arrangement::default();
        // The index counts the old slot: Source to 2 lands between Mode and Fps.
        assert_eq!(
            moved(&a, Source, BOTTOM_RIGHT, 2).corners[BOTTOM_RIGHT],
            [Mode, Source, Fps, Latency]
        );
        // To the end: index 4 (past the last) is reduced to 3.
        assert_eq!(
            moved(&a, Source, BOTTOM_RIGHT, 4).corners[BOTTOM_RIGHT],
            [Mode, Fps, Latency, Source]
        );
        // Back to the front, and onto its own slot (0 or 1 keep the order).
        assert_eq!(
            moved(&a, Latency, BOTTOM_RIGHT, 0).corners[BOTTOM_RIGHT],
            [Latency, Source, Mode, Fps]
        );
        assert_eq!(
            moved(&a, Mode, BOTTOM_RIGHT, 1).corners[BOTTOM_RIGHT],
            a.corners[BOTTOM_RIGHT]
        );
        assert_eq!(
            moved(&a, Mode, BOTTOM_RIGHT, 2).corners[BOTTOM_RIGHT],
            a.corners[BOTTOM_RIGHT]
        );
        // An index before the old place is not reduced.
        assert_eq!(
            moved(&a, Fps, BOTTOM_RIGHT, 1).corners[BOTTOM_RIGHT],
            [Source, Fps, Mode, Latency]
        );
    }

    #[test]
    fn moves_into_an_empty_corner_and_ignores_a_missing_one() {
        let a = moved(&Arrangement::default(), Mode, TOP_RIGHT, 3);
        assert_eq!(a.corners[TOP_RIGHT], [Mode]);
        let b = moved(&a, Source, 4, 0);
        assert_eq!(b, a);
    }

    #[test]
    fn pins_and_unpins_in_kind_order() {
        let mut a = Arrangement::default();
        toggle_pin(&mut a, Latency);
        toggle_pin(&mut a, Source);
        toggle_pin(&mut a, Fps);
        assert_eq!(a.pinned, [Source, Fps, Latency]);
        assert_invariant(&a);
        toggle_pin(&mut a, Fps);
        assert_eq!(a.pinned, [Source, Latency]);
        // Moving keeps the pin.
        let b = moved(&a, Latency, TOP_LEFT, 0);
        assert_eq!(b.pinned, [Source, Latency]);
    }

    #[test]
    fn reset_keeps_the_display_choices() {
        let mut overlay = Overlay {
            latency: LatencyDisplay::Total,
            fps: FpsDisplay::Nominal,
            mode: ModeDisplay::Size,
            bubbles: moved(&Arrangement::default(), Fps, TOP_LEFT, 0),
        };
        toggle_pin(&mut overlay.bubbles, Fps);
        let before = overlay.clone();
        reset(&mut overlay);
        assert_eq!(overlay.bubbles, Arrangement::default());
        assert_eq!(
            (overlay.latency, overlay.fps, overlay.mode),
            (before.latency, before.fps, before.mode)
        );
    }

    #[test]
    fn kind_at_follows_the_arrays() {
        assert_eq!(kind_at(0), Some(Source));
        assert_eq!(kind_at(1), Some(Mode));
        assert_eq!(kind_at(2), Some(Fps));
        assert_eq!(kind_at(3), Some(Latency));
        assert_eq!(kind_at(4), None);
        assert_eq!(kind_at(-1), None);
    }

    #[test]
    fn slint_arrays_by_kind() {
        let arrays = slint_arrays(&Arrangement::default());
        assert_eq!(arrays.corner, [3, 3, 3, 3]);
        assert_eq!(arrays.rank, [0, 1, 2, 3]);
        assert_eq!(arrays.pinned, [false; 4]);

        let mut a = moved(&Arrangement::default(), Latency, TOP_LEFT, 0);
        move_to(&mut a, Source, TOP_LEFT, 1);
        toggle_pin(&mut a, Mode);
        let arrays = slint_arrays(&a);
        // Source, Mode, Fps, Latency.
        assert_eq!(arrays.corner, [0, 3, 3, 0]);
        assert_eq!(arrays.rank, [1, 0, 1, 0]);
        assert_eq!(arrays.pinned, [false, true, false, false]);
    }
}
