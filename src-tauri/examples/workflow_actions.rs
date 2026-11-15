//! Metadata inspection only. Does not discover or execute anything.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "{}",
        serde_json::to_string_pretty(
            &nexus_lib::action_registry::ActionRegistry::default().descriptors()
        )?
    );
    Ok(())
}
