use tauri::Manager;

/// Libera câmera e microfone no WebView2 sem mostrar o prompt de permissão.
#[cfg(windows)]
fn auto_grant_media_permissions(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_PERMISSION_KIND, COREWEBVIEW2_PERMISSION_KIND_CAMERA, COREWEBVIEW2_PERMISSION_KIND_MICROPHONE,
        COREWEBVIEW2_PERMISSION_STATE_ALLOW,
    };
    use webview2_com::PermissionRequestedEventHandler;

    window.with_webview(|webview| unsafe {
        let Ok(core) = webview.controller().CoreWebView2() else { return };
        let handler = PermissionRequestedEventHandler::create(Box::new(|_, args| {
            let Some(args) = args else { return Ok(()) };
            let mut kind = COREWEBVIEW2_PERMISSION_KIND::default();
            args.PermissionKind(&mut kind)?;
            if kind == COREWEBVIEW2_PERMISSION_KIND_CAMERA || kind == COREWEBVIEW2_PERMISSION_KIND_MICROPHONE {
                args.SetState(COREWEBVIEW2_PERMISSION_STATE_ALLOW)?;
            }
            Ok(())
        }));
        let mut token = Default::default();
        if let Err(error) = core.add_PermissionRequested(&handler, &mut token) {
            eprintln!("Failed to register PermissionRequested handler: {error}");
        }
    })
}

/// Liga/desliga as animações do Windows (maximizar/restaurar) da janela.
/// Usado na troca de fullscreen, que precisa desmaximizar antes (ver FullscreenContext).
/// Roda na fila da thread principal para ficar na ordem certa com os outros comandos de janela.
#[tauri::command]
fn set_window_animations(window: tauri::WebviewWindow, enabled: bool) -> tauri::Result<()> {
    #[cfg(windows)]
    {
        use windows::core::BOOL;
        use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_TRANSITIONS_FORCEDISABLED};

        let hwnd = window.hwnd()?;
        let hwnd_raw = hwnd.0 as isize;
        window.run_on_main_thread(move || unsafe {
            let disabled = BOOL::from(!enabled);
            let _ = DwmSetWindowAttribute(
                windows::Win32::Foundation::HWND(hwnd_raw as _),
                DWMWA_TRANSITIONS_FORCEDISABLED,
                &disabled as *const BOOL as *const _,
                std::mem::size_of::<BOOL>() as u32,
            );
        })?;
    }
    #[cfg(not(windows))]
    let _ = (window, enabled);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            #[cfg(windows)]
            if let Some(window) = app.get_webview_window("main") {
                auto_grant_media_permissions(&window)?;
            }
            #[cfg(not(windows))]
            let _ = app;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![set_window_animations])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
