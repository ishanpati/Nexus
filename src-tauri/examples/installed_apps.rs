//! Read-only diagnostic using the exact catalog/cache used by the Tauri commands.
fn main() -> Result<(), String> {
    let cache = nexus_lib::installed_apps::CatalogCache::default();
    let first = cache.list()?;
    let cached = cache.list()?;
    if first != cached {
        return Err("Cached catalog unexpectedly changed".into());
    }
    let refreshed = cache.refresh()?;
    if refreshed.generation != first.generation + 1 {
        return Err("Refresh did not advance the catalog generation".into());
    }
    eprintln!(
        "Verified initial discovery, identical cached read, and explicit refresh: {} applications, {} diagnostics.",
        refreshed.apps.len(),
        refreshed.warnings.len()
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&refreshed).map_err(|e| e.to_string())?
    );
    Ok(())
}
