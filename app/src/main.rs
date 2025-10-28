#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod dialogs;
mod fonts;
mod handlers;
mod installer;
mod resources;
mod startup;
mod window;

use anyhow::Result;
use installer::{perform_installation, InstallerState};
use log::info;
use slint::ComponentHandle;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

slint::include_modules!();

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    pretty_env_logger::env_logger::builder()
        .format_timestamp(None)
        .filter_level(log::LevelFilter::Debug)
        .init();
    info!("Starting Obsidian Installer");

    // Load embedded fonts (WOFF2 fonts need conversion to TTF for Windows GDI)
    // See app/res/fonts/README.md for conversion instructions
    if let Err(e) = fonts::load_embedded_fonts() {
        log::warn!(
            "Failed to load embedded fonts: {}. Using system fallback.",
            e
        );
    }

    // Create the UI instance
    let ui = App::new()?;

    // Load and set application icon
    if let Some(icon) = resources::load_app_icon() {
        ui.set_app_icon(icon);
    }

    // Load Terms of Service
    let tos_content = load_tos_content();
    ui.set_tos_content(tos_content.into());

    // Center the window on screen
    let window = ui.window();
    window::center_window(window, 1280.0, 720.0);

    // Setup basic window handlers
    handlers::setup_handlers(&ui);

    // Setup file browser callback
    let ui_handle_browse = ui.as_weak();
    ui.on_browse_folder(move || {
        if let Some(ui) = ui_handle_browse.upgrade()
            && let Some(path) = dialogs::browse_folder()
        {
            ui.set_install_path(path.to_string_lossy().to_string().into());
        }
    });

    // Setup installation callback
    let ui_handle_install = ui.as_weak();
    ui.on_start_installation(move || {
        if let Some(ui) = ui_handle_install.upgrade() {
            let install_path = ui.get_install_path().to_string();
            let install_as_service = ui.get_install_as_service();
            let start_with_windows = ui.get_start_with_windows();

            // Create shared state
            let state = Arc::new(Mutex::new(InstallerState::default()));
            let state_clone = Arc::clone(&state);
            let ui_weak = ui.as_weak();

            // Spawn installation task
            tokio::spawn(async move {
                // Perform installation
                if let Err(e) = perform_installation(
                    install_path.clone(),
                    install_as_service,
                    state_clone.clone(),
                )
                .await
                {
                    log::error!("Installation error: {}", e);
                    let mut s = state_clone.lock().unwrap();
                    s.success = false;
                    s.completed = true;
                    s.message = format!("Installation failed: {}", e);
                }

                // Handle startup registry if requested
                if start_with_windows {
                    let exe_path = std::env::current_exe().unwrap_or_default();
                    if let Err(e) = startup::add_to_startup(&exe_path) {
                        log::error!("Failed to add to startup: {}", e);
                    }
                }

                // Wait a moment for final state update
                tokio::time::sleep(Duration::from_millis(500)).await;

                // Update UI to complete page
                if let Some(ui) = ui_weak.upgrade() {
                    let s = state_clone.lock().unwrap();
                    ui.set_install_success(s.success);
                    ui.set_complete_message(s.message.clone().into());
                    ui.set_current_page(Page::Complete);
                }
            });

            // Start progress monitoring
            let state_monitor = Arc::clone(&state);
            let ui_weak_monitor = ui.as_weak();

            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_millis(100)).await;

                    if let Some(ui) = ui_weak_monitor.upgrade() {
                        let s = state_monitor.lock().unwrap();
                        ui.set_install_status(s.status.clone().into());
                        ui.set_install_progress(s.progress);

                        if s.completed {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            });
        }
    });

    // Setup launch app callback
    let ui_handle_launch = ui.as_weak();
    ui.on_launch_app(move || {
        if let Some(ui) = ui_handle_launch.upgrade() {
            let install_path = ui.get_install_path().to_string();
            launch_application(&install_path);
        }
    });

    // Run the application
    ui.run()?;
    Ok(())
}

/// Loads the Terms of Service content from embedded file
fn load_tos_content() -> String {
    // Embed the TOS file at compile time - 100% standalone
    include_str!("../../terms-of-service.md").to_string()
}

/// Launches the installed application
fn launch_application(install_path: &str) {
    let install_dir = PathBuf::from(install_path);

    // Try to find and launch the executable
    if let Ok(entries) = fs::read_dir(&install_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("exe") {
                info!("Launching application: {}", path.display());

                #[cfg(target_os = "windows")]
                {
                    use std::process::Command;
                    if let Err(e) = Command::new(&path).spawn() {
                        log::error!("Failed to launch application: {}", e);
                    }
                }

                break;
            }
        }
    }
}
