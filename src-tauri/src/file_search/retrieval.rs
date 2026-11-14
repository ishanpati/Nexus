use super::{ErrorCode, FileResult, Result, SearchError, arguments, ranking};

pub(super) const POOL_LIMIT: u32 = 128;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Pass {
    Exact,
    Prefix,
    Substring,
    Path,
}

pub(super) fn pass_arguments(query: &str, pass: Pass, directory: bool) -> Result<Vec<String>> {
    let query = if ranking::path_query(query) {
        query.replace('/', "\\")
    } else {
        query.to_string()
    };
    // arguments validates the text and hex-escapes every user character.
    let mut args = arguments(&query, super::MAX_RESULTS)?;
    for flag in ["-n", "-viewport-count"] {
        let index = args.iter().position(|arg| arg == flag).unwrap();
        args[index + 1] = POOL_LIMIT.to_string();
    }
    let literal = args.pop().unwrap();
    args.pop(); // -regex
    let path_flag = args
        .iter_mut()
        .find(|arg| arg.as_str() == "-match-path")
        .unwrap();
    *path_flag = if pass == Pass::Path {
        "-match-path"
    } else {
        "-no-match-path"
    }
    .into();
    let pattern = match pass {
        Pass::Exact if directory => format!("^{literal}$"),
        Pass::Exact => format!("^{literal}(?:\\.[^.]+)?$"),
        Pass::Prefix => format!("^{literal}"),
        Pass::Substring | Pass::Path => literal,
    };
    args.extend([
        if directory { "/ad" } else { "/a-d" }.into(),
        "-sort".into(),
        "path".into(),
        "-no-folder-append-path-separator".into(),
        "-regex".into(),
        pattern,
    ]);
    Ok(args)
}

// At most two ES children concurrently. Each category has an independent file
// and folder pool; stop before lower-quality passes when enough candidates exist.
pub(super) fn collect(
    query: &str,
    limit: usize,
    run: impl Fn(Pass, bool) -> Result<Vec<FileResult>> + Sync,
) -> Result<Vec<FileResult>> {
    let passes: &[Pass] = if ranking::path_query(query) {
        &[Pass::Path]
    } else {
        &[Pass::Exact, Pass::Prefix, Pass::Substring, Pass::Path]
    };
    let mut candidates = Vec::new();
    for &pass in passes {
        let (folders, files) = std::thread::scope(|scope| {
            let folders = scope.spawn(|| run(pass, true));
            let files = run(pass, false);
            (
                folders
                    .join()
                    .map_err(|_| {
                        SearchError::new(
                            ErrorCode::ProcessFailed,
                            "Folder search worker stopped unexpectedly.",
                        )
                    })
                    .and_then(|result| result),
                files,
            )
        });
        candidates.extend(folders?);
        candidates.extend(files?);
        candidates = ranking::deduplicate(candidates);
        if candidates.len() >= limit {
            break;
        }
    }
    Ok(candidates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn every_pass_has_independent_bounded_file_and_folder_pools() {
        let calls = Mutex::new(Vec::new());
        let result = collect("nexus", 100, |pass, directory| {
            calls.lock().unwrap().push((pass, directory));
            Ok(Vec::new())
        })
        .unwrap();
        assert!(result.is_empty());
        let calls = calls.into_inner().unwrap();
        assert_eq!(calls.len(), 8);
        for pass in [Pass::Exact, Pass::Prefix, Pass::Substring, Pass::Path] {
            for directory in [true, false] {
                assert!(calls.contains(&(pass, directory)));
                let args = pass_arguments("nexus", pass, directory).unwrap();
                assert!(args.contains(&if directory { "/ad" } else { "/a-d" }.into()));
                let index = args.iter().position(|arg| arg == "-n").unwrap();
                assert_eq!(args[index + 1], POOL_LIMIT.to_string());
                assert!(
                    args.contains(
                        &if pass == Pass::Path {
                            "-match-path"
                        } else {
                            "-no-match-path"
                        }
                        .into()
                    )
                );
            }
        }
    }

    #[test]
    fn enough_exact_results_skip_lower_tiers_but_never_skip_folder_pool() {
        let calls = Mutex::new(Vec::new());
        let result = collect("nexus", 1, |pass, directory| {
            calls.lock().unwrap().push((pass, directory));
            Ok(vec![FileResult {
                full_path: if directory {
                    r"C:\nexus"
                } else {
                    r"C:\nexus.txt"
                }
                .into(),
                filename: if directory { "nexus" } else { "nexus.txt" }.into(),
                extension: String::new(),
                is_directory: directory,
            }])
        })
        .unwrap();
        assert_eq!(calls.into_inner().unwrap().len(), 2);
        assert_eq!(result.len(), 2); // final limit is applied only after ranking
    }

    #[test]
    fn explicit_paths_only_use_path_pass_and_errors_do_not_become_empty_results() {
        let calls = Mutex::new(Vec::new());
        collect(r"C:\Projects\nexus", 8, |pass, directory| {
            calls.lock().unwrap().push((pass, directory));
            Ok(vec![])
        })
        .unwrap();
        assert_eq!(calls.into_inner().unwrap().len(), 2);
        assert_eq!(
            collect("nexus", 8, |_, _| Err(SearchError::new(
                ErrorCode::EverythingUnavailable,
                "offline"
            )))
            .unwrap_err()
            .code,
            ErrorCode::EverythingUnavailable
        );
        let args = pass_arguments(r"C:/Projects/nexus", Pass::Path, false).unwrap();
        assert!(args.last().unwrap().contains(r"\x{5C}"));
    }

    #[test]
    fn query_text_is_literal_in_every_pattern_and_exact_stem_is_anchored() {
        for pass in [Pass::Exact, Pass::Prefix, Pass::Substring, Pass::Path] {
            let args = pass_arguments("-exit | ext:exe.*", pass, false).unwrap();
            assert!(!args.iter().any(|arg| arg == "-exit"));
            assert!(!args.last().unwrap().contains("ext:exe"));
        }
        assert_eq!(
            pass_arguments("a", Pass::Exact, true)
                .unwrap()
                .last()
                .unwrap(),
            r"^\x{61}$"
        );
        assert_eq!(
            pass_arguments("a", Pass::Exact, false)
                .unwrap()
                .last()
                .unwrap(),
            r"^\x{61}(?:\.[^.]+)?$"
        );
    }
}
