//! Regra do Firewall do Windows liberando a entrada TCP só para o executável do CapturaX.
//!
//! O instalador já cria a regra (ver `windows/hooks.nsh`); aqui fica a verificação e o botão
//! "corrigir firewall" para quem roda fora do instalador ou negou o aviso do Windows.

use std::os::windows::process::CommandExt;
use std::process::Command;
use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;

pub const RULE_NAME: &str = "CapturaX Stream";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn rule_exists() -> bool {
    Command::new("netsh")
        .args(["advfirewall", "firewall", "show", "rule", &format!("name={RULE_NAME}")])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Recria a regra (pede permissão de administrador ao Windows). Também apaga regras antigas do
/// mesmo executável, inclusive bloqueios criados quando alguém clica "Cancelar" no aviso do Windows.
pub fn fix() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe = exe.display().to_string();
    if exe.contains('"') {
        return Err(crate::error::code("firewall_failed"));
    }
    let script = format!(
        "/c netsh advfirewall firewall delete rule name=all program=\"{exe}\" & \
         netsh advfirewall firewall delete rule name=\"{RULE_NAME}\" & \
         netsh advfirewall firewall add rule name=\"{RULE_NAME}\" dir=in action=allow program=\"{exe}\" protocol=TCP enable=yes profile=any"
    );
    let parameters = HSTRING::from(script);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: w!("runas"),
        lpFile: w!("cmd.exe"),
        lpParameters: PCWSTR(parameters.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    unsafe {
        ShellExecuteExW(&mut info).map_err(|_| crate::error::code("firewall_denied"))?;
        if !info.hProcess.is_invalid() {
            WaitForSingleObject(info.hProcess, 30_000);
            let mut code = 0;
            let _ = GetExitCodeProcess(info.hProcess, &mut code);
            let _ = CloseHandle(info.hProcess);
        }
    }
    if rule_exists() {
        Ok(())
    } else {
        Err(crate::error::code("firewall_failed"))
    }
}
