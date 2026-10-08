//! Relógio comum a áudio e vídeo: QPC (QueryPerformanceCounter) em microssegundos.
//!
//! O mixer carimba o áudio com ele e o front calibra o `performance.now()` contra `now_us()`,
//! então áudio e vídeo ficam no mesmo domínio de tempo.

use std::sync::OnceLock;
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

static FREQUENCY: OnceLock<i64> = OnceLock::new();

pub fn now_us() -> i64 {
    let frequency = *FREQUENCY.get_or_init(|| {
        let mut f = 0;
        unsafe { QueryPerformanceFrequency(&mut f).expect("QueryPerformanceFrequency") };
        f
    });
    let mut counter = 0;
    unsafe { QueryPerformanceCounter(&mut counter).expect("QueryPerformanceCounter") };
    (counter as i128 * 1_000_000 / frequency as i128) as i64
}
