use anyhow::Result;
use log::{error, info};
use oim::{InstallationConfig, InstallationManager, State, StateProgress};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const GITHUB_REPO: &str = "Obsidian-Minecraft-Server-Portal/obsidian-server-panel";
const SERVICE_NAME: &str = "ObsidianServerPanel";
const SERVICE_DISPLAY_NAME: &str = "Obsidian Minecraft Server Panel";
const SERVICE_DESCRIPTION: &str = "Self-hosted Minecraft server management panel";

/// Installer state for managing installation progress
pub struct InstallerState {
    pub status: String,
    pub progress: f32,
    pub completed: bool,
    pub success: bool,
    pub message: String,
}

impl Default for InstallerState {
    fn default() -> Self {
        Self {
            status: "Preparing installation...".to_string(),
            progress: 0.0,
            completed: false,
            success: false,
            message: String::new(),
        }
    }
}

/// Performs the installation using the OIM library
///
/// # Arguments
/// * `install_path` - Where to install the application
/// * `install_as_service` - Whether to install as a Windows Service
/// * `state` - Shared state for tracking progress
pub async fn perform_installation(
    install_path: String,
    install_as_service: bool,
    state: Arc<Mutex<InstallerState>>,
) -> Result<()> {
    info!("Starting installation to: {}", install_path);

    // Create installation configuration
    let config = InstallationConfig::new(
        PathBuf::from(&install_path),
        GITHUB_REPO.to_string(),
        SERVICE_NAME.to_string(),
    )
    .service_display_name(SERVICE_DISPLAY_NAME.to_string())
    .service_description(SERVICE_DESCRIPTION.to_string())
    .working_directory(PathBuf::from(&install_path));

    // If not installing as service, we'll just download and extract
    // The service installation is handled separately by the manager

    // Create installation manager (returns InstallationManager, not Result)
    let mut manager = InstallationManager::new(config);

    // Subscribe to progress updates
    let mut progress_rx = manager.subscribe();

    // Clone state for the spawned task
    let state_clone = Arc::clone(&state);

    // Spawn a task to listen for progress updates
    tokio::spawn(async move {
        while let Ok(progress) = progress_rx.recv().await {
            update_progress_state(&state_clone, &progress);
        }
    });

    // Perform installation
    {
        let mut s = state.lock().unwrap();
        s.status = "Fetching latest release...".to_string();
        s.progress = 0.1;
    }

    match manager.install(false) {
        Ok(_) => {
            info!("Installation completed successfully");
            let mut s = state.lock().unwrap();
            s.status = "Installation complete!".to_string();
            s.progress = 1.0;
            s.completed = true;
            s.success = true;
            s.message = format!(
                "Obsidian Server Panel has been successfully installed to {}",
                install_path
            );

            if install_as_service {
                s.message.push_str("\nThe service has been installed and started.");
            }
        }
        Err(e) => {
            error!("Installation failed: {}", e);
            let mut s = state.lock().unwrap();
            s.status = "Installation failed".to_string();
            s.completed = true;
            s.success = false;
            s.message = format!("Installation failed: {}", e);
        }
    }

    Ok(())
}

/// Updates the installer state based on OIM progress
fn update_progress_state(state: &Arc<Mutex<InstallerState>>, progress: &StateProgress) {
    let mut s = state.lock().unwrap();

    match progress.state {
        State::Downloading => {
            s.status = "Downloading application files...".to_string();
            s.progress = 0.2 + (progress.progress * 0.4); // 20-60%
        }
        State::Extracting => {
            s.status = "Extracting files...".to_string();
            s.progress = 0.6 + (progress.progress * 0.2); // 60-80%
        }
        State::Installing => {
            s.status = "Installing service...".to_string();
            s.progress = 0.8 + (progress.progress * 0.15); // 80-95%
        }
        State::Updating => {
            s.status = "Updating...".to_string();
            s.progress = 0.5 + (progress.progress * 0.5);
        }
    }
}
