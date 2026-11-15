//! Read-only diagnostic. No shell, helper process, or volume changes.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = nexus_lib::audio::AudioService::default();
    let request =
        nexus_lib::audio::parse_request(serde_json::json!({"id":"get_audio_state","args":{}}))?;
    match service.execute(request) {
        Ok(state) => println!("{}", serde_json::to_string_pretty(&state)?),
        Err(error) => {
            eprintln!("{}", serde_json::to_string_pretty(&error)?);
            return Err(error.into());
        }
    }
    Ok(())
}
