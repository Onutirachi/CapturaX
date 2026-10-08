//! Conversão de vídeo em Annex B (códigos de início 00 00 01) para o formato do MP4
//! (cada NAL precedido do tamanho), e montagem do `avcC`/`hvcC` a partir dos parâmetros.
//!
//! Necessário porque o encoder HEVC do Chromium entrega Annex B com VPS/SPS/PPS dentro do
//! vídeo e sem `decoderConfig.description`, mesmo pedindo o formato "hevc".

use super::fmp4::VideoCodec;

pub fn is_annexb(data: &[u8]) -> bool {
    data.starts_with(&[0, 0, 0, 1]) || data.starts_with(&[0, 0, 1])
}

/// Separa as NAL units de um pacote Annex B.
pub fn split_annexb(data: &[u8]) -> Vec<&[u8]> {
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 3 <= data.len() {
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            starts.push(i + 3);
            i += 3;
        } else {
            i += 1;
        }
    }
    let mut nals = Vec::with_capacity(starts.len());
    for (k, &start) in starts.iter().enumerate() {
        let mut end = if k + 1 < starts.len() { starts[k + 1] - 3 } else { data.len() };
        // O zero extra de um código de 4 bytes (00 00 00 01) pertence ao próximo início.
        while end > start && data[end - 1] == 0 && k + 1 < starts.len() {
            end -= 1;
        }
        if end > start {
            nals.push(&data[start..end]);
        }
    }
    nals
}

fn nal_type(codec: VideoCodec, nal: &[u8]) -> u8 {
    match codec {
        VideoCodec::Avc => nal[0] & 0x1f,
        VideoCodec::Hevc => (nal[0] >> 1) & 0x3f,
    }
}

/// Parâmetros de codec (vão no `avcC`/`hvcC`) e delimitadores de acesso (descartáveis).
fn is_parameter_or_delimiter(codec: VideoCodec, nal: &[u8]) -> bool {
    let kind = nal_type(codec, nal);
    match codec {
        VideoCodec::Avc => matches!(kind, 7 | 8 | 9),
        VideoCodec::Hevc => matches!(kind, 32..=35),
    }
}

/// Pacote Annex B → NALs com prefixo de 4 bytes, sem parâmetros nem delimitadores.
pub fn to_length_prefixed(codec: VideoCodec, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    for nal in split_annexb(data) {
        if is_parameter_or_delimiter(codec, nal) {
            continue;
        }
        out.extend_from_slice(&(nal.len() as u32).to_be_bytes());
        out.extend_from_slice(nal);
    }
    out
}

/// Monta o `avcC`/`hvcC` com os parâmetros encontrados num keyframe Annex B.
pub fn build_description(codec: VideoCodec, keyframe: &[u8]) -> Option<Vec<u8>> {
    let nals = split_annexb(keyframe);
    let find = |kind: u8| nals.iter().find(|n| !n.is_empty() && nal_type(codec, n) == kind).copied();
    match codec {
        VideoCodec::Avc => build_avcc(find(7)?, find(8)?),
        VideoCodec::Hevc => build_hvcc(find(32)?, find(33)?, find(34)?),
    }
}

fn build_avcc(sps: &[u8], pps: &[u8]) -> Option<Vec<u8>> {
    if sps.len() < 4 {
        return None;
    }
    let mut out = vec![1, sps[1], sps[2], sps[3], 0xff, 0xe1];
    out.extend_from_slice(&(sps.len() as u16).to_be_bytes());
    out.extend_from_slice(sps);
    out.push(1);
    out.extend_from_slice(&(pps.len() as u16).to_be_bytes());
    out.extend_from_slice(pps);
    Some(out)
}

/// Remove os bytes de prevenção de emulação (00 00 03 → 00 00).
fn unescape(nal: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(nal.len());
    let mut zeros = 0;
    for &byte in nal {
        if zeros >= 2 && byte == 3 {
            zeros = 0;
            continue;
        }
        zeros = if byte == 0 { zeros + 1 } else { 0 };
        out.push(byte);
    }
    out
}

fn build_hvcc(vps: &[u8], sps: &[u8], pps: &[u8]) -> Option<Vec<u8>> {
    let rbsp = unescape(sps);
    // Cabeçalho NAL (2 bytes) + 1 byte (vps id, max_sub_layers_minus1, temporal_id_nesting)
    // + profile_tier_level geral (12 bytes), que vai igual para o hvcC.
    if rbsp.len() < 15 {
        return None;
    }
    let max_sub_layers_minus1 = (rbsp[2] >> 1) & 0x07;
    let temporal_id_nested = rbsp[2] & 0x01;
    let ptl = &rbsp[3..15];
    let profile_idc = ptl[0] & 0x1f;
    // Main 10 (2) e Main Still Picture… só o Main 10 muda a profundidade de bits.
    let bit_depth_minus8: u8 = if profile_idc == 2 { 2 } else { 0 };

    let mut out = vec![1];
    out.extend_from_slice(ptl);
    out.extend_from_slice(&[0xf0, 0x00]); // min_spatial_segmentation_idc = 0
    out.push(0xfc); // parallelismType = 0
    out.push(0xfc | 1); // chroma 4:2:0
    out.push(0xf8 | bit_depth_minus8);
    out.push(0xf8 | bit_depth_minus8);
    out.extend_from_slice(&[0, 0]); // avgFrameRate
    out.push(((max_sub_layers_minus1 + 1) << 3) | (temporal_id_nested << 2) | 3); // NALs com 4 bytes de tamanho
    out.push(3);
    for (kind, nal) in [(32u8, vps), (33, sps), (34, pps)] {
        out.push(0x80 | kind);
        out.extend_from_slice(&1u16.to_be_bytes());
        out.extend_from_slice(&(nal.len() as u16).to_be_bytes());
        out.extend_from_slice(nal);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_annexb_and_builds_avcc() {
        let sps = [0x67, 0x64, 0x00, 0x1f, 0xac];
        let pps = [0x68, 0xee, 0x3c];
        let idr = [0x65, 0x88, 0x00, 0x00, 0x00, 0x10];
        let mut packet = vec![0, 0, 0, 1, 0x09, 0xf0];
        for nal in [&sps[..], &pps[..], &idr[..]] {
            packet.extend_from_slice(&[0, 0, 0, 1]);
            packet.extend_from_slice(nal);
        }
        assert!(is_annexb(&packet));
        assert_eq!(split_annexb(&packet).len(), 4);
        let converted = to_length_prefixed(VideoCodec::Avc, &packet);
        assert_eq!(converted, [&[0, 0, 0, 6][..], &idr[..]].concat());
        let avcc = build_description(VideoCodec::Avc, &packet).unwrap();
        assert_eq!(&avcc[..4], &[1, 0x64, 0x00, 0x1f]);
    }

    #[test]
    fn unescapes_emulation_prevention() {
        assert_eq!(unescape(&[0, 0, 3, 1, 0, 0, 3, 0]), vec![0, 0, 1, 0, 0, 0]);
    }
}
