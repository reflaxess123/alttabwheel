use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use winapi::ctypes::*;
use winapi::shared::minwindef::*;
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::winuser::*;

lazy_static::lazy_static! {
    static ref ALT_TAB_ACTIVE: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    static ref XBUTTON1_PRESSED: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    static ref XBUTTON2_PRESSED: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
}

const WM_MOUSEWHEEL: u32 = 0x020A;
const WM_XBUTTONDOWN: u32 = 0x020B;
const WM_XBUTTONUP: u32 = 0x020C;

const XBUTTON1: u32 = 0x0001;
const XBUTTON2: u32 = 0x0002;

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

fn main() {
    unsafe {
        let h_instance = GetModuleHandleW(ptr::null());
        
        // Install low-level mouse hook
        let mouse_hook = SetWindowsHookExW(
            WH_MOUSE_LL,
            Some(low_level_mouse_proc),
            h_instance,
            0,
        );
        
        if mouse_hook.is_null() {
            eprintln!("Failed to install mouse hook. Error: {}", GetLastError());
            return;
        }
        
        // Install low-level keyboard hook
        let keyboard_hook = SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(low_level_keyboard_proc),
            h_instance,
            0,
        );
        
        if keyboard_hook.is_null() {
            eprintln!("Failed to install keyboard hook. Error: {}", GetLastError());
            UnhookWindowsHookEx(mouse_hook);
            return;
        }
        
        println!("Alt+Tab mouse wheel navigation active!");
        println!("Hold XButton1 or XButton2 and scroll mouse wheel to navigate Alt+Tab");
        println!("Press Ctrl+C to exit");
        
        // Message loop
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) != 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        
        // Cleanup
        UnhookWindowsHookEx(mouse_hook);
        UnhookWindowsHookEx(keyboard_hook);
    }
}