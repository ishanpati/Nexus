use super::{ErrorCode, FileResult, Result, SearchError};

fn malformed() -> SearchError {
    SearchError::new(
        ErrorCode::MalformedOutput,
        "ES returned invalid UTF-8 or an unexpected EFU result format. Update ES and retry.",
    )
}

// Strict single-line CSV fields. Windows filenames cannot contain CR/LF; reject
// unexpected multiline records rather than accidentally returning partial paths.
fn fields(line: &str) -> Result<Vec<String>> {
    let mut chars = line.chars().peekable();
    let mut result = Vec::new();
    loop {
        let mut field = String::new();
        if chars.peek() == Some(&'"') {
            chars.next();
            loop {
                match chars.next() {
                    Some('"') if chars.peek() == Some(&'"') => {
                        chars.next();
                        field.push('"');
                    }
                    Some('"') => break,
                    Some(c) => field.push(c),
                    None => return Err(malformed()),
                }
            }
            if chars.peek().is_some_and(|c| *c != ',') {
                return Err(malformed());
            }
        } else {
            while chars.peek().is_some_and(|c| *c != ',') {
                let c = chars.next().unwrap();
                if c == '"' {
                    return Err(malformed());
                }
                field.push(c);
            }
        }
        result.push(field);
        if result.len() > 32 {
            return Err(malformed());
        }
        if chars.next().is_none() {
            break;
        }
    }
    Ok(result)
}

pub(super) fn parse(bytes: &[u8], limit: usize) -> Result<Vec<FileResult>> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| malformed())?
        .trim_start_matches('\u{feff}');
    let mut lines = text.lines();
    let header = fields(lines.next().ok_or_else(malformed)?)?;
    let column = |name| -> Result<usize> {
        let matches: Vec<_> = header
            .iter()
            .enumerate()
            .filter(|(_, s)| s.as_str() == name)
            .map(|(i, _)| i)
            .collect();
        if matches.len() == 1 {
            Ok(matches[0])
        } else {
            Err(malformed())
        }
    };
    let (path_column, attribute_column) = (column("Filename")?, column("Attributes")?);
    let mut result = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if result.len() >= limit {
            return Err(malformed());
        }
        let row = fields(line)?;
        if row.len() != header.len() {
            return Err(malformed());
        }
        let path = &row[path_column];
        if path.chars().any(char::is_control)
            || path.contains('"')
            || path.len() > 131072
            || !windows_absolute(path)
        {
            return Err(malformed());
        }
        let attributes = &row[attribute_column];
        let attributes = if let Some(hex) = attributes.strip_prefix("0x") {
            u32::from_str_radix(hex, 16)
        } else {
            attributes.parse::<u32>()
        }
        .map_err(|_| malformed())?;
        let is_directory = attributes & 16 != 0;
        if !is_directory && path.ends_with(['\\', '/']) {
            return Err(malformed());
        }
        let filename = path
            .trim_end_matches(['\\', '/'])
            .rsplit(['\\', '/'])
            .next()
            .ok_or_else(malformed)?
            .to_owned();
        if filename.is_empty() {
            return Err(malformed());
        }
        let extension = if is_directory {
            ""
        } else {
            filename
                .rsplit_once('.')
                .filter(|(stem, _)| !stem.is_empty())
                .map_or("", |(_, ext)| ext)
        }
        .to_owned();
        result.push(FileResult {
            full_path: path.clone(),
            filename,
            extension,
            is_directory,
        });
    }
    Ok(result)
}

fn windows_absolute(path: &str) -> bool {
    let b = path.as_bytes();
    (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && matches!(b[2], b'\\' | b'/'))
        || (path.starts_with("\\\\")
            && path[2..].split('\\').filter(|s| !s.is_empty()).count() >= 2)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_spaces_commas_folders_unc_and_extensions() {
        let input = "\u{feff}Filename,Size,Date Modified,Date Created,Attributes\r\n\"C:\\資料\\my, file.TXT\",12,,,32\r\n\"C:\\folder.with.dot\\\",,,,16\r\n\"\\\\server\\share\\README\",1,,,0x20\r\n";
        let rows = parse(input.as_bytes(), 8).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].filename, "my, file.TXT");
        assert_eq!(rows[0].extension, "TXT");
        assert_eq!(rows[0].full_path, "C:\\資料\\my, file.TXT");
        assert!(rows[1].is_directory);
        assert_eq!(rows[1].extension, "");
        assert_eq!(rows[2].extension, "");
    }
    #[test]
    fn empty_catalog_is_success_but_empty_or_malformed_output_is_not() {
        assert!(parse(b"Filename,Attributes\r\n", 8).unwrap().is_empty());
        for input in [
            "",
            "error message",
            "Filename,Attributes\nrelative.txt,32",
            "Filename,Attributes\n\"C:\\broken,32",
            "Filename,Attributes\nC:\\a,invalid",
            "Filename,Attributes\nC:\\a,32,extra",
            "Filename,Attributes\nhttps://example.com,32",
        ] {
            assert!(parse(input.as_bytes(), 8).is_err(), "{input}");
        }
        assert!(parse(&[255], 8).is_err());
        assert!(parse(b"Filename,Attributes\nC:\\a,32\nC:\\b,32", 1).is_err());
        assert_eq!(fields("\"a\"\"b\",c").unwrap(), ["a\"b", "c"]);
    }
}
