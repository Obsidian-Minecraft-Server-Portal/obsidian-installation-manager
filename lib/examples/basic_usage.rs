/// Basic usage example demonstrating simple installation
///
/// This example shows how to use the installation manager with default settings
/// to install an application from a GitHub repository.

use oim::{InstallationManager, InstallationConfig};
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    println!("Basic Installation Manager Example");
    println!("===================================\n");

    // Create a basic configuration
    let config = InstallationConfig::new(
        PathBuf::from("/opt/myapp"),           // Installation path
        "owner/repository".to_string(),         // GitHub repo (format: owner/repo)
        "myapp".to_string(),                    // Service name
    );

    // Create the installation manager
    let manager = InstallationManager::new(config);

    println!("Configuration:");
    println!("  Service Name: {}", manager.config().service_name);
    println!("  GitHub Repo: {}", manager.config().github_repo);
    println!("  Install Path: {}", manager.config().install_path.display());
    println!();

    // Check if already installed
    println!("Checking installation status...");
    if manager.is_installed() {
        println!("Application is already installed!");
        if let Some(version) = manager.current_version() {
            println!("Current version: {}", version);
        }
    } else {
        println!("Application is not installed.");
    }

    println!("\nTo install:");
    println!("  manager.install(false)?;  // false = exclude pre-releases");

    println!("\nTo check for updates:");
    println!("  if manager.check_for_updates(false)? {{");
    println!("      manager.update(false)?;");
    println!("  }}");

    println!("\nTo uninstall:");
    println!("  manager.uninstall()?;");

    Ok(())
}
