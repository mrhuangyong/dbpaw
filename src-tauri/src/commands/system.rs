use font_kit::source::SystemSource;

#[tauri::command]
pub async fn list_system_fonts() -> Result<Vec<String>, String> {
    let source = SystemSource::new();
    let fonts = source.all_fonts().map_err(|e| e.to_string())?;

    let mut families: Vec<String> = Vec::new();

    for font_handle in fonts {
        if let Ok(font) = font_handle.load() {
            let family = font.family_name();
            if !family.is_empty() {
                families.push(family);
            }
        }
    }

    families.sort();
    families.dedup();
    Ok(families)
}
