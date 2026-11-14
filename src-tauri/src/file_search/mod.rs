//! Local Everything ES integration. Search text is data, never an executable or
//! command line. No filesystem scanning, result launching, or UI integration.
mod output;
mod process;
mod ranking;
mod retrieval;

use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

pub const MAX_RESULTS: u32 = 100;
const MAX_QUERY_BYTES: usize = 1024;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidInput,
    EsNotFound,
    EverythingUnavailable,
    Timeout,
    ProcessFailed,
    MalformedOutput,
    Busy,
    UnsupportedPlatform,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchError {
    pub code: ErrorCode,
    pub message: String,
}
impl SearchError {
    fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
impl std::fmt::Display for SearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
impl std::error::Error for SearchError {}
type Result<T> = std::result::Result<T, SearchError>;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FileResult {
    pub full_path: String,
    pub filename: String,
    /// Without the leading dot; empty for directories and extensionless files.
    pub extension: String,
    pub is_directory: bool,
}

/// Set NEXUS_ES_PATH to an absolute es.exe path before starting NEXUS, or put
/// es.exe on PATH. This configuration never comes from a search IPC request.
#[derive(Default)]
pub struct FileSearch {
    active: AtomicBool,
    context: OnceLock<ranking::Context>,
}
impl FileSearch {
    pub fn search(&self, query: &str, limit: u32) -> Result<Vec<FileResult>> {
        self.search_with_opened(query, limit, &[])
    }

    pub fn search_with_opened(
        &self,
        query: &str,
        limit: u32,
        opened: &[String],
    ) -> Result<Vec<FileResult>> {
        arguments(query, limit)?;
        validate_opened(opened)?;
        self.active
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| {
                SearchError::new(
                    ErrorCode::Busy,
                    "A file search is already running. Try again shortly.",
                )
            })?;
        struct Guard<'a>(&'a AtomicBool);
        impl Drop for Guard<'_> {
            fn drop(&mut self) {
                self.0.store(false, Ordering::SeqCst);
            }
        }
        let _guard = Guard(&self.active);
        if !cfg!(windows) {
            return Err(SearchError::new(
                ErrorCode::UnsupportedPlatform,
                "Everything file search requires Windows.",
            ));
        }
        let executable = locate(
            std::env::var_os("NEXUS_ES_PATH").map(PathBuf::from),
            std::env::var_os("PATH"),
        )?;
        let started = Instant::now();
        let candidates = retrieval::collect(query, limit as usize, |pass, directory| {
            let remaining = Duration::from_secs(5).checked_sub(started.elapsed()).filter(|duration| !duration.is_zero())
                .ok_or_else(|| SearchError::new(ErrorCode::Timeout, "Everything did not finish searching within five seconds. Retry when its index is ready."))?;
            let args = retrieval::pass_arguments(query, pass, directory)?;
            let response = process::run(&executable, &args, remaining, 4 * 1024 * 1024)?;
            check_exit(response.code)?;
            let results = output::parse(&response.stdout, retrieval::POOL_LIMIT as usize)?;
            if (response.code == Some(9) && !results.is_empty())
                || results.iter().any(|item| item.is_directory != directory)
            {
                return Err(SearchError::new(
                    ErrorCode::MalformedOutput,
                    "ES returned inconsistent result types or no-match status.",
                ));
            }
            Ok(results)
        })?;
        Ok(ranking::rank(
            candidates,
            query,
            self.context.get_or_init(ranking::Context::system),
            opened,
            limit as usize,
        ))
    }
}

fn validate_opened(opened: &[String]) -> Result<()> {
    if opened.len() > 256
        || opened
            .iter()
            .any(|path| path.len() > 4096 || path.chars().any(char::is_control))
    {
        Err(SearchError::new(
            ErrorCode::InvalidInput,
            "File history exceeds its allowed size.",
        ))
    } else {
        Ok(())
    }
}

fn locate(configured: Option<PathBuf>, path: Option<std::ffi::OsString>) -> Result<PathBuf> {
    if let Some(candidate) = configured {
        if candidate.is_absolute()
            && candidate
                .file_name()
                .is_some_and(|s| s.eq_ignore_ascii_case("es.exe"))
            && candidate.is_file()
        {
            return std::fs::canonicalize(candidate).map_err(|e| {
                SearchError::new(
                    ErrorCode::EsNotFound,
                    format!("Cannot access configured es.exe: {e}"),
                )
            });
        }
        return Err(SearchError::new(
            ErrorCode::EsNotFound,
            "NEXUS_ES_PATH must name an existing absolute es.exe path. Correct it and restart NEXUS.",
        ));
    }
    if let Some(path) = path {
        for directory in std::env::split_paths(&path).filter(|p| p.is_absolute()) {
            let candidate = directory.join("es.exe");
            if candidate.is_file() {
                return std::fs::canonicalize(candidate).map_err(|e| {
                    SearchError::new(
                        ErrorCode::EsNotFound,
                        format!("Cannot access es.exe on PATH: {e}"),
                    )
                });
            }
        }
    }
    Err(SearchError::new(
        ErrorCode::EsNotFound,
        "Everything's ES command-line tool was not found. Install ES separately and set NEXUS_ES_PATH or add its folder to PATH, then restart NEXUS.",
    ))
}

fn arguments(query: &str, limit: u32) -> Result<Vec<String>> {
    if query.len() > MAX_QUERY_BYTES
        || query.chars().any(char::is_control)
        || query.trim().is_empty()
        || !(1..=MAX_RESULTS).contains(&limit)
    {
        return Err(SearchError::new(
            ErrorCode::InvalidInput,
            "Enter 1–1024 UTF-8 bytes of search text without control characters, and a result limit from 1 to 100.",
        ));
    }
    // Encode EVERY character as a PCRE hex literal. Thus quotes, spaces, regex
    // metacharacters, ext:, !, |, wildcards and ES switches have no syntax role.
    // Regex mode bypasses Everything's normal expression language entirely.
    let literal: String = query
        .trim()
        .chars()
        .map(|c| format!("\\x{{{:X}}}", c as u32))
        .collect();
    Ok([
        "-argv",
        "-utf8-bom",
        "-code-page",
        "65001",
        "-no-pause",
        "-no-highlight",
        "-no-footer",
        "-header",
        "-no-case",
        "-no-whole-word",
        "-no-prefix",
        "-no-suffix",
        "-no-ignore-punctuation",
        "-no-ignore-whitespace",
        "-no-result-error",
        "-viewport-offset",
        "0",
        "-viewport-count",
        "100",
        "-match-path",
        "-efu",
        "-timeout",
        "3000",
        "-n",
    ]
    .into_iter()
    .map(String::from)
    .chain([limit.to_string(), "-regex".into(), literal])
    .collect())
}

fn check_exit(code: Option<i32>) -> Result<()> {
    match code {
        Some(0 | 9) => Ok(()), // 9 is requested explicitly with -no-result-error.
        Some(7 | 8) => Err(SearchError::new(
            ErrorCode::EverythingUnavailable,
            "Everything is not running or its search IPC is inaccessible. Start the Everything desktop client in this Windows session (the service alone is insufficient).",
        )),
        Some(4 | 6) => Err(SearchError::new(
            ErrorCode::ProcessFailed,
            "ES rejected the required options. Use a current ES version supporting -argv, -efu and -regex.",
        )),
        _ => Err(SearchError::new(
            ErrorCode::ProcessFailed,
            format!("ES failed with exit code {code:?}. Check Everything and the ES installation."),
        )),
    }
}

#[tauri::command]
pub async fn search_files(
    query: String,
    limit: u32,
    opened_paths: Option<Vec<String>>,
    search: tauri::State<'_, Arc<FileSearch>>,
) -> Result<Vec<FileResult>> {
    // Reject invalid IPC inputs before allocating a blocking worker.
    arguments(&query, limit)?;
    let opened = opened_paths.unwrap_or_default();
    validate_opened(&opened)?;
    let search = search.inner().clone();
    tauri::async_runtime::spawn_blocking(move || search.search_with_opened(&query, limit, &opened))
        .await
        .map_err(|e| {
            SearchError::new(
                ErrorCode::ProcessFailed,
                format!("File search worker failed: {e}"),
            )
        })?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn history_is_bounded_and_cannot_supply_process_arguments() {
        assert!(validate_opened(&vec!["x".into(); 257]).is_err());
        assert!(validate_opened(&["x".repeat(4097)]).is_err());
        assert!(validate_opened(&["bad\npath".into()]).is_err());
        assert!(validate_opened(&[]).is_ok());
    }
    #[test]
    fn user_text_cannot_become_switches_or_everything_operators() {
        for query in [
            "-exit",
            "ext:exe | !foo",
            "a\"b",
            "a.*",
            "文件 name",
            r"C:\a b\",
            "foo <bar>",
        ] {
            let args = arguments(query, 8).unwrap();
            assert_eq!(args[args.len() - 2], "-regex");
            let encoded = args.last().unwrap();
            assert!(!encoded.contains(query));
            assert!(encoded.starts_with("\\x{"));
            assert_eq!(args.iter().filter(|a| a.as_str() == "-exit").count(), 0);
        }
        assert_eq!(
            arguments(" a b ", 1).unwrap().last().unwrap(),
            "\\x{61}\\x{20}\\x{62}"
        );
    }
    #[test]
    fn validates_size_controls_and_limit() {
        for query in ["", "  ", "a\nb", "a\0b", "a\tb"] {
            assert!(arguments(query, 8).is_err());
        }
        assert!(arguments(&"a".repeat(1025), 8).is_err());
        assert!(arguments(&"a".repeat(1024), 100).is_ok());
        assert!(arguments("ok", 0).is_err());
        assert!(arguments("ok", 101).is_err());
    }
    #[test]
    fn availability_is_distinct_from_empty_success() {
        assert!(check_exit(Some(0)).is_ok());
        for code in [7, 8] {
            assert_eq!(
                check_exit(Some(code)).unwrap_err().code,
                ErrorCode::EverythingUnavailable
            );
        }
        assert!(check_exit(Some(9)).is_ok());
        for code in [1, 4, 6] {
            assert_eq!(
                check_exit(Some(code)).unwrap_err().code,
                ErrorCode::ProcessFailed
            );
        }
        assert!(check_exit(None).is_err());
    }
    #[test]
    fn bad_configuration_does_not_fall_back_to_another_executable() {
        assert!(
            locate(
                Some(PathBuf::from("relative/es.exe")),
                std::env::var_os("PATH")
            )
            .is_err()
        );
        assert!(locate(Some(std::env::current_exe().unwrap()), None).is_err());
        assert!(locate(None, None).is_err());
    }
}
