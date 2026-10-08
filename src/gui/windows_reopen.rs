use windows::{
    Win32::{
        Foundation::{CloseHandle, HWND, LPARAM},
        System::{
            ProcessStatus::{K32EnumProcessModules, K32GetModuleFileNameExW},
            Threading::{
                OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION,
                PROCESS_VM_READ, QueryFullProcessImageNameW,
            },
        },
        UI::WindowsAndMessaging::{
            EnumWindows, FLASHW_TIMERNOFG, FLASHW_TRAY, FLASHWINFO, FlashWindowEx, GetClassNameW, GetWindowTextW,
            GetWindowThreadProcessId, IsIconic, IsWindowVisible, SW_RESTORE, SW_SHOW, SetForegroundWindow, ShowWindow,
        },
    },
    core::BOOL,
};

// Older tray builds cannot receive the instance socket request after an update in place.
#[derive(Debug)]
pub enum Reopen {
    Focused,
    AttentionRequested,
}

pub fn show_existing() -> Option<Reopen> {
    let Ok(executable) = std::env::current_exe() else {
        return None;
    };
    let mut search = WindowSearch {
        executable: executable.to_string_lossy().into_owned(),
        windows: Vec::new(),
    };
    let _ = unsafe { EnumWindows(Some(find_window), LPARAM(&mut search as *mut WindowSearch as isize)) };
    let [window] = search.windows.as_slice() else {
        return None;
    };
    unsafe {
        let mode = if IsIconic(*window).as_bool() {
            SW_RESTORE
        } else {
            SW_SHOW
        };
        let _ = ShowWindow(*window, mode);
        if !IsWindowVisible(*window).as_bool() {
            return None;
        }
        if SetForegroundWindow(*window).as_bool() {
            return Some(Reopen::Focused);
        }
        let _ = FlashWindowEx(&FLASHWINFO {
            cbSize: std::mem::size_of::<FLASHWINFO>() as u32,
            hwnd: *window,
            dwFlags: FLASHW_TRAY | FLASHW_TIMERNOFG,
            uCount: 3,
            dwTimeout: 0,
        });
    }
    Some(Reopen::AttentionRequested)
}

struct WindowSearch {
    executable: String,
    windows: Vec<HWND>,
}

unsafe extern "system" fn find_window(window: HWND, context: LPARAM) -> BOOL {
    let search = unsafe { &mut *(context.0 as *mut WindowSearch) };
    let mut class = [0; 64];
    let length = unsafe { GetClassNameW(window, &mut class) };
    if !class[..length as usize]
        .iter()
        .copied()
        .eq("Window Class".encode_utf16())
    {
        return true.into();
    }
    let mut title = [0; 512];
    let length = unsafe { GetWindowTextW(window, &mut title) };
    if !is_main_window_title(&String::from_utf16_lossy(&title[..length as usize])) {
        return true.into();
    }
    let mut process = 0;
    unsafe { GetWindowThreadProcessId(window, Some(&mut process)) };
    let mut path = vec![0; 32768];
    let length;
    if let Ok(handle) = unsafe { OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, process) } {
        // Query the loaded module: the disk image path changes when an update moves the old executable.
        let mut module = windows::Win32::Foundation::HMODULE::default();
        let mut needed = 0;
        let modules =
            unsafe { K32EnumProcessModules(handle, &mut module, std::mem::size_of_val(&module) as u32, &mut needed) };
        length = if modules.as_bool() {
            unsafe { K32GetModuleFileNameExW(Some(handle), Some(module), &mut path) }
        } else {
            0
        };
        let _ = unsafe { CloseHandle(handle) };
    } else if let Ok(handle) = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process) } {
        let mut size = path.len() as u32;
        length = if unsafe {
            QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_WIN32,
                windows::core::PWSTR(path.as_mut_ptr()),
                &mut size,
            )
        }
        .is_ok()
        {
            size
        } else {
            0
        };
        let _ = unsafe { CloseHandle(handle) };
    } else {
        return true.into();
    }
    if length > 0
        && String::from_utf16_lossy(&path[..length as usize])
            .trim_start_matches(r"\\?\")
            .eq_ignore_ascii_case(search.executable.trim_start_matches(r"\\?\"))
    {
        search.windows.push(window);
    }
    true.into()
}

fn is_main_window_title(title: &str) -> bool {
    title
        .rsplit_once(" v")
        .is_some_and(|(_, version)| version.chars().next().is_some_and(|c| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_window_does_not_match_the_main_window() {
        assert!(is_main_window_title("Ludusavi v0.31.0"));
        assert!(is_main_window_title("لودوسافي v0.31.0 (debug)"));
        assert!(!is_main_window_title("Ludusavi — Automatic backups"));
        assert!(!is_main_window_title("Ludusavi"));
    }

    #[test]
    fn native_reopen_selects_the_main_window_with_settings_open() {
        use windows::{
            Win32::UI::WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, UnregisterClassW, WINDOW_EX_STYLE,
                WNDCLASSW, WS_OVERLAPPEDWINDOW,
            },
            core::w,
        };
        unsafe extern "system" fn window_proc(
            window: HWND,
            message: u32,
            wparam: windows::Win32::Foundation::WPARAM,
            lparam: LPARAM,
        ) -> windows::Win32::Foundation::LRESULT {
            unsafe { DefWindowProcW(window, message, wparam, lparam) }
        }
        unsafe {
            let class = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                lpszClassName: w!("Window Class"),
                ..Default::default()
            };
            assert_ne!(RegisterClassW(&class), 0);
            let create = |title| {
                CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    class.lpszClassName,
                    title,
                    WS_OVERLAPPEDWINDOW,
                    0,
                    0,
                    320,
                    200,
                    None,
                    None,
                    None,
                    None,
                )
                .unwrap()
            };
            let main = create(w!("Ludusavi v0.31.0"));
            let settings = create(w!("Ludusavi — Automatic backups"));
            let outcome = show_existing();
            let main_visible = IsWindowVisible(main).as_bool();
            let settings_visible = IsWindowVisible(settings).as_bool();
            DestroyWindow(main).unwrap();
            DestroyWindow(settings).unwrap();
            UnregisterClassW(class.lpszClassName, None).unwrap();
            assert!(outcome.is_some());
            assert!(main_visible);
            assert!(!settings_visible);
        }
    }
}
