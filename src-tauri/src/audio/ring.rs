use std::collections::VecDeque;
use std::sync::Mutex;

use super::CHANNELS;

/// Fila de amostras f32 intercaladas (estéreo) entre uma thread de captura e quem consome
/// (preview ou mixer). Quando enche, descarta as mais antigas: atraso nunca cresce sem limite.
pub struct Ring {
    samples: Mutex<VecDeque<f32>>,
    capacity: usize,
}

impl Ring {
    pub fn new(max_frames: usize) -> Self {
        Self { samples: Mutex::new(VecDeque::with_capacity(max_frames * CHANNELS)), capacity: max_frames * CHANNELS }
    }

    pub fn push(&self, samples: &[f32]) {
        let mut queue = self.samples.lock().unwrap();
        queue.extend(samples);
        let excess = queue.len().saturating_sub(self.capacity);
        if excess > 0 {
            queue.drain(..excess);
        }
    }

    pub fn len_frames(&self) -> usize {
        self.samples.lock().unwrap().len() / CHANNELS
    }

    /// Copia até `out.len()` amostras para `out` e devolve quantos frames leu.
    pub fn read(&self, out: &mut [f32]) -> usize {
        let mut queue = self.samples.lock().unwrap();
        let count = out.len().min(queue.len());
        let count = count - count % CHANNELS;
        for (dst, src) in out.iter_mut().zip(queue.drain(..count)) {
            *dst = src;
        }
        count / CHANNELS
    }

    /// Descarta as amostras mais antigas até sobrarem no máximo `frames`.
    pub fn trim_to(&self, frames: usize) {
        let mut queue = self.samples.lock().unwrap();
        let excess = queue.len().saturating_sub(frames * CHANNELS);
        if excess > 0 {
            queue.drain(..excess);
        }
    }
}
