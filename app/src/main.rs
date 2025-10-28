#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::Result;
use log::info;
use slint::LogicalPosition;

slint::include_modules!();

fn main() -> Result<()> {
    pretty_env_logger::env_logger::builder()
        .format_timestamp(None)
        .filter_level(log::LevelFilter::Debug)
        .init();
    info!("Starting Obsidian Installer");

    let ui = App::new()?;

    // Embed and decode the icon
    let icon_bytes = include_bytes!("../res/icon.ico");
    if let Ok(img) = image::load_from_memory(icon_bytes) {
        let rgba = img.to_rgba8();
        let width = rgba.width();
        let height = rgba.height();
        let pixel_buffer = slint::SharedPixelBuffer::clone_from_slice(rgba.as_raw(), width, height);
        let icon = slint::Image::from_rgba8(pixel_buffer);

        // Set the property to use in the UI
        ui.set_app_icon(icon);
    }

    // Center the window on the screen
    #[cfg(target_os = "windows")]
    {
        let window = ui.window();

        // Get screen dimensions (platform-specific)
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
        let center_x = (screen_width - 1280f32) / 2.0;
        let center_y = (screen_height - 720f32) / 2.0;

        window.set_position(LogicalPosition::new(center_x, center_y));
    }

    ui.on_request_exit_app(move || {
        std::process::exit(0);
    });

    let ui_handle_maximize = ui.as_weak();
    ui.on_toggle_maximize_window(move || {
        if let Some(ui) = ui_handle_maximize.upgrade() {
            let window = ui.window();
            window.set_maximized(!window.is_maximized());
        }
    });

    let ui_handle_minimize = ui.as_weak();
    ui.on_minimize_window(move || {
        if let Some(ui) = ui_handle_minimize.upgrade() {
            let window = ui.window();
            window.set_minimized(true);
        }
    });

    let ui_handle_drag = ui.as_weak();
    ui.on_drag_window(move |delta_x, delta_y| {
        if let Some(ui) = ui_handle_drag.upgrade() {
            let window = ui.window();
            let logical_pos = window.position().to_logical(window.scale_factor());
            window.set_position(LogicalPosition::new(
                logical_pos.x + delta_x,
                logical_pos.y + delta_y,
            ));
        }
    });

    ui.run()?;
    Ok(())
}
