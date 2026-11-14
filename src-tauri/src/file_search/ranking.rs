use super::FileResult;
use std::collections::{BTreeMap, HashSet};

#[cfg(test)]
#[path = "ranking_tests.rs"]
mod tests;

#[derive(Default)]
pub(super) struct Context {
    pub desktop: Option<String>,
    pub documents: Option<String>,
    pub downloads: Option<String>,
    pub projects: Vec<String>,
}

pub(super) fn normalized(path: &str) -> String {
    let path = path.replace('/', "\\");
    let path = path
        .strip_prefix(r"\\?\UNC\")
        .map(|tail| format!(r"\\{tail}"))
        .unwrap_or_else(|| path.strip_prefix(r"\\?\").unwrap_or(&path).to_string());
    path.trim_end_matches('\\').to_lowercase()
}

pub(super) fn path_query(query: &str) -> bool {
    query.contains(['\\', '/'])
}

impl Context {
    pub fn system() -> Self {
        let mut context = Self::default();
        #[cfg(windows)]
        {
            use windows::Win32::{System::Com::CoTaskMemFree, UI::Shell::*};
            let folder = |id| unsafe {
                let value = SHGetKnownFolderPath(id, KF_FLAG_DONT_VERIFY, None).ok()?;
                let text = value.to_string().ok();
                CoTaskMemFree(Some(value.0.cast()));
                text.map(|text| normalized(&text))
            };
            context.desktop = folder(&FOLDERID_Desktop);
            context.documents = folder(&FOLDERID_Documents);
            context.downloads = folder(&FOLDERID_Downloads);
        }
        // Environment configuration is trusted configuration, never query syntax.
        if let Some(paths) = std::env::var_os("NEXUS_PROJECT_DIRS") {
            context.projects = std::env::split_paths(&paths)
                .filter(|p| p.is_absolute())
                .take(32)
                .filter_map(|p| p.to_str().map(normalized))
                .collect();
        }
        context
    }
}

fn tier(item: &FileResult, query: &str) -> Option<u8> {
    let name = item.filename.to_lowercase();
    let stem = if item.is_directory {
        name.as_str()
    } else {
        name.rsplit_once('.')
            .filter(|(stem, _)| !stem.is_empty())
            .map_or(name.as_str(), |(stem, _)| stem)
    };
    if name == query || stem == query {
        return Some(0);
    }
    if name.starts_with(query) {
        return Some(1);
    }
    if name.match_indices(query).any(|(index, _)| {
        index > 0
            && name[..index]
                .chars()
                .next_back()
                .is_some_and(|c| !c.is_alphanumeric())
    }) {
        return Some(2);
    }
    if name.contains(query) {
        return Some(3);
    }
    // No fuzzy source: do not fabricate fuzzy matches from an unrelated pool.
    if normalized(&item.full_path).contains(query) {
        Some(5)
    } else {
        None
    }
}

fn context_score(path: &str, context: &Context, opened: &HashSet<String>) -> i32 {
    let parent = path
        .rsplit_once('\\')
        .map(|(parent, _)| parent)
        .unwrap_or("");
    let mut score = 0;
    if context.desktop.as_deref() == Some(parent) {
        score += 12;
    }
    if context.documents.as_deref() == Some(parent) || context.downloads.as_deref() == Some(parent)
    {
        score += 8;
    }
    if context.projects.iter().any(|root| {
        path == root
            || path
                .strip_prefix(root)
                .is_some_and(|tail| tail.starts_with('\\'))
    }) {
        score += 4;
    }
    if opened.contains(path) {
        score += 2;
    }
    // Components, not substring guesses ("attempt" and "distillery" are ordinary names).
    for part in parent.split('\\') {
        score -= match part {
            "temp" | "tmp" | "cache" | "caches" | ".cache" | "crashdumps" | "ipch" => 12,
            "node_modules" | ".git" => 16,
            "dist" | "target" | "build" | "out" => 8,
            _ => 0,
        };
    }
    score.max(-48)
}

pub(super) fn deduplicate(items: Vec<FileResult>) -> Vec<FileResult> {
    let mut unique = BTreeMap::<String, FileResult>::new();
    for item in items {
        let key = normalized(&item.full_path);
        match unique.get_mut(&key) {
            Some(previous) if item.full_path < previous.full_path => *previous = item,
            None => {
                unique.insert(key, item);
            }
            _ => {}
        }
    }
    unique.into_values().collect()
}

pub(super) fn rank(
    items: Vec<FileResult>,
    query: &str,
    context: &Context,
    opened: &[String],
    limit: usize,
) -> Vec<FileResult> {
    let query = query.trim().replace('/', "\\").to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    let opened = opened.iter().map(|path| normalized(path)).collect();
    let mut scored: Vec<_> = deduplicate(items)
        .into_iter()
        .filter_map(|item| {
            let tier = tier(&item, &query)?;
            let path = normalized(&item.full_path);
            let score = context_score(&path, context, &opened);
            Some((
                (
                    tier,
                    std::cmp::Reverse(score),
                    item.filename.to_lowercase(),
                    path,
                    item.full_path.clone(),
                ),
                item,
            ))
        })
        .collect();
    scored.sort_by(|a, b| a.0.cmp(&b.0));
    scored
        .into_iter()
        .take(limit)
        .map(|(_, item)| item)
        .collect()
}
