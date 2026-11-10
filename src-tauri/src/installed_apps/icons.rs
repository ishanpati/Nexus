use super::CatalogCache;
use serde::Serialize;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AppIcon {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Default)]
pub struct IconCache(Mutex<(u64, BTreeMap<String, Option<AppIcon>>)>);
impl IconCache {
    fn get(
        &self,
        generation: u64,
        id: String,
        extract: impl FnOnce() -> Option<AppIcon>,
    ) -> Result<Option<AppIcon>, String> {
        let mut cache = self
            .0
            .lock()
            .map_err(|_| "The icon cache is unavailable.")?;
        // Negative results are cached too. A catalog refresh permits one retry.
        if generation < cache.0 {
            return Ok(None);
        }
        if generation > cache.0 {
            *cache = (generation, BTreeMap::new());
        }
        if let Some(icon) = cache.1.get(&id) {
            return Ok(icon.clone());
        }
        let icon = extract();
        cache.1.insert(id, icon.clone());
        Ok(icon)
    }
}

#[tauri::command]
pub async fn get_installed_app_icon(
    app_id: String,
    cache: tauri::State<'_, Arc<CatalogCache>>,
    icons: tauri::State<'_, Arc<IconCache>>,
) -> Result<Option<AppIcon>, String> {
    let (cache, icons) = (cache.inner().clone(), icons.inner().clone());
    tauri::async_runtime::spawn_blocking(move || {
        let (generation, app) = cache.lookup(&app_id)?;
        icons.get(generation, app_id, || {
            #[cfg(windows)]
            {
                super::native::on_sta(move || super::native::icon(&app)).ok()
            }
            #[cfg(not(windows))]
            {
                let _ = app;
                None
            }
        })
    })
    .await
    .map_err(|e| format!("Could not load the application icon: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn caches_pixels_and_missing_icons_until_refresh() {
        let cache = IconCache::default();
        let icon = AppIcon {
            width: 1,
            height: 1,
            rgba: vec![1, 2, 3, 255],
        };
        assert_eq!(
            cache.get(1, "a".into(), || Some(icon.clone())).unwrap(),
            Some(icon.clone())
        );
        assert_eq!(
            cache.get(1, "a".into(), || panic!("cached")).unwrap(),
            Some(icon)
        );
        assert_eq!(cache.get(1, "missing".into(), || None).unwrap(), None);
        assert_eq!(
            cache
                .get(1, "missing".into(), || panic!("negative cached"))
                .unwrap(),
            None
        );
        assert_eq!(cache.get(2, "a".into(), || None).unwrap(), None);
        assert_eq!(
            cache
                .get(1, "a".into(), || panic!("stale generation"))
                .unwrap(),
            None
        );
    }
}
