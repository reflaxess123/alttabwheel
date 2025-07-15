#![windows_subsystem = "windows"]

use std::ffi::OsStr;
use std::iter::once;
use std::mem;
use std::os::windows::ffi::OsStrExt;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use winapi::ctypes::*;
use winapi::shared::minwindef::*;
use winapi::shared::windef::*;
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::shellapi::*;
use winapi::um::winuser::*;

lazy_static::lazy_static! {
    static ref ALT_TAB_ACTIVE: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    static ref XBUTTON1_PRESSED: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    static ref XBUTTON2_PRESSED: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    static ref SHOULD_EXIT: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
}

const WM_MOUSEWHEEL: u32 = 0x020A;
const WM_XBUTTONDOWN: u32 = 0x020B;
const WM_XBUTTONUP: u32 = 0x020C;
const WM_TRAYICON: u32 = WM_USER + 1;
const ID_TRAY_EXIT: u32 = 1001;

const XBUTTON1: u32 = 0x0001;
const XBUTTON2: u32 = 0x0002;

fn to_wide_chars(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(once(0)).collect()
}

fn send_key_combination(vk_codes: &[u32], down: bool) {
    for &vk in vk_codes {
        unsafe {
            let mut input = INPUT {
                type_: INPUT_KEYBOARD,
                u: std::mem::zeroed(),
            };
            
            *input.u.ki_mut() = KEYBDINPUT {
                wVk: vk as u16,
                wScan: 0,
                dwFlags: if down { 0 } else { KEYEVENTF_KEYUP },
                time: 0,
                dwExtraInfo: 0,
            };
            
            SendInput(1, &mut input, std::mem::size_of::<INPUT>() as c_int);
        }
    }
}

fn send_alt_tab(reverse: bool) {
    let alt_tab_active = ALT_TAB_ACTIVE.load(Ordering::Relaxed);
    
    if !alt_tab_active {
        // Start Alt+Tab
        if reverse {
            send_key_combination(&[VK_MENU as u32, VK_SHIFT as u32, VK_TAB as u32], true);
        } else {
            send_key_combination(&[VK_MENU as u32, VK_TAB as u32], true);
        }
        ALT_TAB_ACTIVE.store(true, Ordering::Relaxed);
    } else {
        // Continue Alt+Tab navigation
        if reverse {
            send_key_combination(&[VK_SHIFT as u32, VK_TAB as u32], true);
            send_key_combination(&[VK_SHIFT as u32, VK_TAB as u32], false);
        } else {
            send_key_combination(&[VK_TAB as u32], true);
            send_key_combination(&[VK_TAB as u32], false);
        }
    }
}

fn end_alt_tab() {
    if ALT_TAB_ACTIVE.load(Ordering::Relaxed) {
        // Release Shift first if pressed
        unsafe {
            if GetKeyState(VK_SHIFT) & 0x8000u16 as i16 != 0 {
                send_key_combination(&[VK_SHIFT as u32], false);
            }
        }
        
        // Release Alt
        send_key_combination(&[VK_MENU as u32], false);
        ALT_TAB_ACTIVE.store(false, Ordering::Relaxed);
    }
}

unsafe extern "system" fn low_level_mouse_proc(
    n_code: c_int,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0 {
        match w_param as u32 {
            WM_XBUTTONDOWN => {
                let mouse_data = *(l_param as *const MSLLHOOKSTRUCT);
                let button = HIWORD(mouse_data.mouseData as u32);
                
                if u32::from(button) == XBUTTON1 {
                    XBUTTON1_PRESSED.store(true, Ordering::Relaxed);
                } else if u32::from(button) == XBUTTON2 {
                    XBUTTON2_PRESSED.store(true, Ordering::Relaxed);
                }
            }
            WM_XBUTTONUP => {
                let mouse_data = *(l_param as *const MSLLHOOKSTRUCT);
                let button = HIWORD(mouse_data.mouseData as u32);
                
                if u32::from(button) == XBUTTON1 {
                    XBUTTON1_PRESSED.store(false, Ordering::Relaxed);
                    end_alt_tab();
                } else if u32::from(button) == XBUTTON2 {
                    XBUTTON2_PRESSED.store(false, Ordering::Relaxed);
                    end_alt_tab();
                }
            }
            WM_MOUSEWHEEL => {
                let xbutton1_pressed = XBUTTON1_PRESSED.load(Ordering::Relaxed);
                let xbutton2_pressed = XBUTTON2_PRESSED.load(Ordering::Relaxed);
                
                if xbutton1_pressed || xbutton2_pressed {
                    let mouse_data = *(l_param as *const MSLLHOOKSTRUCT);
                    let wheel_delta = HIWORD(mouse_data.mouseData as u32) as i16;
                    
                    if wheel_delta > 0 {
                        // Wheel up - forward navigation
                        send_alt_tab(false);
                    } else if wheel_delta < 0 {
                        // Wheel down - reverse navigation
                        send_alt_tab(true);
                    }
                    
                    return 1; // Consume the message
                }
            }
            _ => {}
        }
    }
    
    CallNextHookEx(ptr::null_mut(), n_code, w_param, l_param)
}

unsafe extern "system" fn low_level_keyboard_proc(
    n_code: c_int,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0 && w_param as u32 == WM_KEYUP {
        let keyboard_data = *(l_param as *const KBDLLHOOKSTRUCT);
        if keyboard_data.vkCode == VK_MENU as u32 {
            // Alt key released - safety measure
            ALT_TAB_ACTIVE.store(false, Ordering::Relaxed);
        }
    }
    
    CallNextHookEx(ptr::null_mut(), n_code, w_param, l_param)
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: UINT,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    match msg {
        WM_TRAYICON => {
            match l_param as u32 {
                WM_RBUTTONUP => {
                    // Show context menu
                    let mut pt: POINT = mem::zeroed();
                    GetCursorPos(&mut pt);
                    
                    let hmenu = CreatePopupMenu();
                    if !hmenu.is_null() {
                        let exit_text = to_wide_chars("Exit");
                        AppendMenuW(hmenu, MF_STRING, ID_TRAY_EXIT as usize, exit_text.as_ptr());
                        
                        SetForegroundWindow(hwnd);
                        TrackPopupMenu(
                            hmenu,
                            TPM_RIGHTBUTTON,
                            pt.x,
                            pt.y,
                            0,
                            hwnd,
                            ptr::null(),
                        );
                        DestroyMenu(hmenu);
                    }
                }
                _ => {}
            }
        }
        WM_COMMAND => {
            if u32::from(LOWORD(w_param as u32)) == ID_TRAY_EXIT {
                SHOULD_EXIT.store(true, Ordering::Relaxed);
                PostQuitMessage(0);
            }
        }
        WM_DESTROY => {
            PostQuitMessage(0);
        }
        _ => return DefWindowProcW(hwnd, msg, w_param, l_param),
    }
    0
}

fn create_tray_icon(hwnd: HWND) -> bool {
    unsafe {
        let mut nid: NOTIFYICONDATAW = mem::zeroed();
        nid.cbSize = mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
        nid.uCallbackMessage = WM_TRAYICON;
        
        // Load default application icon
        nid.hIcon = LoadIconW(ptr::null_mut(), IDI_APPLICATION);
        
        // Set tooltip
        let tooltip = to_wide_chars("Alt+Tab Mouse Wheel Navigation");
        let tooltip_len = tooltip.len().min(128);
        ptr::copy_nonoverlapping(tooltip.as_ptr(), nid.szTip.as_mut_ptr(), tooltip_len);
        
        Shell_NotifyIconW(NIM_ADD, &mut nid) != 0
    }
}

fn remove_tray_icon(hwnd: HWND) {
    unsafe {
        let mut nid: NOTIFYICONDATAW = mem::zeroed();
        nid.cbSize = mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        Shell_NotifyIconW(NIM_DELETE, &mut nid);
    }
}

fn main() {
    unsafe {
        let h_instance = GetModuleHandleW(ptr::null());
        
        // Register window class
        let class_name = to_wide_chars("AltTabWheelClass");
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: h_instance,
            hIcon: LoadIconW(ptr::null_mut(), IDI_APPLICATION),
            hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
            hbrBackground: ptr::null_mut(),
            lpszMenuName: ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };
        
        if RegisterClassW(&wc) == 0 {
            return;
        }
        
        // Create hidden window
        let window_name = to_wide_chars("Alt+Tab Mouse Wheel Navigation");
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            window_name.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            ptr::null_mut(),
            ptr::null_mut(),
            h_instance,
            ptr::null_mut(),
        );
        
        if hwnd.is_null() {
            return;
        }
        
        // Create tray icon
        if !create_tray_icon(hwnd) {
            return;
        }
        
        // Install hooks
        let mouse_hook = SetWindowsHookExW(
            WH_MOUSE_LL,
            Some(low_level_mouse_proc),
            h_instance,
            0,
        );
        
        if mouse_hook.is_null() {
            remove_tray_icon(hwnd);
            return;
        }
        
        let keyboard_hook = SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(low_level_keyboard_proc),
            h_instance,
            0,
        );
        
        if keyboard_hook.is_null() {
            UnhookWindowsHookEx(mouse_hook);
            remove_tray_icon(hwnd);
            return;
        }
        
        // Message loop
        let mut msg: MSG = mem::zeroed();
        while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            if SHOULD_EXIT.load(Ordering::Relaxed) {
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        
        // Cleanup
        UnhookWindowsHookEx(mouse_hook);
        UnhookWindowsHookEx(keyboard_hook);
        remove_tray_icon(hwnd);
    }
}