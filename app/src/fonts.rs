use log::{info, warn};
use ttf_parser::{Face, name_id};

/// Embedded Minecraft fonts (TTF format - fully supported)
const MINECRAFT_BODY_FONT: &[u8] = include_bytes!("../res/fonts/Minecraft-Seven_v2.ttf");
const MINECRAFT_HEADER_FONT: &[u8] = include_bytes!("../res/fonts/Minecraft-Tenv2.ttf");

/// Extract font family name from TTF data
fn get_font_family_name(font_data: &[u8]) -> Option<String> {
    let face = Face::parse(font_data, 0).ok()?;

    // Try to get the font family name (name ID 1)
    for name in face.names() {
        if name.name_id == name_id::FAMILY {
            if let Some(family_name) = name.to_string() {
                return Some(family_name);
            }
        }
    }
    None
}

/// Loads and registers embedded fonts with the system
///
/// On Windows, this uses AddFontMemResourceEx to register fonts for this process only.
/// The fonts are automatically unregistered when the application exits.
#[cfg(target_os = "windows")]
pub fn load_embedded_fonts() -> Result<(), Box<dyn std::error::Error>> {
    use windows::Win32::Graphics::Gdi::AddFontMemResourceEx;

    info!("Loading embedded Minecraft fonts (TTF format)...");

    // Extract and log the actual font family names
    if let Some(body_font_name) = get_font_family_name(MINECRAFT_BODY_FONT) {
        info!("Body font family name: '{}'", body_font_name);
    } else {
        warn!("Could not extract body font family name");
    }

    if let Some(header_font_name) = get_font_family_name(MINECRAFT_HEADER_FONT) {
        info!("Header font family name: '{}'", header_font_name);
    } else {
        warn!("Could not extract header font family name");
    }

    unsafe {
        let mut num_fonts: u32 = 0;

        // Try to register body font (Minecraft-Seven)
        let body_font_handle = AddFontMemResourceEx(
            MINECRAFT_BODY_FONT.as_ptr() as *const _,
            MINECRAFT_BODY_FONT.len() as u32,
            None,
            &mut num_fonts,
        );

        if body_font_handle.is_invalid() {
            warn!("Failed to register Minecraft body font - may need TTF format");
        } else {
            info!("Successfully registered Minecraft body font ({} fonts registered)", num_fonts);
        }

        // Try to register header font (Minecraft-Ten)
        let header_font_handle = AddFontMemResourceEx(
            MINECRAFT_HEADER_FONT.as_ptr() as *const _,
            MINECRAFT_HEADER_FONT.len() as u32,
            None,
            &mut num_fonts,
        );

        if header_font_handle.is_invalid() {
            warn!("Failed to register Minecraft header font - may need TTF format");
        } else {
            info!("Successfully registered Minecraft header font ({} fonts registered)", num_fonts);
        }

        // Note: Fonts are automatically unregistered when the process exits
        // We don't explicitly call RemoveFontMemResourceEx here
    }

    Ok(())
}

/// Loads embedded fonts (non-Windows fallback)
///
/// On non-Windows platforms, this is a no-op as font registration
/// mechanisms vary by platform. System fonts will be used as fallback.
#[cfg(not(target_os = "windows"))]
pub fn load_embedded_fonts() -> Result<(), Box<dyn std::error::Error>> {
    info!("Font loading not implemented for this platform - using system fonts");
    Ok(())
}

/// Returns the font family names to use in the UI
///
/// These are extracted from the TTF font metadata at runtime.
/// - Body font: "Minecraft Seven v2" (for main UI text)
/// - Header font: "Minecraft Ten v2" (for larger headers)
#[allow(dead_code)]
pub fn get_font_families() -> (String, String) {
    (
        get_font_family_name(MINECRAFT_BODY_FONT).unwrap_or_else(|| "Minecraft Seven v2".to_string()),
        get_font_family_name(MINECRAFT_HEADER_FONT).unwrap_or_else(|| "Minecraft Ten v2".to_string()),
    )
}
