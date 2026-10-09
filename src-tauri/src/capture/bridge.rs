//! Entrega os quadros capturados ao WebView sem copiar por IPC: memória compartilhada do WebView2
//! (`CreateSharedBuffer` + `PostSharedBufferToScript`) e um aviso curto por quadro.
//!
//! Protocolo (ver `src/core/native/screenCapture.ts`): `SLOTS` buffers por geração, cada um com
//! um cabeçalho de `HEADER` bytes cujo primeiro `u32` diz se o quadro está à espera do WebView
//! (1) ou livre (0). O Rust só escreve em buffer livre; o WebView copia o quadro e zera o
//! cabeçalho. Quando o quadro passa a não caber (janela maior), uma geração nova é criada.
//!
//! Tudo do WebView2 roda na thread principal; os ponteiros dos buffers podem ser escritos de
//! qualquer thread.

use std::cell::RefCell;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{mpsc, Mutex};

use tauri::WebviewWindow;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2, ICoreWebView2Environment12, ICoreWebView2SharedBuffer, ICoreWebView2_17, ICoreWebView2_2,
    COREWEBVIEW2_SHARED_BUFFER_ACCESS_READ_WRITE,
};
use windows::core::{Interface, HSTRING};

pub const SLOTS: usize = 3;
pub const HEADER: usize = 64;

thread_local! {
    /// Só existe na thread principal.
    static WEBVIEW: RefCell<Option<ICoreWebView2>> = const { RefCell::new(None) };
    static BUFFERS: RefCell<Vec<ICoreWebView2SharedBuffer>> = const { RefCell::new(Vec::new()) };
}

/// Endereço de um buffer compartilhado (válido até a próxima geração).
#[derive(Clone, Copy)]
struct Slot(usize);

pub struct Bridge {
    window: WebviewWindow,
    generation: AtomicU32,
    slots: Mutex<(usize, Vec<Slot>)>,
}

/// Quadro escrito num buffer, para o aviso ao WebView.
pub struct FrameInfo {
    pub width: u32,
    pub height: u32,
    pub timestamp_us: i64,
}

impl Bridge {
    pub fn new(window: WebviewWindow) -> tauri::Result<Self> {
        window.with_webview(|webview| unsafe {
            if let Ok(core) = webview.controller().CoreWebView2() {
                WEBVIEW.with(|w| *w.borrow_mut() = Some(core));
            }
        })?;
        Ok(Self { window, generation: AtomicU32::new(0), slots: Mutex::new((0, Vec::new())) })
    }

    /// Roda na thread principal e espera o resultado.
    fn on_main<T: Send + 'static>(&self, f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
        let (sender, receiver) = mpsc::channel();
        self.window.run_on_main_thread(move || drop(sender.send(f()))).ok()?;
        receiver.recv().ok()
    }

    /// Garante buffers com pelo menos `size` bytes de quadro (cria uma geração nova se preciso).
    fn ensure(&self, size: usize) -> Result<(), String> {
        if self.slots.lock().unwrap().0 >= size {
            return Ok(());
        }
        let generation = self.generation.fetch_add(1, Ordering::Relaxed) + 1;
        // Folga para pequenas mudanças de tamanho (janela sendo redimensionada).
        let capacity = size + size / 8;
        let created = self.on_main(move || -> Result<Vec<usize>, String> {
            WEBVIEW.with(|w| {
                let core = w.borrow().clone().ok_or("WebView not ready")?;
                unsafe {
                    let environment = core.cast::<ICoreWebView2_2>().and_then(|c| c.Environment()).map_err(|e| e.to_string())?;
                    let environment = environment.cast::<ICoreWebView2Environment12>().map_err(|_| "WebView2 runtime too old for screen capture")?;
                    let poster = core.cast::<ICoreWebView2_17>().map_err(|_| "WebView2 runtime too old for screen capture")?;
                    // Os buffers antigos deixam de valer no WebView também.
                    BUFFERS.with(|b| b.borrow_mut().drain(..).for_each(|old| drop(old.Close())));
                    let mut pointers = Vec::new();
                    for slot in 0..SLOTS {
                        let buffer = environment.CreateSharedBuffer((HEADER + capacity) as u64).map_err(|e| e.to_string())?;
                        let mut pointer = std::ptr::null_mut();
                        buffer.Buffer(&mut pointer).map_err(|e| e.to_string())?;
                        let data = HSTRING::from(format!(r#"{{"capturax":"buffer","generation":{generation},"slot":{slot}}}"#));
                        poster.PostSharedBufferToScript(&buffer, COREWEBVIEW2_SHARED_BUFFER_ACCESS_READ_WRITE, &data).map_err(|e| e.to_string())?;
                        pointers.push(pointer as usize);
                        BUFFERS.with(|b| b.borrow_mut().push(buffer));
                    }
                    Ok(pointers)
                }
            })
        });
        let pointers = created.ok_or("Main thread unavailable")??;
        *self.slots.lock().unwrap() = (capacity, pointers.into_iter().map(Slot).collect());
        Ok(())
    }

    /// Escreve um quadro BGRA (linhas de `width * 4` bytes) num buffer livre e avisa o WebView.
    /// `fill` recebe o destino. Devolve `false` se o WebView ainda não leu os anteriores (descarta).
    pub fn send(&self, info: FrameInfo, fill: impl FnOnce(&mut [u8])) -> Result<bool, String> {
        let size = info.width as usize * info.height as usize * 4;
        self.ensure(size)?;
        let generation = self.generation.load(Ordering::Relaxed);
        let (slot_index, pointer) = {
            let slots = self.slots.lock().unwrap();
            let free = slots.1.iter().enumerate().find(|(_, slot)| unsafe { std::ptr::read_volatile(slot.0 as *const u32) } == 0);
            match free {
                Some((index, slot)) => (index, slot.0),
                None => return Ok(false),
            }
        };
        unsafe {
            fill(std::slice::from_raw_parts_mut((pointer + HEADER) as *mut u8, size));
            std::ptr::write_volatile(pointer as *mut u32, 1);
        }
        let message = format!(
            r#"{{"capturax":"frame","generation":{generation},"slot":{slot_index},"width":{},"height":{},"timestamp":{}}}"#,
            info.width, info.height, info.timestamp_us
        );
        self.post(message);
        Ok(true)
    }

    /// Mensagem curta ao WebView (`chrome.webview` "message").
    pub fn post(&self, json: String) {
        let _ = self.window.run_on_main_thread(move || {
            WEBVIEW.with(|w| {
                if let Some(core) = w.borrow().as_ref() {
                    let _ = unsafe { core.PostWebMessageAsJson(&HSTRING::from(json)) };
                }
            })
        });
    }

    /// Libera os buffers (fim da captura).
    pub fn release(&self) {
        *self.slots.lock().unwrap() = (0, Vec::new());
        self.on_main(|| BUFFERS.with(|b| b.borrow_mut().drain(..).for_each(|old| drop(unsafe { old.Close() }))));
    }
}
