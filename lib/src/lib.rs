use semver::Version;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use anyhow::{Context, Result};
use tokio::sync::broadcast;

#[cfg(target_os = "linux")]
mod nix;
#[cfg(target_os = "windows")]
mod win;

/// GitHub release information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub name: String,
    pub prerelease: bool,
    pub assets: Vec<GitHubAsset>,
}

/// GitHub release asset
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GitHubAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

/// Platform architecture information
#[derive(Debug, Clone, PartialEq)]
pub enum Architecture {
    WindowsX64,
    WindowsArm64,
    LinuxX64,
    LinuxArm64,
    MacOSX64,
    MacOSArm64,
}

impl Architecture {
    /// Detect current system architecture
    pub fn detect() -> Result<Self> {
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;

        match (os, arch) {
            ("windows", "x86_64") => Ok(Architecture::WindowsX64),
            ("windows", "aarch64") => Ok(Architecture::WindowsArm64),
            ("linux", "x86_64") => Ok(Architecture::LinuxX64),
            ("linux", "aarch64") => Ok(Architecture::LinuxArm64),
            ("macos", "x86_64") => Ok(Architecture::MacOSX64),
            ("macos", "aarch64") => Ok(Architecture::MacOSArm64),
            _ => anyhow::bail!("Unsupported platform: {} {}", os, arch),
        }
    }

    /// Get patterns to match against asset names
    pub fn asset_patterns(&self) -> Vec<&str> {
        match self {
            Architecture::WindowsX64 => vec!["windows", "win", "x64", "x86_64", "amd64"],
            Architecture::WindowsArm64 => vec!["windows", "win", "arm64", "aarch64"],
            Architecture::LinuxX64 => vec!["linux", "x64", "x86_64", "amd64"],
            Architecture::LinuxArm64 => vec!["linux", "arm64", "aarch64"],
            Architecture::MacOSX64 => vec!["macos", "darwin", "x64", "x86_64"],
            Architecture::MacOSArm64 => vec!["macos", "darwin", "arm64", "aarch64"],
        }
    }

    /// Check if this is a Windows platform
    pub fn is_windows(&self) -> bool {
        matches!(self, Architecture::WindowsX64 | Architecture::WindowsArm64)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[repr(u8)]
pub enum State{
    Downloading,
    Extracting,
    Installing,
    Updating
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StateProgress{
    pub state: State,
    /// The progress from 0.0 to 1.0
    pub progress: f32,
}

impl StateProgress {
    pub fn new(state: State, progress: f32) -> Self {
        Self { state, progress: progress.clamp(0.0, 1.0) }
    }
}

/// Configuration for the installation manager
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstallationConfig {
    /// Path where the application will be installed
    pub install_path: PathBuf,
    /// GitHub repository in format "owner/repo"
    pub github_repo: String,
    /// Name of the service
    pub service_name: String,
    /// Display name for the service (optional, defaults to service_name)
    pub service_display_name: Option<String>,
    /// Description of the service
    pub service_description: Option<String>,
    /// Custom binary name to look for (optional)
    pub binary_name: Option<String>,
    /// Custom registry path for Windows (optional, defaults to SOFTWARE\ObsidianInstallationManager)
    pub registry_path: Option<String>,
    /// Custom version file directory for Linux (optional, defaults to /var/lib/oim)
    pub version_file_dir: Option<String>,
    /// Working directory for the service (optional, defaults to install_path)
    pub working_directory: Option<PathBuf>,
}

impl InstallationConfig {
    /// Create a new configuration with required fields
    pub fn new(
        install_path: PathBuf,
        github_repo: String,
        service_name: String,
    ) -> Self {
        Self {
            install_path,
            github_repo,
            service_name,
            service_display_name: None,
            service_description: None,
            binary_name: None,
            registry_path: None,
            version_file_dir: None,
            working_directory: None,
        }
    }

    /// Set the service display name
    pub fn service_display_name(mut self, name: String) -> Self {
        self.service_display_name = Some(name);
        self
    }

    /// Set the service description
    pub fn service_description(mut self, description: String) -> Self {
        self.service_description = Some(description);
        self
    }

    /// Set a custom binary name to look for
    pub fn binary_name(mut self, name: String) -> Self {
        self.binary_name = Some(name);
        self
    }

    /// Set a custom registry path (Windows only)
    pub fn registry_path(mut self, path: String) -> Self {
        self.registry_path = Some(path);
        self
    }

    /// Set a custom version file directory (Linux only)
    pub fn version_file_dir(mut self, dir: String) -> Self {
        self.version_file_dir = Some(dir);
        self
    }

    /// Set a custom working directory for the service
    pub fn working_directory(mut self, dir: PathBuf) -> Self {
        self.working_directory = Some(dir);
        self
    }

    /// Get the service display name (returns service_name if not set)
    pub fn get_display_name(&self) -> &str {
        self.service_display_name.as_deref().unwrap_or(&self.service_name)
    }

    /// Get the service description (returns a default if not set)
    pub fn get_description(&self) -> String {
        self.service_description.clone().unwrap_or_else(|| {
            format!("{} Service", self.get_display_name())
        })
    }

    /// Get the working directory (returns install_path if not set)
    pub fn get_working_directory(&self) -> &PathBuf {
        self.working_directory.as_ref().unwrap_or(&self.install_path)
    }

    /// Get the registry path (Windows)
    pub fn get_registry_path(&self) -> &str {
        self.registry_path.as_deref().unwrap_or(r"SOFTWARE\ObsidianInstallationManager")
    }

    /// Get the version file directory (Linux)
    pub fn get_version_file_dir(&self) -> &str {
        self.version_file_dir.as_deref().unwrap_or("/var/lib/oim")
    }
}

#[derive(Debug, Clone, Serialize)]
/// Installation manager for handling application installations
pub struct InstallationManager {
    is_installed: bool,
    current_version: Option<Version>,
    latest_version: Option<Version>,
    config: InstallationConfig,
    #[serde(skip)]
    progress_tx: broadcast::Sender<StateProgress>,
}

impl InstallationManager {
    /// Create a new installation manager with configuration
    pub fn new(config: InstallationConfig) -> Self {
        let (tx, _) = broadcast::channel(100);
        Self {
            is_installed: false,
            current_version: None,
            latest_version: None,
            config,
            progress_tx: tx,
        }
    }

    /// Create a new installation manager with basic parameters
    pub fn with_defaults(
        install_path: PathBuf,
        github_repo: String,
        service_name: String,
    ) -> Self {
        Self::new(InstallationConfig::new(install_path, github_repo, service_name))
    }

    /// Get a reference to the configuration
    pub fn config(&self) -> &InstallationConfig {
        &self.config
    }

    /// Subscribe to progress updates
    pub fn subscribe(&self) -> broadcast::Receiver<StateProgress> {
        self.progress_tx.subscribe()
    }

    /// Broadcast progress update (internal helper)
    fn broadcast_progress(&self, state: State, progress: f32) {
        let _ = self.progress_tx.send(StateProgress::new(state, progress));
    }

    /// Check if the application is currently installed
    pub fn is_installed(&self) -> bool {
        self.is_installed
    }

    /// Get the current installed version
    pub fn current_version(&self) -> Option<&Version> {
        self.current_version.as_ref()
    }

    /// Get the latest available version
    pub fn latest_version(&self) -> Option<&Version> {
        self.latest_version.as_ref()
    }

    /// Fetch releases from GitHub
    pub fn fetch_releases(&self) -> Result<Vec<GitHubRelease>> {
        let url = format!(
            "https://api.github.com/repos/{}/releases",
            self.config.github_repo
        );

        let client = reqwest::blocking::Client::builder()
            .user_agent("obsidian-installation-manager")
            .build()?;

        let response = client
            .get(&url)
            .send()
            .context("Failed to fetch releases from GitHub")?;

        if !response.status().is_success() {
            anyhow::bail!("GitHub API returned status: {}", response.status());
        }

        let releases: Vec<GitHubRelease> = response
            .json()
            .context("Failed to parse GitHub releases")?;

        Ok(releases)
    }

    /// Get the latest release (excluding pre-releases by default)
    pub fn get_latest_release(&mut self, include_prerelease: bool) -> Result<GitHubRelease> {
        let releases = self.fetch_releases()?;

        let latest = releases
            .into_iter()
            .find(|r| include_prerelease || !r.prerelease)
            .context("No releases found")?;

        // Update latest version
        let version_str = latest.tag_name.trim_start_matches('v');
        self.latest_version = Some(Version::parse(version_str)?);

        Ok(latest)
    }

    /// Check for updates
    pub fn check_for_updates(&mut self, include_prerelease: bool) -> Result<bool> {
        let latest = self.get_latest_release(include_prerelease)?;
        let latest_version_str = latest.tag_name.trim_start_matches('v');
        let latest_version = Version::parse(latest_version_str)?;

        #[cfg(target_os = "windows")]
        {
            self.current_version = win::get_installed_version(&self.config)?;
        }

        #[cfg(target_os = "linux")]
        {
            self.current_version = nix::get_installed_version(&self.config)?;
        }

        self.is_installed = self.current_version.is_some();

        Ok(match &self.current_version {
            Some(current) => latest_version > *current,
            None => true, // No version installed, update available
        })
    }

    /// Select the appropriate asset for the current architecture
    pub fn select_asset(&self, release: &GitHubRelease) -> Result<GitHubAsset> {
        let arch = Architecture::detect()?;
        let patterns = arch.asset_patterns();

        // Try to find an asset that matches the architecture patterns
        for asset in &release.assets {
            let name_lower = asset.name.to_lowercase();

            // Count how many patterns match
            let match_count = patterns.iter()
                .filter(|&&p| name_lower.contains(p))
                .count();

            // If we match multiple patterns, it's likely the right asset
            if match_count >= 2 {
                return Ok(asset.clone());
            }
        }

        // Fallback: try to match at least one pattern
        for asset in &release.assets {
            let name_lower = asset.name.to_lowercase();
            if patterns.iter().any(|&p| name_lower.contains(p)) {
                return Ok(asset.clone());
            }
        }

        anyhow::bail!("No suitable asset found for architecture: {:?}", arch)
    }

    /// Download a release asset
    pub fn download_asset(&self, asset: &GitHubAsset, dest_path: &PathBuf) -> Result<()> {
        use std::io::Read;

        let client = reqwest::blocking::Client::builder()
            .user_agent("obsidian-installation-manager")
            .build()?;

        let mut response = client
            .get(&asset.browser_download_url)
            .send()
            .context("Failed to download asset")?;

        if !response.status().is_success() {
            anyhow::bail!("Download failed with status: {}", response.status());
        }

        let total_size = asset.size;
        let mut file = std::fs::File::create(dest_path)
            .context("Failed to create download file")?;

        let mut downloaded: u64 = 0;
        let mut buffer = [0u8; 8192];

        self.broadcast_progress(State::Downloading, 0.0);

        loop {
            let bytes_read = response.read(&mut buffer)
                .context("Failed to read from download stream")?;

            if bytes_read == 0 {
                break;
            }

            std::io::Write::write_all(&mut file, &buffer[..bytes_read])
                .context("Failed to write downloaded file")?;

            downloaded += bytes_read as u64;

            if total_size > 0 {
                let progress = downloaded as f32 / total_size as f32;
                self.broadcast_progress(State::Downloading, progress);
            }
        }

        self.broadcast_progress(State::Downloading, 1.0);
        Ok(())
    }

    /// Extract downloaded archive
    pub fn extract_archive(&self, archive_path: &PathBuf, extract_to: &PathBuf) -> Result<()> {
        self.broadcast_progress(State::Extracting, 0.0);
        std::fs::create_dir_all(extract_to)?;

        let file_name = archive_path
            .file_name()
            .and_then(|n| n.to_str())
            .context("Invalid archive path")?;

        if file_name.ends_with(".tar.gz") || file_name.ends_with(".tgz") {
            self.extract_tar_gz(archive_path, extract_to)?;
        } else if file_name.ends_with(".zip") {
            self.extract_zip(archive_path, extract_to)?;
        } else {
            anyhow::bail!("Unsupported archive format: {}", file_name);
        }

        self.broadcast_progress(State::Extracting, 1.0);
        Ok(())
    }

    fn extract_tar_gz(&self, archive_path: &PathBuf, extract_to: &PathBuf) -> Result<()> {
        let file = std::fs::File::open(archive_path)?;
        let decoder = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);
        archive.unpack(extract_to)?;
        Ok(())
    }

    fn extract_zip(&self, archive_path: &PathBuf, extract_to: &std::path::Path) -> Result<()> {
        let file = std::fs::File::open(archive_path)?;
        let mut archive = zip::ZipArchive::new(file)?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let outpath = match file.enclosed_name() {
                Some(path) => extract_to.join(path),
                None => continue,
            };

            if file.name().ends_with('/') {
                std::fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() && !p.exists() {
                    std::fs::create_dir_all(p)?;
                }
                let mut outfile = std::fs::File::create(&outpath)?;
                std::io::copy(&mut file, &mut outfile)?;
            }

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = file.unix_mode() {
                    std::fs::set_permissions(&outpath, std::fs::Permissions::from_mode(mode))?;
                }
            }
        }

        Ok(())
    }

    /// Install a release
    pub fn install(&mut self, include_prerelease: bool) -> Result<()> {
        let release = self.get_latest_release(include_prerelease)?;
        let asset = self.select_asset(&release)?;

        println!("Installing {} version {}...", self.config.service_name, release.tag_name);
        println!("Downloading {}...", asset.name);

        // Create temporary download directory
        let temp_dir = std::env::temp_dir().join(format!("oim-{}", self.config.service_name));
        std::fs::create_dir_all(&temp_dir)?;

        let download_path = temp_dir.join(&asset.name);
        self.download_asset(&asset, &download_path)?;

        println!("Extracting to {}...", self.config.install_path.display());
        self.extract_archive(&download_path, &self.config.install_path)?;

        // Platform-specific installation
        self.broadcast_progress(State::Installing, 0.0);

        #[cfg(target_os = "windows")]
        {
            win::install_service(&self.config, &release.tag_name)?;
        }

        #[cfg(target_os = "linux")]
        {
            nix::install_service(&self.config, &release.tag_name)?;
        }

        self.broadcast_progress(State::Installing, 1.0);

        // Update internal state
        let version_str = release.tag_name.trim_start_matches('v');
        self.current_version = Some(Version::parse(version_str)?);
        self.is_installed = true;

        // Cleanup
        std::fs::remove_file(download_path)?;

        println!("Installation complete!");
        Ok(())
    }

    /// Update an existing installation
    pub fn update(&mut self, include_prerelease: bool) -> Result<()> {
        if !self.is_installed {
            anyhow::bail!("No installation found. Use install() instead.");
        }

        let has_update = self.check_for_updates(include_prerelease)?;
        if !has_update {
            println!("Already up to date!");
            return Ok(());
        }

        println!(
            "Updating from {} to {}...",
            self.current_version.as_ref().unwrap(),
            self.latest_version.as_ref().unwrap()
        );

        self.broadcast_progress(State::Updating, 0.0);

        // Platform-specific service stop
        #[cfg(target_os = "windows")]
        {
            win::stop_service(&self.config)?;
        }

        #[cfg(target_os = "linux")]
        {
            nix::stop_service(&self.config)?;
        }

        self.broadcast_progress(State::Updating, 0.2);

        // Perform installation (which will overwrite existing files)
        self.install(include_prerelease)?;

        self.broadcast_progress(State::Updating, 0.8);

        // Platform-specific service start
        #[cfg(target_os = "windows")]
        {
            win::start_service(&self.config)?;
        }

        #[cfg(target_os = "linux")]
        {
            nix::start_service(&self.config)?;
        }

        self.broadcast_progress(State::Updating, 1.0);

        println!("Update complete!");
        Ok(())
    }

    /// Uninstall the application
    pub fn uninstall(&mut self) -> Result<()> {
        if !self.is_installed {
            anyhow::bail!("No installation found.");
        }

        println!("Uninstalling {}...", self.config.service_name);

        // Platform-specific service removal
        #[cfg(target_os = "windows")]
        {
            win::uninstall_service(&self.config)?;
        }

        #[cfg(target_os = "linux")]
        {
            nix::uninstall_service(&self.config)?;
        }

        // Remove installation directory
        if self.config.install_path.exists() {
            std::fs::remove_dir_all(&self.config.install_path)?;
        }

        self.is_installed = false;
        self.current_version = None;

        println!("Uninstall complete!");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_architecture_detect() {
        let arch = Architecture::detect();
        assert!(arch.is_ok());
    }

    #[test]
    fn test_architecture_patterns() {
        let arch = Architecture::WindowsX64;
        let patterns = arch.asset_patterns();
        assert!(patterns.contains(&"windows"));
        assert!(patterns.contains(&"x64"));
    }

    #[test]
    fn test_config_builder() {
        let config = InstallationConfig::new(
            PathBuf::from("/opt/myapp"),
            "owner/repo".to_string(),
            "myapp".to_string(),
        )
        .service_display_name("My Application".to_string())
        .service_description("A test application".to_string())
        .binary_name("myapp-bin".to_string());

        assert_eq!(config.get_display_name(), "My Application");
        assert_eq!(config.get_description(), "A test application");
        assert_eq!(config.binary_name, Some("myapp-bin".to_string()));
    }

    #[test]
    fn test_config_defaults() {
        let config = InstallationConfig::new(
            PathBuf::from("/opt/myapp"),
            "owner/repo".to_string(),
            "myapp".to_string(),
        );

        assert_eq!(config.get_display_name(), "myapp");
        assert_eq!(config.get_description(), "myapp Service");
        assert_eq!(config.get_working_directory(), &PathBuf::from("/opt/myapp"));
    }

    #[test]
    fn test_installation_manager_creation() {
        let config = InstallationConfig::new(
            PathBuf::from("/opt/myapp"),
            "owner/repo".to_string(),
            "myapp".to_string(),
        );

        let manager = InstallationManager::new(config);
        assert!(!manager.is_installed());
        assert!(manager.current_version().is_none());
        assert!(manager.latest_version().is_none());
    }

    #[test]
    fn test_installation_manager_with_defaults() {
        let manager = InstallationManager::with_defaults(
            PathBuf::from("/opt/myapp"),
            "owner/repo".to_string(),
            "myapp".to_string(),
        );

        assert_eq!(manager.config().service_name, "myapp");
        assert_eq!(manager.config().github_repo, "owner/repo");
    }

    #[test]
    fn test_select_asset() {
        let config = InstallationConfig::new(
            PathBuf::from("/opt/myapp"),
            "owner/repo".to_string(),
            "myapp".to_string(),
        );

        let manager = InstallationManager::new(config);

        let release = GitHubRelease {
            tag_name: "v1.0.0".to_string(),
            name: "Release 1.0.0".to_string(),
            prerelease: false,
            assets: vec![
                GitHubAsset {
                    name: "myapp-windows-x64.zip".to_string(),
                    browser_download_url: "https://example.com/myapp-windows-x64.zip".to_string(),
                    size: 1024,
                },
                GitHubAsset {
                    name: "myapp-linux-x64.tar.gz".to_string(),
                    browser_download_url: "https://example.com/myapp-linux-x64.tar.gz".to_string(),
                    size: 1024,
                },
            ],
        };

        let result = manager.select_asset(&release);
        assert!(result.is_ok());
        let asset = result.unwrap();

        // The selected asset should match the current platform
        if cfg!(target_os = "windows") {
            assert!(asset.name.contains("windows"));
        } else if cfg!(target_os = "linux") {
            assert!(asset.name.contains("linux"));
        }
    }
}
