//! Modos de vídeo que cada dispositivo de captura realmente oferece (resolução × FPS × formato),
//! lidos pelo Media Foundation. O navegador só informa faixas mínimas/máximas, então sem isto os
//! dropdowns mostrariam resoluções que a placa não tem (e que viram redimensionamento/distorção).

use serde::Serialize;
use windows::core::{Result, BOOL, GUID, PWSTR};
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::CoTaskMemFree;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VideoMode {
    pub width: u32,
    pub height: u32,
    /// FPS exato (ex.: 59.94).
    pub fps: f64,
    /// "NV12", "YUY2", "MJPG"…
    pub format: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct VideoDeviceModes {
    pub name: String,
    /// Link simbólico do Windows (contém `vid_xxxx&pid_xxxx`, usado para casar com o WebView).
    pub symbolic_link: String,
    pub modes: Vec<VideoMode>,
}

fn format_name(subtype: GUID) -> String {
    match subtype {
        s if s == MFVideoFormat_NV12 => "NV12".into(),
        s if s == MFVideoFormat_YUY2 => "YUY2".into(),
        s if s == MFVideoFormat_MJPG => "MJPG".into(),
        // FourCC nos 4 primeiros bytes do GUID de subtipo.
        s => String::from_utf8_lossy(&s.data1.to_le_bytes()).trim().to_string(),
    }
}

fn string_attribute(activate: &IMFActivate, key: &GUID) -> String {
    unsafe {
        let mut value = PWSTR::null();
        let mut length = 0;
        if activate.GetAllocatedString(key, &mut value, &mut length).is_err() {
            return String::new();
        }
        let text = value.to_string().unwrap_or_default();
        CoTaskMemFree(Some(value.0 as _));
        text
    }
}

fn modes_of(activate: &IMFActivate) -> Result<Vec<VideoMode>> {
    let mut modes = Vec::new();
    unsafe {
        let source: IMFMediaSource = activate.ActivateObject()?;
        let result = (|| -> Result<()> {
            let descriptor = source.CreatePresentationDescriptor()?;
            for i in 0..descriptor.GetStreamDescriptorCount()? {
                let mut selected = BOOL(0);
                let mut stream = None;
                descriptor.GetStreamDescriptorByIndex(i, &mut selected, &mut stream)?;
                let Some(stream) = stream else { continue };
                let handler = stream.GetMediaTypeHandler()?;
                for j in 0..handler.GetMediaTypeCount()? {
                    let media_type = handler.GetMediaTypeByIndex(j)?;
                    let (Ok(size), Ok(rate)) = (media_type.GetUINT64(&MF_MT_FRAME_SIZE), media_type.GetUINT64(&MF_MT_FRAME_RATE)) else { continue };
                    let (numerator, denominator) = ((rate >> 32) as u32, rate as u32);
                    if denominator == 0 {
                        continue;
                    }
                    let mode = VideoMode {
                        width: (size >> 32) as u32,
                        height: size as u32,
                        fps: (numerator as f64 / denominator as f64 * 100.0).round() / 100.0,
                        format: media_type.GetGUID(&MF_MT_SUBTYPE).map(format_name).unwrap_or_default(),
                    };
                    if !modes.contains(&mode) {
                        modes.push(mode);
                    }
                }
            }
            Ok(())
        })();
        // Libera o dispositivo na hora (só lemos a lista, nada foi aberto para captura).
        let _ = source.Shutdown();
        let _ = activate.ShutdownObject();
        result?;
    }
    Ok(modes)
}

pub fn list() -> Result<Vec<VideoDeviceModes>> {
    crate::media::aac::startup();
    let mut devices = Vec::new();
    unsafe {
        let mut attributes = None;
        MFCreateAttributes(&mut attributes, 1)?;
        let attributes = attributes.unwrap();
        attributes.SetGUID(&MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE, &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID)?;
        let mut list = std::ptr::null_mut();
        let mut count = 0u32;
        MFEnumDeviceSources(&attributes, &mut list, &mut count)?;
        let activates = std::slice::from_raw_parts_mut(list, count as usize);
        for activate in activates.iter_mut() {
            // Pega posse de cada item (o array é liberado com CoTaskMemFree logo abaixo).
            let Some(activate) = activate.take() else { continue };
            let name = string_attribute(&activate, &MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME);
            let symbolic_link = string_attribute(&activate, &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK);
            match modes_of(&activate) {
                Ok(modes) => devices.push(VideoDeviceModes { name, symbolic_link, modes }),
                Err(error) => eprintln!("Could not read modes of {name}: {error}"),
            }
        }
        CoTaskMemFree(Some(list as _));
    }
    Ok(devices)
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore]
    fn list_video_modes() {
        let _com = crate::audio::ComGuard::new();
        for device in super::list().unwrap() {
            println!("{} [{}]", device.name, device.symbolic_link);
            for mode in &device.modes {
                println!("   {}x{} @ {} {}", mode.width, mode.height, mode.fps, mode.format);
            }
        }
    }
}
