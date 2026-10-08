//! Gravação em arquivo.
//!
//! Durante a gravação o arquivo é um MP4 fragmentado: se o app ou o PC cair no meio, tudo até o
//! último fragmento continua tocável. Ao parar, acrescenta um `moov` completo no fim e troca o
//! tipo dos `moov`/`moof` antigos para `free`, virando um MP4 comum (seek rápido em qualquer player).

use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::PathBuf;

use super::fmp4::{progressive_moov, AudioTrack, Fragment, IndexedSample, VideoTrack};
use super::FragmentSink;

pub struct FileSink {
    path: PathBuf,
    file: Option<BufWriter<File>>,
    position: u64,
    moov_offset: Option<u64>,
    moof_offsets: Vec<u64>,
    video_index: Vec<IndexedSample>,
    audio_index: Vec<IndexedSample>,
}

impl FileSink {
    pub fn create(path: PathBuf) -> std::io::Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let file = BufWriter::with_capacity(4 << 20, File::create(&path)?);
        Ok(Self { path, file: Some(file), position: 0, moov_offset: None, moof_offsets: Vec::new(), video_index: Vec::new(), audio_index: Vec::new() })
    }

    fn file(&mut self) -> std::io::Result<&mut BufWriter<File>> {
        self.file.as_mut().ok_or_else(|| std::io::Error::other("recording already finished"))
    }

    fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.file()?.write_all(bytes)?;
        self.position += bytes.len() as u64;
        Ok(())
    }
}

impl FragmentSink for FileSink {
    fn init(&mut self, bytes: &[u8], moov_offset: usize, _codecs: &str) -> std::io::Result<()> {
        self.moov_offset = Some(self.position + moov_offset as u64);
        self.write(bytes)
    }

    fn fragment(&mut self, fragment: &Fragment) -> std::io::Result<()> {
        self.moof_offsets.push(self.position);
        let mut offset = self.position + fragment.payload_offset as u64;
        for (meta, index) in [(&fragment.video, &mut self.video_index), (&fragment.audio, &mut self.audio_index)] {
            for m in meta {
                index.push(IndexedSample { offset, size: m.size, duration: m.duration, sync: m.sync });
                offset += m.size as u64;
            }
        }
        self.write(&fragment.bytes)?;
        // Fragmento a fragmento no disco: é isso que protege contra queda.
        self.file()?.flush()
    }

    fn finish(&mut self, video: &VideoTrack, audio: Option<&AudioTrack>) -> std::io::Result<Option<PathBuf>> {
        if self.video_index.is_empty() {
            // Nada gravado (ex.: parou antes do primeiro keyframe): não deixa arquivo vazio.
            self.file = None;
            let _ = std::fs::remove_file(&self.path);
            return Ok(None);
        }
        let moov = progressive_moov(video, audio, &self.video_index, &self.audio_index);
        self.write(&moov)?;
        let mut writer = self.file.take().unwrap();
        writer.flush()?;

        let file = writer.get_mut();
        for offset in self.moov_offset.iter().chain(&self.moof_offsets) {
            file.seek(SeekFrom::Start(offset + 4))?;
            file.write_all(b"free")?;
        }
        file.flush()?;
        file.sync_all()?;
        Ok(Some(self.path.clone()))
    }
}
