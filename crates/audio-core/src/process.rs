use crate::error::{AudioError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE, MAX_PATH};
use windows::Win32::System::ProcessStatus::K32GetModuleFileNameExW;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub exe_path: Option<String>,
    pub exe_name: Option<String>,
    pub parent_pid: Option<u32>,
}

pub fn process_image_path(pid: u32) -> Option<String> {
    if pid == 0 {
        return None;
    }
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let path = query_image_path(handle);
        let _ = CloseHandle(handle);
        path
    }
}

unsafe fn query_image_path(handle: HANDLE) -> Option<String> {
    let mut buf = vec![0u16; 1024];
    let mut size = buf.len() as u32;
    if QueryFullProcessImageNameW(handle, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut size)
        .is_ok()
    {
        let s = String::from_utf16_lossy(&buf[..size as usize]);
        if !s.is_empty() {
            return Some(s);
        }
    }
    let mut path = [0u16; MAX_PATH as usize];
    let len = K32GetModuleFileNameExW(Some(handle), None, &mut path);
    if len > 0 {
        Some(String::from_utf16_lossy(&path[..len as usize]))
    } else {
        None
    }
}

pub fn file_name_from_path(path: &str) -> Option<String> {
    PathBuf::from(path)
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
}

pub fn process_parent_map() -> Result<std::collections::HashMap<u32, u32>> {
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let mut map = std::collections::HashMap::new();
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
            .map_err(|e| AudioError::message(format!("CreateToolhelp32Snapshot failed: {e}")))?;
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snap, &mut entry).is_ok() {
            loop {
                map.insert(entry.th32ProcessID, entry.th32ParentProcessID);
                if Process32NextW(snap, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
    }
    Ok(map)
}

pub fn collect_process_tree(root_pid: u32) -> Result<Vec<u32>> {
    let parents = process_parent_map()?;
    let mut children: std::collections::HashMap<u32, Vec<u32>> =
        std::collections::HashMap::new();
    for (pid, parent) in &parents {
        children.entry(*parent).or_default().push(*pid);
    }

    let mut out = Vec::new();
    let mut stack = vec![root_pid];
    while let Some(pid) = stack.pop() {
        if out.contains(&pid) {
            continue;
        }
        out.push(pid);
        if let Some(kids) = children.get(&pid) {
            stack.extend(kids.iter().copied());
        }
    }
    Ok(out)
}

pub fn expand_related_pids(seed_pids: &[u32], preferred_exe: Option<&str>) -> Result<Vec<u32>> {
    let parents = process_parent_map()?;
    let mut related = std::collections::BTreeSet::new();

    for seed in seed_pids {
        related.insert(*seed);
        let mut current = *seed;
        for _ in 0..6 {
            if let Some(parent) = parents.get(&current).copied() {
                if parent == 0 || parent == current {
                    break;
                }
                if let Some(path) = process_image_path(parent) {
                    let same_family = preferred_exe
                        .map(|exe| {
                            file_name_from_path(&path)
                                .map(|n| n.eq_ignore_ascii_case(exe))
                                .unwrap_or(false)
                        })
                        .unwrap_or(false);
                    if same_family {
                        related.insert(parent);
                        current = parent;
                        continue;
                    }
                }
                break;
            }
            break;
        }

        let tree = collect_process_tree(*seed)?;
        for pid in tree {
            if let Some(pref) = preferred_exe {
                if let Some(path) = process_image_path(pid) {
                    if file_name_from_path(&path)
                        .map(|n| n.eq_ignore_ascii_case(pref))
                        .unwrap_or(false)
                    {
                        related.insert(pid);
                    }
                } else if pid == *seed {
                    related.insert(pid);
                }
            } else {
                related.insert(pid);
            }
        }
    }

    if let Some(pref) = preferred_exe {
        for (pid, _) in parents {
            if related.contains(&pid) {
                continue;
            }
            if let Some(path) = process_image_path(pid) {
                if file_name_from_path(&path)
                    .map(|n| n.eq_ignore_ascii_case(pref))
                    .unwrap_or(false)
                {
                    related.insert(pid);
                }
            }
        }
    }

    Ok(related.into_iter().collect())
}

pub fn is_critical_system_process(exe_name: &str) -> bool {
    let name = exe_name.to_ascii_lowercase();
    matches!(
        name.as_str(),
        "system"
            | "smss.exe"
            | "csrss.exe"
            | "wininit.exe"
            | "services.exe"
            | "lsass.exe"
            | "svchost.exe"
            | "audiodg.exe"
            | "dwm.exe"
            | "explorer.exe"
            | "fontdrvhost.exe"
            | "runtimebroker.exe"
            | "sihost.exe"
            | "taskhostw.exe"
            | "shellhost.exe"
            | "startmenuexperiencehost.exe"
            | "searchhost.exe"
            | "textinputhost.exe"
            | "ctfmon.exe"
            | "conhost.exe"
            | "registry"
            | "idle"
            | "secure system"
            | "memory compression"
    )
}

pub fn is_known_capture_process(exe_name: &str) -> bool {
    let name = exe_name.to_ascii_lowercase();
    const HINTS: &[&str] = &[
        "stardesk.exe",
        "parsec.exe",
        "parsecd.exe",
        "rustdesk.exe",
        "anydesk.exe",
        "teamviewer.exe",
        "tv_w32.exe",
        "tv_x64.exe",
        "moonlight.exe",
        "sunshine.exe",
        "steam.exe",
        "streaming_client.exe",
        "mstsc.exe",
        "msrdc.exe",
        "obs64.exe",
        "obs32.exe",
        "obs.exe",
    ];
    HINTS.iter().any(|h| name == *h)
}

pub fn pid_from_hwnd(hwnd: isize) -> u32 {
    let mut pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(
            windows::Win32::Foundation::HWND(hwnd as *mut _),
            Some(&mut pid),
        );
    }
    pid
}

#[allow(dead_code)]
fn _path_keep(p: &Path) -> bool {
    p.exists()
}
