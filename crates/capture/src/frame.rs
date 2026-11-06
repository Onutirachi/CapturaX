//! Frames handed to the video thread (data-model.md "Frame"): the device's own planes, rows
//! packed, with the matrix and range the shader converts with, and when the frame was captured
//! and when it became ready (research R4, R11).

use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

/// The pixel formats the video thread uploads; anything else is converted to NV12 in the
/// pipeline (research R4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// Luma plane, then interleaved chroma at half size.
    Nv12,
    /// Packed 4:2:2, Y0 U Y1 V.
    Yuy2,
    /// Packed 4:2:2, U Y0 V Y1.
    Uyvy,
    /// Luma, then U and V planes at half size.
    I420,
    Bgrx,
    Rgba,
}

impl PixelFormat {
    /// How many planes a frame of this format has.
    pub fn plane_count(self) -> usize {
        match self {
            Self::Nv12 => 2,
            Self::I420 => 3,
            Self::Yuy2 | Self::Uyvy | Self::Bgrx | Self::Rgba => 1,
        }
    }

    /// Row length in bytes and number of rows of each plane, rows packed.
    pub fn plane_sizes(self, width: u32, height: u32) -> Vec<(usize, usize)> {
        let (w, h) = (width as usize, height as usize);
        let (half_w, half_h) = (w.div_ceil(2), h.div_ceil(2));
        match self {
            Self::Nv12 => vec![(w, h), (half_w * 2, half_h)],
            Self::I420 => vec![(w, h), (half_w, half_h), (half_w, half_h)],
            Self::Yuy2 | Self::Uyvy => vec![(half_w * 4, h)],
            Self::Bgrx | Self::Rgba => vec![(w * 4, h)],
        }
    }

    /// The name GStreamer and the stats use (`NV12`, `YUY2`…).
    pub fn name(self) -> &'static str {
        match self {
            Self::Nv12 => "NV12",
            Self::Yuy2 => "YUY2",
            Self::Uyvy => "UYVY",
            Self::I420 => "I420",
            Self::Bgrx => "BGRx",
            Self::Rgba => "RGBA",
        }
    }
}

/// The YUV to RGB matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Matrix {
    Bt601,
    Bt709,
}

/// The color range of the values in a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Range {
    /// 0–255.
    Full,
    /// 16–235 luma, 16–240 chroma.
    Limited,
}

/// The matrix of a frame: the declared one, else BT.709 at 720p and above and BT.601 below.
pub fn matrix_for(colorimetry: Option<Matrix>, height: u32) -> Matrix {
    colorimetry.unwrap_or(if height >= 720 { Matrix::Bt709 } else { Matrix::Bt601 })
}

/// When a frame was captured (research R11): `now − (clock_time − (pts + base_time))`, all in
/// nanoseconds of the pipeline clock. A frame stamped after the clock's present time (negative
/// age) counts as captured now.
pub fn captured_at(now: Instant, clock_time: u64, pts: u64, base_time: u64) -> Instant {
    let age = clock_time.saturating_sub(pts.saturating_add(base_time));
    now.checked_sub(Duration::from_nanos(age)).unwrap_or(now)
}

/// One plane's bytes, rows packed (`stride` is the row length in bytes).
#[derive(Debug, Default)]
pub struct Plane {
    pub bytes: PooledBytes,
    pub stride: usize,
}

/// A frame from a session (data-model.md "Frame").
#[derive(Debug)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    planes: [Plane; 3],
    pub matrix: Matrix,
    pub range: Range,
    /// When the device captured it (research R11).
    pub captured: Instant,
    /// When the appsink handed it over.
    pub ready: Instant,
    /// The number of the session that produced it; frames of a closed session are dropped.
    pub session: u64,
}

impl Frame {
    /// A frame with the given planes (at most three; extra ones are ignored).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        width: u32,
        height: u32,
        format: PixelFormat,
        planes: Vec<Plane>,
        matrix: Matrix,
        range: Range,
        captured: Instant,
        ready: Instant,
        session: u64,
    ) -> Self {
        let mut slots: [Plane; 3] = Default::default();
        for (slot, plane) in slots.iter_mut().zip(planes) {
            *slot = plane;
        }
        Self {
            width,
            height,
            format,
            planes: slots,
            matrix,
            range,
            captured,
            ready,
            session,
        }
    }

    /// The planes of the frame's format, in order.
    pub fn planes(&self) -> &[Plane] {
        &self.planes[..self.format.plane_count()]
    }
}

type Shelf = Mutex<Vec<Vec<u8>>>;

/// Byte buffers reused from frame to frame: once warm, no allocation per frame. A buffer goes
/// back to its pool when the frame holding it is dropped.
#[derive(Debug, Clone, Default)]
pub struct PlanePool {
    shelf: Arc<Shelf>,
}

/// Buffers kept on the shelf at most: two frames of three planes in flight, plus spare.
const SHELF_LIMIT: usize = 8;

impl PlanePool {
    pub fn new() -> Self {
        Self::default()
    }

    /// A buffer of exactly `len` bytes (contents unspecified), from the shelf when one is there.
    pub fn take(&self, len: usize) -> PooledBytes {
        let reused = self.shelf.lock().ok().and_then(|mut shelf| {
            let best = shelf
                .iter()
                .position(|b| b.capacity() >= len)
                .or_else(|| (!shelf.is_empty()).then_some(0))?;
            Some(shelf.swap_remove(best))
        });
        let mut bytes = reused.unwrap_or_default();
        bytes.resize(len, 0);
        PooledBytes {
            bytes,
            home: Arc::downgrade(&self.shelf),
        }
    }

    /// How many buffers wait on the shelf (for tests and logs).
    pub fn idle(&self) -> usize {
        self.shelf.lock().map(|s| s.len()).unwrap_or(0)
    }
}

/// Bytes that return to their pool when dropped.
#[derive(Debug, Default)]
pub struct PooledBytes {
    bytes: Vec<u8>,
    home: Weak<Shelf>,
}

impl PooledBytes {
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.bytes
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

impl From<Vec<u8>> for PooledBytes {
    /// Bytes outside any pool (tests, one-off frames).
    fn from(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            home: Weak::new(),
        }
    }
}

impl Drop for PooledBytes {
    fn drop(&mut self) {
        if self.bytes.capacity() == 0 {
            return;
        }
        if let Some(shelf) = self.home.upgrade()
            && let Ok(mut shelf) = shelf.lock()
            && shelf.len() < SHELF_LIMIT
        {
            shelf.push(std::mem::take(&mut self.bytes));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_from_colorimetry_or_height() {
        assert_eq!(matrix_for(Some(Matrix::Bt601), 1080), Matrix::Bt601);
        assert_eq!(matrix_for(Some(Matrix::Bt709), 480), Matrix::Bt709);
        assert_eq!(matrix_for(None, 720), Matrix::Bt709);
        assert_eq!(matrix_for(None, 2160), Matrix::Bt709);
        assert_eq!(matrix_for(None, 719), Matrix::Bt601);
        assert_eq!(matrix_for(None, 480), Matrix::Bt601);
    }

    #[test]
    fn captured_time() {
        let now = Instant::now() + Duration::from_secs(10);
        // Clock at 1 s 8 ms, frame stamped at 1 s (running time 900 ms + base 100 ms): 8 ms old.
        let at = captured_at(now, 1_008_000_000, 900_000_000, 100_000_000);
        assert_eq!(now - at, Duration::from_millis(8));
        // Stamped after the clock (negative age): now.
        assert_eq!(captured_at(now, 1_000_000_000, 1_000_000_000, 5), now);
        // Zero age.
        assert_eq!(captured_at(now, 1_000, 900, 100), now);
    }

    #[test]
    fn plane_sizes() {
        assert_eq!(
            PixelFormat::Nv12.plane_sizes(1920, 1080),
            vec![(1920, 1080), (1920, 540)]
        );
        assert_eq!(PixelFormat::Yuy2.plane_sizes(1920, 1080), vec![(3840, 1080)]);
        assert_eq!(PixelFormat::I420.plane_sizes(5, 3), vec![(5, 3), (3, 2), (3, 2)]);
        assert_eq!(PixelFormat::Bgrx.plane_sizes(2, 2), vec![(8, 2)]);
    }

    #[test]
    fn pool_reuses_a_returned_buffer() {
        let pool = PlanePool::new();
        let first = pool.take(1024);
        let address = first.as_slice().as_ptr();
        assert_eq!(pool.idle(), 0);
        drop(first);
        assert_eq!(pool.idle(), 1);
        let second = pool.take(1000);
        assert_eq!(second.len(), 1000);
        assert_eq!(second.as_slice().as_ptr(), address);
        assert_eq!(pool.idle(), 0);
    }

    #[test]
    fn frame_planes_follow_the_format() {
        let now = Instant::now();
        let planes = vec![
            Plane {
                bytes: vec![0; 4].into(),
                stride: 2,
            },
            Plane {
                bytes: vec![0; 2].into(),
                stride: 2,
            },
        ];
        let frame = Frame::new(
            2,
            2,
            PixelFormat::Nv12,
            planes,
            Matrix::Bt709,
            Range::Limited,
            now,
            now,
            1,
        );
        assert_eq!(frame.planes().len(), 2);
        assert_eq!(frame.planes()[1].bytes.len(), 2);
    }
}
