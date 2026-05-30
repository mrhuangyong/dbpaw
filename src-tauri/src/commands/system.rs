use fontique::Collection;
use std::sync::OnceLock;

static FONT_FAMILIES: OnceLock<Vec<String>> = OnceLock::new();

#[tauri::command]
pub async fn list_system_fonts() -> Result<Vec<String>, String> {
    let families = FONT_FAMILIES.get_or_init(|| {
        let mut collection = Collection::default();
        let mut families: Vec<String> = collection
            .family_names()
            .map(|s| s.to_string())
            .collect();
        families.sort();
        families.dedup();
        families
    });
    Ok(families.clone())
}
