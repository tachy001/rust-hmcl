//! Native outer-window rounding. UI cards cannot round a borderless OS window.
#[derive(Default)]
pub struct Corners {
    #[cfg(target_os = "windows")]
    last: Option<(isize, i32, i32, u32, bool)>,
}
impl Corners {
    pub fn update(&mut self, frame: &eframe::Frame) {
        #[cfg(target_os = "windows")]
        {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let Ok(handle) = frame.window_handle() {
                if let RawWindowHandle::Win32(handle) = handle.as_raw() {
                    let hwnd = handle.hwnd.get();
                    if let Some(state) = windows::state(hwnd) {
                        if self.last != Some(state) && windows::apply(state) {
                            self.last = Some(state);
                        }
                    }
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        let _ = frame;
    }
}
#[cfg(target_os = "windows")]
mod windows {
    use std::ffi::c_void;
    type Handle = *mut c_void;
    #[repr(C)]
    #[derive(Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetWindowRect(hwnd: Handle, rect: *mut Rect) -> i32;
        fn GetDpiForWindow(hwnd: Handle) -> u32;
        fn IsZoomed(hwnd: Handle) -> i32;
        fn SetWindowRgn(hwnd: Handle, region: Handle, redraw: i32) -> i32;
    }
    #[link(name = "gdi32")]
    unsafe extern "system" {
        fn CreateRoundRectRgn(
            left: i32,
            top: i32,
            right: i32,
            bottom: i32,
            width: i32,
            height: i32,
        ) -> Handle;
        fn DeleteObject(object: Handle) -> i32;
    }
    pub(super) fn state(hwnd: isize) -> Option<(isize, i32, i32, u32, bool)> {
        let mut rect = Rect::default();
        // The handle is borrowed from eframe's live native window on its UI thread.
        unsafe {
            if GetWindowRect(hwnd as Handle, &mut rect) == 0 {
                return None;
            }
            Some((
                hwnd,
                rect.right - rect.left,
                rect.bottom - rect.top,
                GetDpiForWindow(hwnd as Handle).max(96),
                IsZoomed(hwnd as Handle) != 0,
            ))
        }
    }
    pub(super) fn apply(
        (hwnd, width, height, dpi, maximized): (isize, i32, i32, u32, bool),
    ) -> bool {
        if width <= 0 || height <= 0 {
            return false;
        }
        unsafe {
            if maximized {
                return SetWindowRgn(hwnd as Handle, std::ptr::null_mut(), 1) != 0;
            }
            let diameter = (24. * dpi as f64 / 96.).round() as i32;
            let region = CreateRoundRectRgn(0, 0, width + 1, height + 1, diameter, diameter);
            if region.is_null() {
                return false;
            }
            if SetWindowRgn(hwnd as Handle, region, 1) == 0 {
                DeleteObject(region);
                return false;
            }
            // SetWindowRgn transfers ownership to Windows on success.
            true
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[link(name = "user32")]
        unsafe extern "system" {
            fn CreateWindowExW(
                ex_style: u32,
                class: *const u16,
                title: *const u16,
                style: u32,
                x: i32,
                y: i32,
                w: i32,
                h: i32,
                parent: Handle,
                menu: Handle,
                instance: Handle,
                param: Handle,
            ) -> Handle;
            fn DestroyWindow(hwnd: Handle) -> i32;
            fn GetWindowRgn(hwnd: Handle, region: Handle) -> i32;
        }
        #[link(name = "gdi32")]
        unsafe extern "system" {
            fn CreateRectRgn(left: i32, top: i32, right: i32, bottom: i32) -> Handle;
            fn PtInRegion(region: Handle, x: i32, y: i32) -> i32;
        }
        #[test]
        fn hidden_native_window_has_rounded_corners_and_can_clear_region() {
            unsafe {
                let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
                let title: Vec<u16> = "Rust HMCL corner test\0".encode_utf16().collect();
                // WS_POPUP, without WS_VISIBLE: never displayed, activated or focused.
                let hwnd = CreateWindowExW(
                    0,
                    class.as_ptr(),
                    title.as_ptr(),
                    0x80000000,
                    0,
                    0,
                    400,
                    300,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                );
                assert!(!hwnd.is_null());
                let region = CreateRectRgn(0, 0, 0, 0);
                assert!(!region.is_null());
                assert!(apply((hwnd as isize, 400, 300, 96, false)));
                assert!(GetWindowRgn(hwnd, region) > 1);
                assert_eq!(PtInRegion(region, 0, 0), 0);
                assert_ne!(PtInRegion(region, 200, 150), 0);
                assert_ne!(PtInRegion(region, 200, 0), 0);
                assert!(apply((hwnd as isize, 700, 500, 144, false)));
                assert!(GetWindowRgn(hwnd, region) > 1);
                assert_ne!(PtInRegion(region, 650, 450), 0);
                assert_eq!(PtInRegion(region, 699, 0), 0);
                assert!(apply((hwnd as isize, 700, 500, 144, true)));
                assert_eq!(GetWindowRgn(hwnd, region), 0);
                DeleteObject(region);
                DestroyWindow(hwnd);
            }
        }
    }
}
