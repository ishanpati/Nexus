//! Discovery, ID-only launching and local icons. Only backend catalog entries
//! may supply launch targets; frontend search text is never execution authority.
pub mod icons;
pub mod launching;
mod model;
#[cfg(windows)]
mod native;
#[cfg(windows)]
mod windows;

pub use model::*;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct CatalogCache(Mutex<Option<Catalog>>);

impl CatalogCache {
    pub fn lookup(&self, id: &str) -> Result<(u64, InstalledApp), String> {
        if id.trim().is_empty() || id.len() > 131072 || id.contains('\0') {
            return Err("Invalid application ID. Select an application from the results.".into());
        }
        let cached = self
            .0
            .lock()
            .map_err(|_| "The application catalog is unavailable. Restart NEXUS.")?;
        let catalog = cached.as_ref().ok_or(
            "The application catalog is not loaded yet. Try again after loading completes.",
        )?;
        let app = catalog
            .apps
            .iter()
            .find(|app| app.id == id)
            .cloned()
            .ok_or(
                "This application is no longer in the catalog. Refresh apps and select it again.",
            )?;
        Ok((catalog.generation, app))
    }
    pub fn list(&self) -> Result<Catalog, String> {
        self.load(false, discover)
    }

    pub fn refresh(&self) -> Result<Catalog, String> {
        self.load(true, discover)
    }

    fn load(
        &self,
        refresh: bool,
        scan: impl FnOnce() -> Result<Catalog, String>,
    ) -> Result<Catalog, String> {
        // Serialize refreshes and coalesce simultaneous first requests. No COM
        // objects are retained; only owned, serializable data enters the cache.
        let mut cached = self.0.lock().map_err(|_| {
            "The application catalog cache is unavailable. Restart NEXUS.".to_string()
        })?;
        if !refresh {
            if let Some(catalog) = cached.as_ref() {
                return Ok(catalog.clone());
            }
        }
        let mut next = scan()?; // A failed refresh preserves the previous snapshot.
        next.generation = cached.as_ref().map_or(1, |old| old.generation + 1);
        *cached = Some(next.clone());
        Ok(next)
    }
}

fn discover() -> Result<Catalog, String> {
    #[cfg(windows)]
    {
        // A fresh STA avoids COM apartment conflicts on Tauri's shared worker pool.
        std::thread::Builder::new()
            .name("nexus-app-discovery".into())
            .spawn(windows::scan)
            .map_err(|e| format!("Could not start application discovery: {e}"))?
            .join()
            .map_err(|_| {
                "Application discovery stopped unexpectedly. Try refreshing.".to_string()
            })?
    }
    #[cfg(not(windows))]
    Err("Installed-application discovery is supported on Windows only.".into())
}

async fn request(cache: Arc<CatalogCache>, refresh: bool) -> Result<Catalog, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if refresh {
            cache.refresh()
        } else {
            cache.list()
        }
    })
    .await
    .map_err(|e| format!("Could not read the application catalog: {e}"))?
}

#[tauri::command]
pub async fn list_installed_apps(
    cache: tauri::State<'_, Arc<CatalogCache>>,
) -> Result<Catalog, String> {
    request(cache.inner().clone(), false).await
}

#[tauri::command]
pub async fn refresh_installed_apps(
    cache: tauri::State<'_, Arc<CatalogCache>>,
) -> Result<Catalog, String> {
    request(cache.inner().clone(), true).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_uses_only_exact_cached_ids_without_scanning() {
        let cache = CatalogCache::default();
        assert!(
            cache
                .lookup("shell:registered.app")
                .unwrap_err()
                .contains("not loaded")
        );
        let app = InstalledApp::new(
            "Registered app".into(),
            LaunchTarget::ShellApp {
                parsing_name: "registered.app".into(),
                app_user_model_id: None,
            },
            None,
            Source::AppsFolder,
        );
        cache
            .load(false, || {
                Ok(Catalog {
                    apps: vec![app.clone()],
                    ..Default::default()
                })
            })
            .unwrap();
        assert_eq!(cache.lookup(&app.id).unwrap(), (1, app));
        for id in [
            "Registered app",
            "registered.app",
            "shell:missing",
            "C:\\Windows\\notepad.exe",
            "cmd.exe /c calc",
            "SHELL:registered.app",
        ] {
            assert!(
                cache
                    .lookup(id)
                    .unwrap_err()
                    .contains("no longer in the catalog")
            );
        }
        for id in ["", " ", "shell:registered.app\0ignored"] {
            assert!(
                cache
                    .lookup(id)
                    .unwrap_err()
                    .contains("Invalid application ID")
            );
        }
        cache.load(true, || Ok(Catalog::default())).unwrap();
        assert!(cache.lookup("shell:registered.app").is_err());
    }

    #[test]
    fn cached_reads_do_not_scan_and_refresh_replaces_snapshot() {
        let cache = CatalogCache::default();
        let first = cache.load(false, || Ok(Catalog::default())).unwrap();
        assert_eq!(first.generation, 1);
        assert_eq!(
            cache.load(false, || panic!("must not rescan")).unwrap(),
            first
        );
        let next = cache
            .load(true, || {
                Ok(Catalog {
                    warnings: vec!["partial source".into()],
                    ..Default::default()
                })
            })
            .unwrap();
        assert_eq!(next.generation, 2);
        assert_eq!(cache.load(false, || panic!()).unwrap(), next);
    }

    #[test]
    fn failed_refresh_preserves_cache_and_failed_first_scan_can_retry() {
        let cache = CatalogCache::default();
        assert!(cache.load(false, || Err("unavailable".into())).is_err());
        let old = cache.load(false, || Ok(Catalog::default())).unwrap();
        assert!(cache.load(true, || Err("failed refresh".into())).is_err());
        assert_eq!(cache.load(false, || panic!()).unwrap(), old);
    }

    #[test]
    fn simultaneous_cold_reads_scan_once() {
        let cache = Arc::new(CatalogCache::default());
        let scans = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let workers: Vec<_> = (0..8)
            .map(|_| {
                let (cache, scans) = (cache.clone(), scans.clone());
                std::thread::spawn(move || {
                    cache
                        .load(false, || {
                            scans.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                            Ok(Catalog::default())
                        })
                        .unwrap()
                })
            })
            .collect();
        for worker in workers {
            assert_eq!(worker.join().unwrap().generation, 1);
        }
        assert_eq!(scans.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn successful_refresh_removes_stale_entries_including_an_empty_catalog() {
        let cache = CatalogCache::default();
        let app = InstalledApp::new(
            "Old app".into(),
            LaunchTarget::ShellApp {
                parsing_name: "old.app".into(),
                app_user_model_id: None,
            },
            None,
            Source::AppsFolder,
        );
        let first = cache
            .load(false, || {
                Ok(Catalog {
                    apps: vec![app],
                    ..Default::default()
                })
            })
            .unwrap();
        assert_eq!(first.apps.len(), 1);
        let refreshed = cache.load(true, || Ok(Catalog::default())).unwrap();
        assert!(refreshed.apps.is_empty());
        assert_eq!(cache.load(false, || panic!()).unwrap(), refreshed);
    }
}
