//! The notification-area icon on Windows: `Shell_NotifyIconW` on a
//! message-only window, the app's own icon (resource 1, as gpui loads it),
//! the total in the tooltip, and a menu on right-click; a left-click opens
//! the window.
//!
//! The message-only window lives on gpui's thread, whose message loop already
//! pumps every window this process owns, so the icon's callbacks arrive with
//! no thread of their own. Only `windows-sys` here, so this file can be
//! type-checked from any machine (`scripts/check-windows.sh` lifts it).

use std::cell::RefCell;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    Shell_NotifyIconW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    GetCursorPos, HMENU, HWND_MESSAGE, LoadIconW, MF_GRAYED, MF_SEPARATOR, MF_STRING,
    PostMessageW, RegisterClassW, SetForegroundWindow, TPM_NONOTIFY, TPM_RETURNCMD,
    TPM_RIGHTBUTTON, TrackPopupMenu, WM_APP, WM_CONTEXTMENU, WM_LBUTTONUP, WM_NULL,
    WM_RBUTTONUP, WNDCLASSW,
};

use super::{TrayCommand, TrayModel, press};

/// The icon's callback message.
const CALLBACK: u32 = WM_APP + 0x56;
/// The menu's command ids.
const ID_SEND: usize = 1;
const ID_RECEIVE: usize = 2;
const ID_OPEN: usize = 3;
/// The one icon this process adds.
const ICON_ID: u32 = 1;

struct Icon {
    hwnd: HWND,
    /// The menu's words, kept for the next right-click.
    model: TrayModel,
}

thread_local! {
    static ICON: RefCell<Option<Icon>> = const { RefCell::new(None) };
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

/// `text` into a fixed UTF-16 buffer, cut to fit with its NUL.
fn fill(buffer: &mut [u16], text: &str) {
    let mut units: Vec<u16> = text.encode_utf16().collect();
    units.truncate(buffer.len().saturating_sub(1));
    buffer[..units.len()].copy_from_slice(&units);
    buffer[units.len()] = 0;
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == CALLBACK {
        // The low word of lparam is the mouse message (NOTIFYICON_VERSION 0).
        match (lparam & 0xffff) as u32 {
            WM_LBUTTONUP => press(TrayCommand::Open),
            WM_RBUTTONUP | WM_CONTEXTMENU => show_menu(hwnd),
            _ => {}
        }
        return 0;
    }
    // SAFETY: the default procedure for every other message.
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

fn show_menu(hwnd: HWND) {
    let model = ICON.with(|slot| slot.borrow().as_ref().map(|icon| icon.model.clone()));
    let Some(model) = model else {
        return;
    };
    // SAFETY: a popup menu made, shown and destroyed here; strings outlive
    // the calls that read them.
    unsafe {
        let menu: HMENU = CreatePopupMenu();
        if menu.is_null() {
            return;
        }
        let mut said = false;
        for line in [model.figure.as_deref(), model.updated.as_deref()]
            .into_iter()
            .flatten()
        {
            let text = wide(line);
            AppendMenuW(menu, MF_STRING | MF_GRAYED, 0, text.as_ptr());
            said = true;
        }
        if said {
            AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
        }
        let send = wide(&model.send);
        let receive = wide(&model.receive);
        let open = wide(&model.open);
        AppendMenuW(menu, MF_STRING, ID_SEND, send.as_ptr());
        AppendMenuW(menu, MF_STRING, ID_RECEIVE, receive.as_ptr());
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
        AppendMenuW(menu, MF_STRING, ID_OPEN, open.as_ptr());
        let mut at = POINT { x: 0, y: 0 };
        GetCursorPos(&mut at);
        // The documented dance: the owner in front, or the menu never
        // closes on a click elsewhere; WM_NULL after, or the next click is
        // swallowed.
        SetForegroundWindow(hwnd);
        let chosen = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
            at.x,
            at.y,
            0,
            hwnd,
            std::ptr::null(),
        );
        PostMessageW(hwnd, WM_NULL, 0, 0);
        DestroyMenu(menu);
        match usize::try_from(chosen).unwrap_or(0) {
            ID_SEND => press(TrayCommand::Send),
            ID_RECEIVE => press(TrayCommand::Receive),
            ID_OPEN => press(TrayCommand::Open),
            _ => {}
        }
    }
}

fn message_window() -> Option<HWND> {
    // SAFETY: a window class registered once (a second registration fails
    // harmlessly) and a message-only window on this thread.
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let class_name = wide("VelaWalletTray");
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: class_name.as_ptr(),
            ..std::mem::zeroed()
        };
        RegisterClassW(&class);
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            class_name.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        );
        (!hwnd.is_null()).then_some(hwnd)
    }
}

fn data(hwnd: HWND, model: &TrayModel) -> NOTIFYICONDATAW {
    // SAFETY: a plain-old-data struct, zeroed as the API documents.
    let mut data: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    data.cbSize = u32::try_from(std::mem::size_of::<NOTIFYICONDATAW>()).unwrap_or(0);
    data.hWnd = hwnd;
    data.uID = ICON_ID;
    data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    data.uCallbackMessage = CALLBACK;
    // Resource 1: the icon `build.rs` embeds and gpui's window uses.
    // SAFETY: MAKEINTRESOURCE(1) on this module.
    data.hIcon = unsafe { LoadIconW(GetModuleHandleW(std::ptr::null()), 1 as _) };
    let tip = [
        Some(model.open.as_str()),
        model.figure.as_deref(),
        model.updated.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("\n");
    fill(&mut data.szTip, &tip);
    data
}

pub fn show(model: &TrayModel) {
    ICON.with(|slot| {
        let mut slot = slot.borrow_mut();
        match slot.as_mut() {
            Some(icon) => {
                icon.model = model.clone();
                let data = data(icon.hwnd, model);
                // SAFETY: the icon this window added.
                unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) };
            }
            None => {
                let Some(hwnd) = message_window() else {
                    return;
                };
                let data = data(hwnd, model);
                // SAFETY: a new icon on our window.
                if unsafe { Shell_NotifyIconW(NIM_ADD, &data) } == 0 {
                    // SAFETY: the window made just above.
                    unsafe { DestroyWindow(hwnd) };
                    return;
                }
                *slot = Some(Icon {
                    hwnd,
                    model: model.clone(),
                });
            }
        }
    });
}

pub fn hide() {
    ICON.with(|slot| {
        if let Some(icon) = slot.borrow_mut().take() {
            let data = data(icon.hwnd, &icon.model);
            // SAFETY: the icon and window this module made.
            unsafe {
                Shell_NotifyIconW(NIM_DELETE, &data);
                DestroyWindow(icon.hwnd);
            }
        }
    });
}
