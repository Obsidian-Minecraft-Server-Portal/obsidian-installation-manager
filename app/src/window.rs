use slint::{LogicalPosition, Window};

/// Centers the window on the screen
///
/// # Arguments
/// * `window` - The Slint window to center
/// * `width` - Window width in pixels
/// * `height` - Window height in pixels
#[cfg(target_os = "windows")]
pub fn center_window(window: &Window, width: f32, height: f32) {
    // Get screen dimensions using Windows API
    let (screen_width, screen_height) = unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{
            GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN,
        };
        (
            GetSystemMetrics(SM_CXSCREEN) as f32,
            GetSystemMetrics(SM_CYSCREEN) as f32,
        )
    };

    // Calculate center position
    let center_x = (screen_width - width) / 2.0;
    let center_y = (screen_height - height) / 2.0;

    window.set_position(LogicalPosition::new(center_x, center_y));
}

/// Centers the window on the screen (non-Windows platforms)
///
/// This is a placeholder implementation for non-Windows platforms.
/// On Linux/macOS, window positioning may be handled by the window manager.
#[cfg(not(target_os = "windows"))]
pub fn center_window(_window: &Window, _width: f32, _height: f32) {
    // On Linux/macOS, window positioning is typically managed by the window manager
    // This is a no-op for now
}
