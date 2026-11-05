//! The Win32 calls. Everything here is `#[cfg(windows)]`; the pure halves are in `pure.rs` and the
//! live behaviour is `#[ignore]`d tests with what would prove each.
//!
//! # The unsafe blocks
//!
//! Every `unsafe` block in this file is one Win32 FFI call (or the one pointer the `EnumWindows`
//! callback receives). Safe preparation, validation and buffer sizing stay OUTSIDE the blocks, and
//! each block carries a `// SAFETY:` argument naming the API's documented precondition and why this
//! call site meets it: pointer validity/alignment/lifetime, buffer sizes and element counts, handle
//! validity and null handling, thread context, and what a failure means. A handle is an opaque
//! value here, never a pointer, so a stale or null `HWND` can only make the API fail — it cannot
//! make the Rust side unsound. The capture path's GDI handles are owned by the small RAII guards
//! below (`ScreenDc`, `MemDc`, `GdiBitmap`, `Selected`), so an early `?` cannot leak one and a new
//! call between acquire and release cannot be added without its cleanup (HW-F13).

use std::ffi::c_void;

use seal_domain::geometry::{DisplayInfo, Point, Rect, Size};
use windows_sys::Win32::Foundation::{BOOL, GetLastError, HWND, LPARAM, POINT, RECT, TRUE};
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CAPTUREBLT, ClientToScreen,
    CreateCompatibleBitmap, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC,
    GetDIBits, HBITMAP, HDC, HGDIOBJ, RGBQUAD, ReleaseDC, SRCCOPY, SelectObject,
};
use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows_sys::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GetClientRect, GetCursorPos, GetForegroundWindow,
    GetSystemMetrics, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, IsIconic, IsWindowVisible, SM_CXSCREEN, SM_CYSCREEN, SW_RESTORE,
    SetForegroundWindow, ShowWindow,
};

use crate::error::WinError;
use crate::pure::{encode_png, region_to_screen, title_matches};

/// `SelectObject`'s failure sentinel for a failed region selection (`(HGDIOBJ)-1`), which
/// windows-sys exposes as a macro rather than a constant. The capture path only ever selects
/// bitmaps (failure is NULL there), but both are checked so a region can never pass as success.
const HGDI_ERROR: HGDIOBJ = -1;

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

/// The calling thread's last Win32 error, for the log line.
///
/// Used immediately after a failed Win32 call on the same thread: `GetLastError` is thread-local
/// and any later call may overwrite it, so every error path here reads it before doing cleanup
/// work.
fn last_error() -> u32 {
    // SAFETY: `GetLastError` takes no arguments, touches no caller memory and has no preconditions;
    // it only reads the calling thread's error slot.
    unsafe { GetLastError() }
}

/// Find the first visible top-level window whose title contains `title_contains` (case-insensitive;
/// see `pure::title_matches`). Enumeration order is window-manager order, so the first match is the
/// oldest top-level window with that title — the game has one.
pub fn find_window(title_contains: &str) -> Option<Hwnd> {
    struct Ctx<'a> {
        needle: &'a str,
        found: Option<isize>,
    }

    /// The `EnumWindows` callback: `lparam` is the `&mut Ctx` `find_window` passes, and returning 0
    /// stops the enumeration.
    ///
    /// # Safety
    ///
    /// Must only be called by `EnumWindows` during `find_window`'s synchronous call, when `lparam`
    /// is the live `Ctx` pointer it was given.
    unsafe extern "system" fn callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        // SAFETY: `find_window` passes `&raw mut ctx`, its live stack `Ctx`, as `lparam`;
        // `EnumWindows` invokes this callback synchronously on the same thread before
        // `find_window` can return, and never calls it concurrently, so the pointer is non-null,
        // correctly aligned and valid for the whole call — and no other reference to the `Ctx`
        // exists across it, so the `&mut` does not alias.
        let ctx = unsafe { &mut *(lparam as *mut Ctx<'_>) };
        // SAFETY: `IsWindowVisible` is a read-only query of the window's style bits. It takes the
        // handle by value (an opaque value, not a pointer), writes nothing, and an invalid or stale
        // handle only makes it return 0.
        if unsafe { IsWindowVisible(hwnd) } == 0 {
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

    if title_contains.trim().is_empty() {
        return None;
    }

    let mut ctx = Ctx {
        needle: title_contains,
        found: None,
    };
    // `&raw mut` creates no reference, so the callback's `&mut` is the only borrow during the call.
    let lparam = &raw mut ctx as LPARAM;
    // SAFETY: `callback` matches the `WNDENUMPROC` contract (`unsafe extern "system"`, same
    // signature); `lparam` is the live `ctx` pointer as an integer, and `EnumWindows` neither
    // keeps nor dereferences it after this synchronous call returns.
    unsafe { EnumWindows(Some(callback), lparam) };
    ctx.found.map(Hwnd)
}

fn window_title(hwnd: HWND) -> Option<String> {
    // The Win32 text calls speak `i32` counts; the checks here are the ones the casts would
    // otherwise hide, so a negative count is "no text" rather than a wrapped buffer length.
    // SAFETY: `GetWindowTextLengthW` takes the window handle by value, writes nothing through a
    // caller pointer and only reads the window's current caption length; a stale/invalid handle
    // returns 0, which the `usize` conversion maps to `None`.
    let len = usize::try_from(unsafe { GetWindowTextLengthW(hwnd) }).ok()?;
    if len == 0 {
        return None;
    }
    let mut buf = vec![0u16; len + 1];
    let capacity = i32::try_from(buf.len()).ok()?;
    // SAFETY: `buf` is a live, correctly aligned `u16` buffer of exactly `capacity` elements, which
    // is `nMaxCount` for this call (the API documents that it writes at most `nMaxCount` UTF-16
    // units, including the terminating NUL); it reads only `hwnd`'s caption and returns the number
    // of units copied, which is checked below before the buffer is read.
    let written = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), capacity) };
    let written = usize::try_from(written).ok()?;
    if written == 0 {
        return None;
    }
    buf.truncate(written);
    Some(String::from_utf16_lossy(&buf))
}

/// The title bar text of a window, for the protocol's `window.find` result. `None` for a window
/// with no text (or one that closed between the find and the read).
#[must_use]
pub fn title_text(hwnd: Hwnd) -> Option<String> {
    window_title(hwnd.ptr())
}

/// The OS cursor position, in the space a DPI-UNAWARE process sees (logical screen pixels) — the
/// same space the board's placement loop compares against.
#[must_use]
pub fn cursor_position() -> Option<Point> {
    let mut point = POINT { x: 0, y: 0 };
    // SAFETY: `GetCursorPos` writes one `POINT` through the live, correctly aligned `&mut point`
    // and keeps no reference to it; it has no other preconditions and failure leaves the
    // zero-initialised value untouched (reported by a 0 return, mapped to `None`).
    let ok = unsafe { GetCursorPos(&raw mut point) } != 0;
    ok.then(|| Point::new(point.x, point.y))
}

/// Is a window the foreground window right now? Capture is refused otherwise: a capture of a covered
/// game photographs the covering window (`CALIBRATION.md` §3).
#[must_use]
pub fn is_foreground(hwnd: Hwnd) -> bool {
    // SAFETY: `GetForegroundWindow` takes no arguments and returns an opaque handle value. It is
    // compared by value and never dereferenced, so a null or stale result is safe here.
    let foreground = unsafe { GetForegroundWindow() };
    foreground == hwnd.ptr()
}

/// Try to put a window in the foreground, and report whether Windows accepted the request.
///
/// This is the core's small answer to the app-own-window case: after the player starts a tool, or
/// the shell collapses to its run bar or re-shows itself, the shell can be the foreground window and
/// the first gated capture refuses. Windows lets the process the shell started hand the foreground
/// back to the game — "The calling process was started by the foreground process" is one of the
/// documented conditions in `SetForegroundWindow`'s Remarks — so the core can nudge the game back in
/// front itself, without a shell-side window call.
///
/// `SetForegroundWindow` alone is refused while another application owns the foreground. The
/// calibration capture cannot use a covered frame, so the activation briefly attaches this thread's
/// input state to the foreground thread (the documented `AttachThreadInput` workaround), brings the
/// target to the top, asks for the foreground, then detaches again. The attachment is bounded to
/// this call and is released even when Windows refuses the foreground request.
///
/// Best effort by contract: a `true` means "the request was accepted", not "the game is now in
/// front". Callers must still gate every capture on [`is_foreground`]; this never replaces the
/// pixel-integrity check, it only gives the game a chance to satisfy it.
#[must_use]
pub fn activate(hwnd: Hwnd) -> bool {
    // SAFETY: every call takes an opaque window handle by value or a thread id; none writes through
    // caller memory. `GetWindowThreadProcessId` writes only to the process-id pointer, which is NULL
    // here. `AttachThreadInput` is paired with a detach on every path, and the handles are never
    // dereferenced or retained.
    unsafe {
        let target = hwnd.ptr();
        if GetForegroundWindow() == target {
            return true;
        }
        if IsIconic(target) != 0 {
            ShowWindow(target, SW_RESTORE);
        }
        let foreground = GetForegroundWindow();
        let foreground_thread = if foreground == 0 {
            0
        } else {
            GetWindowThreadProcessId(foreground, std::ptr::null_mut())
        };
        let current = GetCurrentThreadId();
        let attached = foreground_thread != 0
            && foreground_thread != current
            && AttachThreadInput(current, foreground_thread, 1) != 0;
        BringWindowToTop(target);
        let accepted = SetForegroundWindow(target) != 0;
        if attached {
            AttachThreadInput(current, foreground_thread, 0);
        }
        accepted
    }
}

/// Measure the window in both DPI spaces: the physical reads run under a THREAD-scoped aware context
/// (the process stays unaware), then the logical client is read under the restored context. The
/// scale is the ratio of client WIDTHS — heights truncate at fractional scale, widths do not
/// (`CALIBRATION.md` §2).
///
/// # Errors
///
/// Returns [`WinError`] naming the Win32 call that failed when the window or its client rectangle
/// cannot be read, and [`WinError::WindowNotFound`] when the client has no area.
pub fn measure(hwnd: Hwnd) -> Result<DisplayInfo, WinError> {
    // The context is THREAD-scoped, so it makes the reads below return physical pixels without
    // changing the process's DPI awareness. It is restored before the first fallible step, so no
    // early return can leave the thread aware.
    // SAFETY: `SetThreadDpiAwarenessContext` takes an opaque context value (no pointer) and returns
    // the previous context for the calling thread. It has no further preconditions beyond being on
    // the thread whose context is being changed, and this function immediately restores the
    // returned value below.
    let previous =
        unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    let physical = read_physical(hwnd);
    // SAFETY: `previous` is exactly the value the call above returned on this same thread, which is
    // the documented way to restore a thread-scoped awareness context; it is opaque and not
    // dereferenced. A NULL return (the first call failed) is passed back and makes the restore fail
    // harmlessly, matching the existing best-effort behaviour.
    unsafe { SetThreadDpiAwarenessContext(previous) };
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
    // SAFETY: `ClientToScreen` writes the mapped origin into the live, correctly aligned
    // `&mut origin` and keeps no reference to it; `hwnd` is an opaque handle value, so a stale or
    // invalid window only makes the call fail (0) without touching the pointer.
    if unsafe { ClientToScreen(hwnd, &raw mut origin) } == 0 {
        return Err(WinError::win32("ClientToScreen", last_error()));
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
    // SAFETY: `GetWindowRect` writes one `RECT` through the live, correctly aligned `&mut frame`
    // and keeps no reference; the window handle is an opaque value, so a stale/invalid window only
    // makes the call fail (0) and leaves the zero-initialised struct untouched.
    if unsafe { GetWindowRect(hwnd.ptr(), &raw mut frame) } == 0 {
        return Err(WinError::win32("GetWindowRect", last_error()));
    }
    // SAFETY: same contract as `GetWindowRect` above, writing one `RECT` into `&mut client`.
    if unsafe { GetClientRect(hwnd.ptr(), &raw mut client) } == 0 {
        return Err(WinError::win32("GetClientRect", last_error()));
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
    // SAFETY: `GetClientRect` writes one `RECT` through the live, correctly aligned `&mut client`
    // and keeps no reference; the window handle is an opaque value, so a stale/invalid window only
    // makes the call fail (0) and leaves the zero-initialised struct untouched.
    if unsafe { GetClientRect(hwnd.ptr(), &raw mut client) } == 0 {
        return Err(WinError::win32("GetClientRect(logical)", last_error()));
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
    // SAFETY: `GetSystemMetrics` is a read-only integer query with no pointer arguments; the index
    // selects the primary screen's width and a valid index cannot fail.
    let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    // SAFETY: same contract as above, with the height index.
    let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    Size::new(width, height)
}

/// Capture the client area (or a client-relative physical region of it) as a PNG, refusing unless the
/// game is in front. `origin` in the result is where the image's top-left sits on the physical
/// screen, which is what `Frame.origin` carries.
///
/// # Errors
///
/// Returns [`WinError::NotForeground`] when the window is not in front, [`WinError::RegionOutsideClient`]
/// when the requested region does not fit the client, and [`WinError`] naming the Win32 call that
/// failed when a GDI resource or the capture itself cannot be created.
pub fn capture_client_region(hwnd: Hwnd, region: Option<Rect>) -> Result<Captured, WinError> {
    capture_client_region_checked(hwnd, region, true)
}

/// Capture without the foreground gate. This exists ONLY for the shell's calibration capture: the
/// shell hides itself immediately before asking, but Windows can keep the hidden shell as the
/// foreground window, which makes [`capture_client_region`] refuse even though the game is visible.
/// A caller that uses this accepts that a genuinely covering window would be photographed; the frame
/// is shown back to the player, so a wrong calibration picture is visible and recoverable.
///
/// # Errors
///
/// Same as [`capture_client_region`] except that [`WinError::NotForeground`] is never returned.
pub fn capture_client_region_ungated(
    hwnd: Hwnd,
    region: Option<Rect>,
) -> Result<Captured, WinError> {
    capture_client_region_checked(hwnd, region, false)
}

fn capture_client_region_checked(
    hwnd: Hwnd,
    region: Option<Rect>,
    require_foreground: bool,
) -> Result<Captured, WinError> {
    let display = measure(hwnd)?;
    if require_foreground && !is_foreground(hwnd) {
        return Err(WinError::NotForeground);
    }
    let client = display.physical_client;
    let screen_rect = match region {
        Some(region) => region_to_screen(client, region)?,
        None => client,
    };
    let (pixels, width_u32, height_u32) = blit_screen_bgra(screen_rect)?;
    let png = encode_png(width_u32, height_u32, &pixels)?;
    Ok(Captured {
        width: screen_rect.w,
        height: screen_rect.h,
        origin: Point::new(screen_rect.x, screen_rect.y),
        png,
    })
}

/// The safe inputs of one capture: the checked dimensions, the zero-initialised pixel buffer and the
/// DIB description. Prepared entirely before any GDI handle exists, so an allocation failure cannot
/// leak a resource.
///
/// # Errors
///
/// Returns [`WinError::WindowNotFound`] when the rect's dimensions cannot be represented as positive
/// `usize`/`u32` lengths.
fn capture_buffer(rect: Rect) -> Result<(Vec<u8>, BITMAPINFO, u32, u32), WinError> {
    // GDI and the buffer arithmetic speak unsigned/native widths; the rect was validated by the
    // caller, so a failed conversion is a measurement bug rather than a caller error.
    let width = usize::try_from(rect.w).map_err(|_| WinError::WindowNotFound)?;
    let height = usize::try_from(rect.h).map_err(|_| WinError::WindowNotFound)?;
    let width_u32 = u32::try_from(width).map_err(|_| WinError::WindowNotFound)?;
    let height_u32 = u32::try_from(height).map_err(|_| WinError::WindowNotFound)?;
    // The header's size field is Win32's `DWORD`; the struct is a fixed few dozen bytes, so the
    // fallback below is unreachable rather than a clamp.
    let header_size = u32::try_from(std::mem::size_of::<BITMAPINFOHEADER>()).unwrap_or(u32::MAX);
    let pixels = vec![0u8; width * height * 4];
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: header_size,
            biWidth: rect.w,
            biHeight: -rect.h, // negative = top-down rows
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        },
        bmiColors: [RGBQUAD {
            rgbBlue: 0,
            rgbGreen: 0,
            rgbRed: 0,
            rgbReserved: 0,
        }],
    };
    Ok((pixels, info, width_u32, height_u32))
}

/// The screen DC from `GetDC(NULL)`, released on drop. A zero handle is released by nobody: the
/// caller checks it before constructing the guard.
struct ScreenDc(HDC);

impl Drop for ScreenDc {
    fn drop(&mut self) {
        // SAFETY: the handle came from `GetDC(NULL)` and has not been released; this is its one
        // matching `ReleaseDC`. `ReleaseDC` requires the same `HWND` (null = the screen) that the
        // `GetDC` used.
        unsafe { ReleaseDC(0, self.0) };
    }
}

/// A memory DC from `CreateCompatibleDC`, deleted on drop.
struct MemDc(HDC);

impl Drop for MemDc {
    fn drop(&mut self) {
        // SAFETY: the handle came from `CreateCompatibleDC` and is not in use by another DC; this is
        // its one matching `DeleteDC`.
        unsafe { DeleteDC(self.0) };
    }
}

/// A compatible bitmap, deleted on drop. It must not be selected into a DC when this runs: the
/// `Selected` guard is declared after it, so Rust drops it (and restores the DC) first.
struct GdiBitmap(HBITMAP);

impl Drop for GdiBitmap {
    fn drop(&mut self) {
        // SAFETY: the handle came from `CreateCompatibleBitmap`, is not selected into a DC at drop
        // time (see the type's doc), and is deleted exactly once here.
        unsafe { DeleteObject(self.0) };
    }
}

/// The object a `SelectObject` displaced. Puts it back on drop, so an early return between selecting
/// a bitmap and reading its bits cannot leave the DC holding a bitmap `GetDIBits` refuses to read.
struct Selected {
    dc: HDC,
    previous: HGDIOBJ,
    restored: bool,
}

impl Selected {
    fn new(dc: HDC, previous: HGDIOBJ) -> Self {
        Self {
            dc,
            previous,
            restored: false,
        }
    }

    /// Select the previous object back now, and report whether that succeeded. A failed restore
    /// leaves the DC unusable for `GetDIBits`, so the caller must skip the read.
    fn restore(&mut self) -> bool {
        if self.restored {
            return true;
        }
        // SAFETY: `dc` is a live DC and `previous` is the object handle its own `SelectObject` call
        // returned, so selecting it back is the documented deselect; no caller pointer is involved.
        let restored = unsafe { SelectObject(self.dc, self.previous) };
        self.restored = true;
        restored != 0 && restored != HGDI_ERROR
    }
}

impl Drop for Selected {
    fn drop(&mut self) {
        // Best effort: a restore that failed during the body has already been reported through
        // `restore`'s return value, and a second attempt would change nothing.
        let _ = self.restore();
    }
}

/// Blit the validated physical `rect` off the screen into a top-down 32-bpp BGRA buffer, and return
/// the buffer with its checked `u32` dimensions.
///
/// The GDI handles are owned by the guards above and released in reverse declaration order —
/// restore the DC, delete the bitmap, delete the memory DC, release the screen DC — which is the
/// order GDI requires (HW-F13). An early `?` cannot leak a handle.
///
/// # Errors
///
/// Returns [`WinError`] naming the Win32 call that failed, and [`WinError::WindowNotFound`] when the
/// rect's dimensions cannot be represented as positive `usize`/`u32` lengths.
fn blit_screen_bgra(rect: Rect) -> Result<(Vec<u8>, u32, u32), WinError> {
    let (mut pixels, mut info, width_u32, height_u32) = capture_buffer(rect)?;

    // SAFETY: `GetDC(NULL)` (a null `HWND` means the screen) writes nothing through a caller pointer
    // and keeps no reference; it returns the screen DC or a null handle, which is checked here.
    let screen_dc = ScreenDc(unsafe { GetDC(0) });
    if screen_dc.0 == 0 {
        return Err(WinError::win32("GetDC", last_error()));
    }

    // SAFETY: `screen_dc` is a live DC (`CreateCompatibleDC` only reads it and keeps no reference);
    // a failure returns null, checked here.
    let mem_dc = MemDc(unsafe { CreateCompatibleDC(screen_dc.0) });
    if mem_dc.0 == 0 {
        return Err(WinError::win32("CreateCompatibleDC", last_error()));
    }

    // SAFETY: `screen_dc` is live and the width/height are positive checked lengths (the client and
    // region validators reject zero/negative sizes), which is all `CreateCompatibleBitmap` needs; it
    // keeps no reference and returns null on failure, checked here.
    let bitmap = GdiBitmap(unsafe { CreateCompatibleBitmap(screen_dc.0, rect.w, rect.h) });
    if bitmap.0 == 0 {
        return Err(WinError::win32("CreateCompatibleBitmap", last_error()));
    }

    // SAFETY: `mem_dc` is a live memory DC and `bitmap` is a live compatible bitmap, which is the
    // documented use; `SelectObject` writes nothing through a caller pointer and returns the object
    // it replaced, which is the handle needed to deselect.
    let previous = unsafe { SelectObject(mem_dc.0, bitmap.0) };
    if previous == 0 || previous == HGDI_ERROR {
        return Err(WinError::win32("SelectObject", last_error()));
    }
    let mut selected = Selected::new(mem_dc.0, previous);

    // SAFETY: both DCs are live and compatible (the bitmap was made for `screen_dc`), the bitmap is
    // selected into `mem_dc`, and the destination/source rectangles are the validated physical
    // client area; `BitBlt` writes only into `mem_dc`'s selected bitmap.
    let copied = unsafe {
        BitBlt(
            mem_dc.0,
            0,
            0,
            rect.w,
            rect.h,
            screen_dc.0,
            rect.x,
            rect.y,
            SRCCOPY | CAPTUREBLT,
        )
    };
    let bitblt_error = (copied == 0).then(last_error);

    // GetDIBits requires that "the bitmap identified by the hbm parameter must not be selected into
    // a device context when the application calls this function" (Microsoft Learn, GetDIBits
    // Remarks), so the previous object is selected back BEFORE the bits are requested. If that
    // deselect fails the DC state is unusable for GetDIBits, so the read is skipped rather than
    // issued against the violated contract.
    let restored = selected.restore();
    let select_error = (!restored).then(last_error);

    let lines = if bitblt_error.is_none() && select_error.is_none() {
        // SAFETY: the bitmap is now deselected and `mem_dc` is the compatible DC, which is
        // GetDIBits' documented precondition; `pixels` is a live `width * height * 4`-byte buffer,
        // exactly the size of a top-down 32-bpp `BI_RGB` bitmap of `width_u32` x `height_u32` (the
        // scan lines are DWORD-aligned and already 4 bytes wide), and `info` is a live, correctly
        // aligned `BITMAPINFO` describing that format. The API writes only into those two and
        // copies at most `height_u32` scan lines.
        unsafe {
            GetDIBits(
                mem_dc.0,
                bitmap.0,
                0,
                height_u32,
                pixels.as_mut_ptr().cast::<c_void>(),
                &raw mut info,
                DIB_RGB_COLORS,
            )
        }
    } else {
        0
    };
    let dibits_error =
        (lines == 0 && bitblt_error.is_none() && select_error.is_none()).then(last_error);

    if let Some(error) = bitblt_error {
        return Err(WinError::win32("BitBlt", error));
    }
    if let Some(error) = select_error {
        return Err(WinError::win32("SelectObject", error));
    }
    if let Some(error) = dibits_error {
        return Err(WinError::win32("GetDIBits", error));
    }

    Ok((pixels, width_u32, height_u32))
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
