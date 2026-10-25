//! The Win32 calls. Everything here is `#[cfg(windows)]`; the pure halves are in `pure.rs` and the
//! live behaviour is `#[ignore]`d tests with what would prove each.

use std::ffi::c_void;

use seal_domain::geometry::{DisplayInfo, Point, Rect, Size};
use windows_sys::Win32::Foundation::{BOOL, GetLastError, HWND, LPARAM, POINT, RECT, TRUE};
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CAPTUREBLT, ClientToScreen,
    CreateCompatibleBitmap, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC,
    GetDIBits, RGBQUAD, ReleaseDC, SRCCOPY, SelectObject,
};
use windows_sys::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClientRect, GetForegroundWindow, GetSystemMetrics, GetWindowRect,
    GetWindowTextLengthW, GetWindowTextW, IsWindowVisible, SM_CXSCREEN, SM_CYSCREEN,
};

use crate::error::WinError;
use crate::pure::{encode_png, region_to_screen, title_matches};

/// A window handle as a plain value so the core can hold it without Win32 types. Zero is never a
/// valid window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hwnd(pub isize);

impl Hwnd {
    fn ptr(self) -> HWND {
        self.0
    }
}

/// One capture: the PNG bytes and where its top-left sits in physical screen pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Captured {
    pub width: i32,
    pub height: i32,
    pub origin: Point,
    pub png: Vec<u8>,
}

/// Find the first visible top-level window whose title contains `title_contains` (case-insensitive;
/// see `pure::title_matches`). Enumeration order is window-manager order, so the first match is the
/// oldest top-level window with that title — the game has one.
pub fn find_window(title_contains: &str) -> Option<Hwnd> {
    if title_contains.trim().is_empty() {
        return None;
    }

    struct Ctx<'a> {
        needle: &'a str,
        found: Option<isize>,
    }

    unsafe extern "system" fn callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        // SAFETY: the caller passes a live `Ctx` for the duration of the synchronous enumeration.
        unsafe {
            let ctx = &mut *(lparam as *mut Ctx<'_>);
            if IsWindowVisible(hwnd) == 0 {
                return TRUE;
            }
            if let Some(title) = window_title(hwnd)
                && title_matches(&title, ctx.needle)
            {
                ctx.found = Some(hwnd);
                return 0; // stop enumeration
            }
            TRUE
        }
    }

    let mut ctx = Ctx {
        needle: title_contains,
        found: None,
    };
    // SAFETY: the context outlives the synchronous enumeration; the callback only reads it.
    unsafe {
        EnumWindows(Some(callback), &mut ctx as *mut Ctx<'_> as LPARAM);
    }
    ctx.found.map(Hwnd)
}

fn window_title(hwnd: HWND) -> Option<String> {
    // SAFETY: hwnd is a live top-level window from EnumWindows.
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return None;
        }
        let mut buf = vec![0u16; len as usize + 1];
        let written = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        if written <= 0 {
            return None;
        }
        buf.truncate(written as usize);
        Some(String::from_utf16_lossy(&buf))
    }
}

/// The title bar text of a window, for the protocol's `window.find` result. `None` for a window
/// with no text (or one that closed between the find and the read).
pub fn title_text(hwnd: Hwnd) -> Option<String> {
    window_title(hwnd.ptr())
}

/// Is a window the foreground window right now? Capture is refused otherwise: a capture of a covered
/// game photographs the covering window (`CALIBRATION.md` §3).
pub fn is_foreground(hwnd: Hwnd) -> bool {
    // SAFETY: both calls are read-only queries; no handle is retained.
    unsafe { GetForegroundWindow() == hwnd.ptr() }
}

/// Measure the window in both DPI spaces: the physical reads run under a THREAD-scoped aware context
/// (the process stays unaware), then the logical client is read under the restored context. The
/// scale is the ratio of client WIDTHS — heights truncate at fractional scale, widths do not
/// (`CALIBRATION.md` §2).
pub fn measure(hwnd: Hwnd) -> Result<DisplayInfo, WinError> {
    // SAFETY: the thread-scoped context is restored on both return paths below; no handle escapes.
    unsafe {
        let previous = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let physical = read_physical(hwnd);
        SetThreadDpiAwarenessContext(previous);
        let (frame, physical_client) = physical?;
        let logical_client = read_logical_client(hwnd)?;
        if physical_client.w <= 0 || physical_client.h <= 0 {
            return Err(WinError::WindowNotFound);
        }
        let screen = screen_size();
        Ok(DisplayInfo::from_rects(
            frame,
            physical_client,
            logical_client,
            screen,
        ))
    }
}

fn rect_from(raw: RECT) -> Rect {
    Rect::new(
        raw.left,
        raw.top,
        raw.right - raw.left,
        raw.bottom - raw.top,
    )
}

fn client_origin(hwnd: HWND) -> Result<POINT, WinError> {
    let mut origin = POINT { x: 0, y: 0 };
    // SAFETY: a read-only query on a live window handle.
    if unsafe { ClientToScreen(hwnd, &mut origin) } == 0 {
        return Err(WinError::win32("ClientToScreen", unsafe { GetLastError() }));
    }
    Ok(origin)
}

fn read_physical(hwnd: Hwnd) -> Result<(Rect, Rect), WinError> {
    let mut frame = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    let mut client = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: read-only queries on a live window handle.
    if unsafe { GetWindowRect(hwnd.ptr(), &mut frame) } == 0 {
        return Err(WinError::win32("GetWindowRect", unsafe { GetLastError() }));
    }
    if unsafe { GetClientRect(hwnd.ptr(), &mut client) } == 0 {
        return Err(WinError::win32("GetClientRect", unsafe { GetLastError() }));
    }
    let origin = client_origin(hwnd.ptr())?;
    let physical_client = Rect::new(
        origin.x,
        origin.y,
        client.right - client.left,
        client.bottom - client.top,
    );
    Ok((rect_from(frame), physical_client))
}

fn read_logical_client(hwnd: Hwnd) -> Result<Rect, WinError> {
    let mut client = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: read-only query on a live window handle.
    if unsafe { GetClientRect(hwnd.ptr(), &mut client) } == 0 {
        return Err(WinError::win32("GetClientRect(logical)", unsafe {
            GetLastError()
        }));
    }
    let origin = client_origin(hwnd.ptr())?;
    Ok(Rect::new(
        origin.x,
        origin.y,
        client.right - client.left,
        client.bottom - client.top,
    ))
}

fn screen_size() -> Size {
    // SAFETY: read-only system queries.
    Size::new(unsafe { GetSystemMetrics(SM_CXSCREEN) }, unsafe {
        GetSystemMetrics(SM_CYSCREEN)
    })
}

/// Capture the client area (or a client-relative physical region of it) as a PNG, refusing unless the
/// game is in front. `origin` in the result is where the image's top-left sits on the physical
/// screen, which is what `Frame.origin` carries.
pub fn capture_client_region(hwnd: Hwnd, region: Option<Rect>) -> Result<Captured, WinError> {
    let display = measure(hwnd)?;
    if !is_foreground(hwnd) {
        return Err(WinError::NotForeground);
    }
    let client = display.physical_client;
    let screen_rect = match region {
        Some(region) => region_to_screen(client, region)?,
        None => client,
    };

    // SAFETY: every GDI handle is released before returning, on the success and error paths alike.
    unsafe {
        let screen_dc = GetDC(0);
        if screen_dc == 0 {
            return Err(WinError::win32("GetDC", GetLastError()));
        }
        let mem_dc = CreateCompatibleDC(screen_dc);
        if mem_dc == 0 {
            ReleaseDC(0, screen_dc);
            return Err(WinError::win32("CreateCompatibleDC", GetLastError()));
        }
        let bitmap = CreateCompatibleBitmap(screen_dc, screen_rect.w, screen_rect.h);
        if bitmap == 0 {
            DeleteDC(mem_dc);
            ReleaseDC(0, screen_dc);
            return Err(WinError::win32("CreateCompatibleBitmap", GetLastError()));
        }
        let previous = SelectObject(mem_dc, bitmap);

        let copied = BitBlt(
            mem_dc,
            0,
            0,
            screen_rect.w,
            screen_rect.h,
            screen_dc,
            screen_rect.x,
            screen_rect.y,
            SRCCOPY | CAPTUREBLT,
        );
        let mut pixels = vec![0u8; screen_rect.w as usize * screen_rect.h as usize * 4];
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: screen_rect.w,
            biHeight: -screen_rect.h, // negative = top-down rows
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        };
        info.bmiColors = [RGBQUAD {
            rgbBlue: 0,
            rgbGreen: 0,
            rgbRed: 0,
            rgbReserved: 0,
        }];
        let lines = if copied != 0 {
            GetDIBits(
                mem_dc,
                bitmap,
                0,
                screen_rect.h as u32,
                pixels.as_mut_ptr().cast::<c_void>(),
                &mut info,
                DIB_RGB_COLORS,
            )
        } else {
            0
        };

        SelectObject(mem_dc, previous);
        DeleteObject(bitmap);
        DeleteDC(mem_dc);
        ReleaseDC(0, screen_dc);

        if copied == 0 {
            return Err(WinError::win32("BitBlt", GetLastError()));
        }
        if lines == 0 {
            return Err(WinError::win32("GetDIBits", GetLastError()));
        }

        let png = encode_png(screen_rect.w as u32, screen_rect.h as u32, &pixels)?;
        Ok(Captured {
            width: screen_rect.w,
            height: screen_rect.h,
            origin: Point::new(screen_rect.x, screen_rect.y),
            png,
        })
    }
}

#[cfg(test)]
mod tests {
    /// Live only: needs this machine's game window. Run by hand:
    /// `$env:SEAL_WINDOW_TITLE="Seal Online"; cargo test -p seal-win -- --ignored`.
    #[test]
    #[ignore = "needs the running game window"]
    fn finds_and_measures_the_game_window() {
        let title = std::env::var("SEAL_WINDOW_TITLE").unwrap_or_else(|_| "Seal Online".into());
        let hwnd = super::find_window(&title).expect("the game window is running");
        let display = super::measure(hwnd).expect("measures");
        assert!(display.physical_client.w > 0);
        assert!(display.scale >= 1.0);
        let captured = super::capture_client_region(hwnd, None).expect("captures");
        assert!(captured.png.len() > 1000, "a real frame is not tiny");
        assert_eq!(captured.width, display.physical_client.w);
    }

    /// Live only: the foreground refusal is the guard that makes a covered game unphotographable.
    #[test]
    #[ignore = "needs two windows and a background game"]
    fn a_non_foreground_window_is_refused() {
        use crate::error::WinError;
        let title = std::env::var("SEAL_WINDOW_TITLE").unwrap_or_else(|_| "Seal Online".into());
        if let Some(hwnd) = super::find_window(&title)
            && !super::is_foreground(hwnd)
        {
            assert_eq!(
                super::capture_client_region(hwnd, None),
                Err(WinError::NotForeground)
            );
        }
    }
}
