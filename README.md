# Alt+Tab Mouse Wheel Navigation

A Windows application that allows you to navigate Alt+Tab using mouse wheel while holding side mouse buttons.

## Features

- Hold **XButton1** (back button) or **XButton2** (forward button) and scroll mouse wheel to navigate Alt+Tab
- Scroll **up** - forward navigation (next window)
- Scroll **down** - reverse navigation (previous window)
- Automatically handles Alt+Tab state management
- Releases Alt key when side button is released

## Requirements

- Windows OS
- Mouse with side buttons (XButton1/XButton2)
- Administrator privileges (required for global hooks)

## Installation

### Option 1: Download Release
1. Download `alttabwheel.exe` from releases
2. Run as administrator

### Option 2: Build from Source
1. Install Rust: https://rustup.rs/
2. Clone repository
3. Build for Windows:
   ```bash
   rustup target add x86_64-pc-windows-gnu
   cargo build --release --target x86_64-pc-windows-gnu
   ```
4. Executable will be in `target/x86_64-pc-windows-gnu/release/alttabwheel.exe`

## Usage

1. Run `alttabwheel.exe` **as administrator**
2. Hold side mouse button (XButton1 or XButton2)
3. Scroll mouse wheel up/down to navigate windows
4. Release side button to select window
5. Press Ctrl+C to exit

## Technical Details

- Uses Windows low-level mouse and keyboard hooks
- Implements proper Alt+Tab state management
- Handles Shift+Tab for reverse navigation
- Cross-compiled from Rust to Windows executable

## Replacement for AutoHotkey

This tool replaces the following AutoHotkey v2 script:

```autohotkey
; Alt+Tab Mouse Wheel Navigation for AutoHotkey v2
; Hold side mouse button and scroll wheel to navigate Alt+Tab

#Requires AutoHotkey v2.0

AltTabActive := false

XButton1 & WheelUp::
{
    global AltTabActive
    if (!AltTabActive) {
        Send("{Alt down}{Tab}")
        AltTabActive := true
    } else {
        Send("{Tab}")
    }
}

; ... (rest of AHK script)
```

## License

MIT License