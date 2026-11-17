//! Read-only discovery context. No executor, probing process or generic dispatch IPC.
use crate::{
    action_registry::{ActionDescriptor, ActionRegistry},
    shortcut::ShortcutStatus,
};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryMetadata {
    application_version: String,
    actions: Vec<ActionDescriptor>,
    launcher_shortcut: ShortcutStatus,
    file_search: Availability,
}

#[derive(Serialize)]
struct Availability {
    status: &'static str,
    reason: String,
}

fn file_availability(result: Result<(), String>) -> Availability {
    match result {
        Ok(()) => Availability { status: "unknown", reason: "ES is configured. Everything client/index readiness is checked on search; discovery does not start ES.".into() },
        Err(reason) => Availability { status: "unavailable", reason },
    }
}

#[tauri::command]
pub async fn command_discovery_metadata(
    app: tauri::AppHandle,
    registry: tauri::State<'_, ActionRegistry>,
    shortcut: tauri::State<'_, ShortcutStatus>,
) -> Result<DiscoveryMetadata, String> {
    let actions = registry.descriptors();
    let launcher_shortcut = shortcut.inner().clone();
    let file_search = tauri::async_runtime::spawn_blocking(|| {
        file_availability(crate::file_search::discovery_availability())
    })
    .await
    .map_err(|error| error.to_string())?;
    Ok(DiscoveryMetadata {
        application_version: app.package_info().version.to_string(),
        actions,
        launcher_shortcut,
        file_search,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_es_is_visible_and_configured_es_does_not_claim_ready_index() {
        assert_eq!(
            file_availability(Err("ES missing".into())).status,
            "unavailable"
        );
        let configured = file_availability(Ok(()));
        assert_eq!(configured.status, "unknown");
        assert!(configured.reason.contains("does not start ES"));
    }

    #[test]
    fn metadata_serializes_authoritative_schemas_without_executors() {
        let registry = ActionRegistry::default();
        let metadata = DiscoveryMetadata {
            application_version: env!("CARGO_PKG_VERSION").into(),
            actions: registry.descriptors(),
            launcher_shortcut: ShortcutStatus {
                keys: crate::shortcut::LABEL.into(),
                registered: false,
                warning: Some("Conflict".into()),
            },
            file_search: file_availability(Err("ES missing".into())),
        };
        let value = serde_json::to_value(metadata).unwrap();
        for action in value["actions"].as_array().unwrap() {
            let authoritative =
                serde_json::to_value(registry.descriptor(action["id"].as_str().unwrap()).unwrap())
                    .unwrap();
            assert_eq!(*action, authoritative);
            assert!(action.get("execute").is_none());
            assert!(action.get("validate").is_none());
        }
        assert_eq!(value["launcherShortcut"]["keys"], crate::shortcut::LABEL);
        assert_eq!(value["fileSearch"]["status"], "unavailable");
    }

    #[test]
    fn frontend_contract_fixture_matches_compiled_action_registrations() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/discovery-actions.json"))
                .unwrap();
        let actual = serde_json::to_value(ActionRegistry::default().descriptors()).unwrap();
        assert_eq!(
            fixture, actual,
            "Regenerate the frontend contract fixture with the workflow_actions example when action metadata changes."
        );
    }
}
