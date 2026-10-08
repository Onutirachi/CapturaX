//! Descobre quais apps estão tocando áudio e decide quais árvores de processo capturar.
//!
//! O loopback por processo do Windows só sabe capturar uma árvore inteira (o processo e todos os
//! filhos). Para montar "o áudio do sistema menos o CapturaX e os apps excluídos", capturamos cada
//! app separadamente e somamos no mixer, tomando cuidado para nenhuma árvore capturada conter um
//! processo excluído (ex.: um launcher que abriu o Discord) e nenhum som entrar duas vezes.

use serde::Serialize;
use std::collections::{HashMap, HashSet};
use windows::core::{Interface, Result};
use windows::Win32::Foundation::{CloseHandle, S_OK};
use windows::Win32::Media::Audio::{IAudioSessionControl2, IAudioSessionManager2, AudioSessionStateActive, AudioSessionStateExpired};
use windows::Win32::System::Com::CLSCTX_ALL;
use windows::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};
use windows::Win32::System::Threading::GetCurrentProcessId;

use super::wasapi::render_devices;

/// Sessão de áudio de um processo em algum dispositivo de saída.
#[derive(Clone, Copy, Debug)]
pub struct AudioSession {
    pub pid: u32,
    /// Tocando agora (e não só com a sessão aberta).
    pub active: bool,
}

pub fn list_sessions() -> Result<Vec<AudioSession>> {
    let mut by_pid: HashMap<u32, bool> = HashMap::new();
    for device in render_devices()? {
        let Ok(manager) = (unsafe { device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) }) else { continue };
        let Ok(sessions) = (unsafe { manager.GetSessionEnumerator() }) else { continue };
        for i in 0..unsafe { sessions.GetCount()? } {
            let Ok(control) = (unsafe { sessions.GetSession(i) }) else { continue };
            let Ok(control) = control.cast::<IAudioSessionControl2>() else { continue };
            unsafe {
                // "Sons do sistema" não pertence a um app capturável.
                if control.IsSystemSoundsSession() == S_OK {
                    continue;
                }
                let state = control.GetState()?;
                if state == AudioSessionStateExpired {
                    continue;
                }
                let Ok(pid) = control.GetProcessId() else { continue };
                if pid == 0 {
                    continue;
                }
                *by_pid.entry(pid).or_default() |= state == AudioSessionStateActive;
            }
        }
    }
    Ok(by_pid.into_iter().map(|(pid, active)| AudioSession { pid, active }).collect())
}

/// Foto da árvore de processos do Windows.
pub struct ProcessTable {
    parent: HashMap<u32, u32>,
    exe: HashMap<u32, String>,
    children: HashMap<u32, Vec<u32>>,
}

impl ProcessTable {
    pub fn snapshot() -> Result<Self> {
        let mut parent = HashMap::new();
        let mut exe = HashMap::new();
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)?;
            let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
            let mut ok = Process32FirstW(snapshot, &mut entry).is_ok();
            while ok {
                let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..len]).to_lowercase();
                parent.insert(entry.th32ProcessID, entry.th32ParentProcessID);
                exe.insert(entry.th32ProcessID, name);
                ok = Process32NextW(snapshot, &mut entry).is_ok();
            }
            let _ = CloseHandle(snapshot);
        }
        let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
        for (&pid, &ppid) in &parent {
            if pid != ppid {
                children.entry(ppid).or_default().push(pid);
            }
        }
        Ok(Self { parent, exe, children })
    }

    pub fn exe(&self, pid: u32) -> Option<&str> {
        self.exe.get(&pid).map(String::as_str)
    }

    /// Pais, avós… até a raiz (com proteção contra ciclos de PID reaproveitado).
    fn ancestors(&self, pid: u32) -> Vec<u32> {
        let mut chain = Vec::new();
        let mut seen = HashSet::from([pid]);
        let mut current = pid;
        while let Some(&ppid) = self.parent.get(&current) {
            if ppid == 0 || !seen.insert(ppid) {
                break;
            }
            chain.push(ppid);
            current = ppid;
        }
        chain
    }

    /// O processo e todos os descendentes.
    pub fn tree(&self, pid: u32) -> HashSet<u32> {
        let mut found = HashSet::from([pid]);
        let mut stack = vec![pid];
        while let Some(current) = stack.pop() {
            for &child in self.children.get(&current).into_iter().flatten() {
                if found.insert(child) {
                    stack.push(child);
                }
            }
        }
        found
    }
}

/// PIDs cujas árvores devem ser capturadas para formar o "áudio do sistema".
pub fn plan_captures(sessions: &[AudioSession], table: &ProcessTable, excluded: &[String]) -> HashSet<u32> {
    let own_tree = table.tree(unsafe { GetCurrentProcessId() });
    let is_excluded = |pid: u32| table.exe(pid).is_some_and(|exe| excluded.iter().any(|e| e == exe));
    let forbidden: HashSet<u32> = table.exe.keys().copied().filter(|&pid| is_excluded(pid) || own_tree.contains(&pid)).collect();

    // Ancestrais primeiro: se o pai for capturado, a árvore dele já cobre os filhos.
    let mut candidates: Vec<(usize, u32)> = sessions.iter().map(|s| (table.ancestors(s.pid).len(), s.pid)).collect();
    candidates.sort();

    let mut chosen = HashSet::new();
    for (_, pid) in candidates {
        if forbidden.contains(&pid) {
            continue;
        }
        if table.ancestors(pid).iter().any(|a| chosen.contains(a)) {
            continue;
        }
        // Árvore com um processo proibido (ex.: Explorer, que é pai de quase tudo) não pode ser
        // capturada inteira; os filhos com sessão própria entram individualmente.
        if table.tree(pid).iter().any(|p| forbidden.contains(p)) {
            continue;
        }
        chosen.insert(pid);
    }
    chosen
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AppEntry {
    /// Nome do executável em minúsculas (é o que fica salvo).
    pub exe: String,
    pub name: String,
    /// Com sessão de áudio aberta agora.
    pub running: bool,
    /// Tocando som agora.
    pub playing: bool,
}

pub fn display_name(exe: &str) -> String {
    let stem = exe.strip_suffix(".exe").unwrap_or(exe);
    let mut chars = stem.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Apps com sessão de áudio agora (sem o próprio CapturaX), para a tela de exclusão.
pub fn running_apps() -> Result<Vec<AppEntry>> {
    let sessions = list_sessions()?;
    let table = ProcessTable::snapshot()?;
    let own_tree = table.tree(unsafe { GetCurrentProcessId() });
    let mut by_exe: HashMap<String, AppEntry> = HashMap::new();
    for session in sessions {
        if own_tree.contains(&session.pid) {
            continue;
        }
        let Some(exe) = table.exe(session.pid) else { continue };
        let entry = by_exe.entry(exe.to_string()).or_insert_with(|| AppEntry {
            exe: exe.to_string(),
            name: display_name(exe),
            running: true,
            playing: false,
        });
        entry.playing |= session.active;
    }
    Ok(by_exe.into_values().collect())
}
