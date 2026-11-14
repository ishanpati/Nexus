use super::*;

fn item(path: &str, directory: bool) -> FileResult {
    let filename = path
        .trim_end_matches(['\\', '/'])
        .rsplit(['\\', '/'])
        .next()
        .unwrap()
        .to_string();
    FileResult {
        extension: if directory {
            String::new()
        } else {
            filename.rsplit_once('.').map_or("", |(_, ext)| ext).into()
        },
        filename,
        full_path: path.into(),
        is_directory: directory,
    }
}
fn context() -> Context {
    Context {
        desktop: Some(normalized(r"C:\Users\A\OneDrive\Desktop")),
        documents: Some(normalized(r"C:\Users\A\OneDrive\Documents")),
        downloads: Some(normalized(r"C:\Users\A\Downloads")),
        projects: vec![normalized(r"C:\Projects")],
    }
}
fn paths(items: Vec<FileResult>) -> Vec<String> {
    items.into_iter().map(|item| item.full_path).collect()
}

#[test]
fn exact_nexus_folder_beats_all_descendant_path_matches() {
    let results = rank(
        vec![
            item(r"C:\Projects\nexus\node_modules\a.txt", false),
            item(r"C:\Projects\nexus\src", true),
            item(r"C:\Projects\nexus", true),
        ],
        "nexus",
        &context(),
        &[],
        100,
    );
    assert_eq!(results[0].full_path, r"C:\Projects\nexus");
    assert!(results[0].is_directory);
    assert_eq!(results.len(), 3);
}

#[test]
fn redirected_desktop_exact_stem_beats_temp_even_with_history() {
    let temp = r"C:\Users\A\AppData\Local\Temp\finance_tracker.xlsx";
    let desktop = r"C:\Users\A\OneDrive\Desktop\finance_tracker.xlsx";
    let items = vec![item(temp, false), item(desktop, false)];
    for query in ["finance_tracker", "finance_tracker.xlsx", "FINANCE_TRACKER"] {
        let result = rank(items.clone(), query, &context(), &[temp.into()], 100);
        assert_eq!(result[0].full_path, desktop);
        assert_eq!(result.len(), 2); // penalty, not exclusion
    }
}

#[test]
fn lexical_tiers_cannot_be_overridden_by_location_or_history() {
    let exact = r"C:\Temp\node_modules\.git\dist\target\finance.xlsx";
    let prefix = r"C:\Users\A\OneDrive\Desktop\finance_tracker.xlsx";
    let word = r"C:\Users\A\OneDrive\Desktop\my_finance.xlsx";
    let substring = r"C:\Users\A\OneDrive\Desktop\myfinance.xlsx";
    let path = r"C:\finance\x.xlsx";
    let inputs = [path, substring, word, prefix, exact];
    let result = paths(rank(
        inputs.map(|p| item(p, false)).to_vec(),
        "finance",
        &context(),
        &[prefix.into(), word.into()],
        100,
    ));
    assert_eq!(result, [exact, prefix, word, substring, path]);
}

#[test]
fn same_tier_context_is_modest_and_component_based() {
    let input = [
        r"C:\Temp\notes.txt",
        r"C:\cache\notes.txt",
        r"C:\Projects\notes.txt",
        r"C:\Users\A\Downloads\notes.txt",
        r"C:\Users\A\OneDrive\Documents\notes.txt",
        r"C:\Users\A\OneDrive\Desktop\notes.txt",
    ];
    let result = paths(rank(
        input.map(|p| item(p, false)).to_vec(),
        "notes",
        &context(),
        &[],
        100,
    ));
    assert_eq!(result[0], input[5]);
    assert!(
        result.iter().position(|p| p == input[2]).unwrap()
            < result.iter().position(|p| p == input[0]).unwrap()
    );
    assert_eq!(
        context_score(
            r"c:\attempt\distillery\x",
            &Context::default(),
            &HashSet::new()
        ),
        0
    );
    let history = rank(
        vec![
            item(r"C:\a\notes.txt", false),
            item(r"C:\z\notes.txt", false),
        ],
        "notes",
        &Context::default(),
        &[r"C:\z\notes.txt".into()],
        2,
    );
    assert_eq!(history[0].full_path, r"C:\z\notes.txt");
}

#[test]
fn unicode_stems_classification_and_empty_queries() {
    let inputs = vec![
        item(r"C:\資料\résumé 日本語.xlsx", false),
        item(r"C:\資料\résumé 日本語", true),
    ];
    let result = rank(
        inputs.clone(),
        "RÉSUMÉ 日本語",
        &Context::default(),
        &[],
        10,
    );
    assert_eq!(result.len(), 2);
    assert!(result.iter().any(|item| item.is_directory));
    assert!(
        result
            .iter()
            .any(|item| !item.is_directory && item.extension == "xlsx")
    );
    assert!(rank(inputs.clone(), "", &Context::default(), &[], 10).is_empty());
    assert!(rank(inputs, "nonsense", &Context::default(), &[], 10).is_empty());
    assert_eq!(
        tier(&item(r"C:\a\report.final.xlsx", false), "report.final"),
        Some(0)
    );
    assert_eq!(tier(&item(r"C:\a\folder.ext", true), "folder"), Some(1));
}

#[test]
fn deduplication_keeps_distinct_paths_and_order_is_stable() {
    let input = vec![
        item(r"C:\Docs\nexus", true),
        item("c:/docs/NEXUS/", true),
        item(r"\\?\C:\Docs\nexus", true),
        item(r"D:\Docs\nexus", true),
        item(r"C:\other\nexus", true),
    ];
    let first = rank(input.clone(), "nexus", &Context::default(), &[], 100);
    let reverse = rank(
        input.into_iter().rev().collect(),
        "nexus",
        &Context::default(),
        &[],
        100,
    );
    assert_eq!(first, reverse);
    assert_eq!(first.len(), 3);
    assert_eq!(
        normalized(r"\\?\UNC\server\share\file"),
        normalized(r"\\SERVER\SHARE\FILE")
    );
}
