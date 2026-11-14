//! Read-only diagnostic: exercises the same service used by the Tauri command.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let query = args
        .next()
        .ok_or("Usage: --example search_files -- <literal query> [limit]")?;
    let limit = args
        .next()
        .map(|s| s.parse::<u32>())
        .transpose()?
        .unwrap_or(8);
    if args.next().is_some() {
        return Err("Quote the query as a single argument.".into());
    }
    match nexus_lib::file_search::FileSearch::default().search(&query, limit) {
        Ok(results) => println!("{}", serde_json::to_string_pretty(&results)?),
        Err(error) => {
            eprintln!("{}", serde_json::to_string_pretty(&error)?);
            return Err(error.into());
        }
    }
    Ok(())
}
