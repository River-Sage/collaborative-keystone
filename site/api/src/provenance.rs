use std::{env, sync::Arc};

use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::{AppState, bootstrap};

const DEFAULT_SOURCE_REPOSITORY_URL: &str = "https://github.com/River-Sage/collaborative-keystone";
const LICENSE_NAME: &str = "GNU Affero General Public License v3.0";
const LICENSE_URL: &str = "https://www.gnu.org/licenses/agpl-3.0.en.html";
const REGISTRY_STATUS_CODES: &[&str] = &[
    "canonical",
    "official",
    "authorized",
    "verified",
    "stale",
    "warning",
    "suspended",
    "compromised",
    "abandoned",
    "community",
    "unverified",
    "development",
];
const TRUST_TIER_CODES: &[&str] = &[
    "development",
    "unsigned",
    "signed-release",
    "signed-release-reproducible",
];

#[derive(Debug, Serialize)]
pub struct SourceInfoResponse {
    pub ok: bool,
    pub product: ProductInfo,
    pub source_repository_url: String,
    pub source_repository_configured: bool,
    pub license: LicenseInfo,
    pub trademark_policy_file: String,
    pub provenance_manifest_path: String,
    pub registry_manifest_path: String,
    pub deployment_status: String,
    pub registry_status: String,
    pub brand_policy: BrandPolicyInfo,
}

#[derive(Debug, Serialize)]
pub struct ProductInfo {
    pub core: String,
    pub global: String,
    pub distribution: String,
    pub instance: String,
}

#[derive(Debug, Serialize)]
pub struct LicenseInfo {
    pub name: String,
    pub url: String,
    pub local_file: String,
    pub source_availability: String,
}

#[derive(Debug, Serialize)]
pub struct BrandPolicyInfo {
    pub official_identity_reserved: String,
    pub authorized_locale_branding: String,
    pub community_fork_branding: String,
    pub trademark_policy_file: String,
    pub operator_agreement_template: String,
}

#[derive(Debug, Serialize)]
pub struct BuildProvenanceResponse {
    pub schema_version: String,
    pub product: String,
    pub deployment_kind: String,
    pub deployment_status: String,
    pub registry_status: String,
    pub trust_tier: String,
    pub source_repository_url: String,
    pub source_repository_configured: bool,
    pub git_commit_sha: Option<String>,
    pub release_id: Option<String>,
    pub build_timestamp: Option<String>,
    pub build_environment: Option<String>,
    pub web_artifact_digest: Option<String>,
    pub api_artifact_digest: Option<String>,
    pub migration_set_digest: Option<String>,
    pub configuration_digest: Option<String>,
    pub signature: Option<String>,
    pub public_verification_key: Option<String>,
    pub signature_status: String,
    pub locale: LocaleInfo,
    pub operator: OperatorInfo,
    pub bootstrap: BootstrapInfo,
    pub source_license_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocaleInfo {
    pub slug: String,
    pub name: String,
    #[serde(alias = "type")]
    pub locale_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorInfo {
    pub name: Option<String>,
    pub contact: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BootstrapInfo {
    pub first_moderator_bootstrap_complete: bool,
}

#[derive(Debug, Serialize)]
pub struct LocaleRegistryResponse {
    pub ok: bool,
    pub schema_version: String,
    pub registry_origin: Option<String>,
    pub generated_for: LocaleInfo,
    pub entries: Vec<LocaleRegistryEntry>,
    pub statuses: Vec<RegistryStatusDefinition>,
    pub registry_config_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocaleRegistryEntry {
    pub locale: LocaleInfo,
    pub web_origin: Option<String>,
    pub api_origin: Option<String>,
    pub operator: OperatorInfo,
    pub registry_status: String,
    pub trust_tier: String,
    pub release_id: Option<String>,
    pub source_repository_url: String,
    pub provenance_manifest_path: String,
    pub instance_public_key: Option<String>,
    pub last_verified_at: Option<String>,
    pub official_branding_allowed: bool,
    pub brand_claim: String,
}

#[derive(Debug, Deserialize)]
struct LocaleRegistryEntryConfig {
    locale: LocaleInfo,
    web_origin: Option<String>,
    api_origin: Option<String>,
    operator: Option<OperatorInfo>,
    registry_status: Option<String>,
    trust_tier: Option<String>,
    release_id: Option<String>,
    source_repository_url: Option<String>,
    provenance_manifest_path: Option<String>,
    instance_public_key: Option<String>,
    last_verified_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RegistryStatusDefinition {
    pub code: String,
    pub label: String,
    pub description: String,
    pub official_branding_allowed: bool,
}

pub async fn source_info_handler() -> Json<SourceInfoResponse> {
    let source_repository_url = source_repository_url();
    let registry_status = registry_status();
    Json(SourceInfoResponse {
        ok: true,
        product: ProductInfo {
            core: "Keystone Core".to_string(),
            global: "Global Keystone".to_string(),
            distribution: "Locale Keystone Distribution".to_string(),
            instance: "Locale Keystone Instance".to_string(),
        },
        source_repository_url: source_repository_url.0,
        source_repository_configured: source_repository_url.1,
        license: LicenseInfo {
            name: LICENSE_NAME.to_string(),
            url: LICENSE_URL.to_string(),
            local_file: "LICENSE".to_string(),
            source_availability:
                "Running instances must provide corresponding source code under AGPL expectations."
                    .to_string(),
        },
        trademark_policy_file: "TRADEMARKS.md".to_string(),
        provenance_manifest_path: "/.well-known/keystone-build.json".to_string(),
        registry_manifest_path: "/.well-known/keystone-locales.json".to_string(),
        deployment_status: env_or_default("CK_DEPLOYMENT_STATUS", "development"),
        registry_status,
        brand_policy: BrandPolicyInfo {
            official_identity_reserved:
                "Global Keystone, official seals/checkmarks, the canonical domain, and official wording are reserved."
                    .to_string(),
            authorized_locale_branding:
                "Official {Locale} Keystone branding requires project authorization and an operator agreement."
                    .to_string(),
            community_fork_branding:
                "Community forks may identify themselves as forks or as powered by Keystone Core, but must not look official."
                    .to_string(),
            trademark_policy_file: "TRADEMARKS.md".to_string(),
            operator_agreement_template: "docs/operator-agreement-template.md".to_string(),
        },
    })
}

pub async fn build_provenance_handler(
    State(state): State<Arc<AppState>>,
) -> Json<BuildProvenanceResponse> {
    let source_repository_url = source_repository_url();
    let signature = optional_env("CK_PROVENANCE_SIGNATURE");
    let signature_status = if signature.is_some() {
        "signed"
    } else {
        "unsigned"
    };
    let registry_status = registry_status();
    let trust_tier = trust_tier(signature_status);

    Json(BuildProvenanceResponse {
        schema_version: "keystone-build-provenance/v1".to_string(),
        product: "Keystone Core".to_string(),
        deployment_kind: env_or_default("CK_DEPLOYMENT_KIND", "local"),
        deployment_status: env_or_default("CK_DEPLOYMENT_STATUS", "development"),
        registry_status,
        trust_tier,
        source_repository_url: source_repository_url.0,
        source_repository_configured: source_repository_url.1,
        git_commit_sha: optional_env("CK_GIT_COMMIT_SHA"),
        release_id: optional_env("CK_RELEASE_ID"),
        build_timestamp: optional_env("CK_BUILD_TIMESTAMP"),
        build_environment: optional_env("CK_BUILD_ENVIRONMENT"),
        web_artifact_digest: optional_env("CK_WEB_ARTIFACT_DIGEST"),
        api_artifact_digest: optional_env("CK_API_ARTIFACT_DIGEST"),
        migration_set_digest: optional_env("CK_MIGRATION_SET_DIGEST"),
        configuration_digest: optional_env("CK_CONFIGURATION_DIGEST"),
        signature,
        public_verification_key: optional_env("CK_PUBLIC_VERIFICATION_KEY"),
        signature_status: signature_status.to_string(),
        locale: load_locale_info(&state).await,
        operator: OperatorInfo {
            name: optional_env("CK_OPERATOR_NAME"),
            contact: optional_env("CK_OPERATOR_CONTACT"),
        },
        bootstrap: BootstrapInfo {
            first_moderator_bootstrap_complete: bootstrap::first_moderator_bootstrap_complete(
                &state.db,
            )
            .await,
        },
        source_license_path: "/source-info".to_string(),
    })
}

pub async fn locale_registry_handler(
    State(state): State<Arc<AppState>>,
) -> Json<LocaleRegistryResponse> {
    let locale = load_locale_info(&state).await;
    let registry_status = registry_status();
    let signature = optional_env("CK_PROVENANCE_SIGNATURE");
    let signature_status = if signature.is_some() {
        "signed"
    } else {
        "unsigned"
    };
    let trust_tier = trust_tier(signature_status);
    let source_repository_url = source_repository_url().0;
    let (configured_entries, registry_config_error) = configured_registry_entries();
    let mut entries = vec![LocaleRegistryEntry {
        locale: locale.clone(),
        web_origin: optional_env("PUBLIC_WEB_ORIGIN").or_else(|| optional_env("WEB_ORIGIN")),
        api_origin: optional_env("PUBLIC_API_ORIGIN").or_else(|| optional_env("CK_API_ORIGIN")),
        operator: OperatorInfo {
            name: optional_env("CK_OPERATOR_NAME"),
            contact: optional_env("CK_OPERATOR_CONTACT"),
        },
        registry_status: registry_status.clone(),
        trust_tier,
        release_id: optional_env("CK_RELEASE_ID"),
        source_repository_url,
        provenance_manifest_path: "/.well-known/keystone-build.json".to_string(),
        instance_public_key: optional_env("CK_INSTANCE_PUBLIC_KEY"),
        last_verified_at: optional_env("CK_LAST_VERIFIED_AT"),
        official_branding_allowed: official_branding_allowed(&registry_status),
        brand_claim: brand_claim(&registry_status),
    }];
    entries.extend(configured_entries);

    Json(LocaleRegistryResponse {
        ok: true,
        schema_version: "keystone-locale-registry/v1".to_string(),
        registry_origin: optional_env("CK_GLOBAL_REGISTRY_ORIGIN")
            .or_else(|| optional_env("PUBLIC_WEB_ORIGIN")),
        generated_for: locale.clone(),
        entries,
        statuses: registry_status_definitions(),
        registry_config_error,
    })
}

async fn load_locale_info(state: &AppState) -> LocaleInfo {
    LocaleInfo {
        slug: state.locale.slug.clone(),
        name: state.locale.name.clone(),
        locale_type: state.locale.locale_type.clone(),
    }
}

fn configured_registry_entries() -> (Vec<LocaleRegistryEntry>, Option<String>) {
    let Some(raw_entries) = optional_env("CK_LOCALE_REGISTRY_JSON") else {
        return (Vec::new(), None);
    };

    let source_repository_url = source_repository_url().0;
    match registry_entries_from_json(&raw_entries, &source_repository_url) {
        Ok(entries) => (entries, None),
        Err(error) => {
            warn!("invalid CK_LOCALE_REGISTRY_JSON: {}", error);
            (Vec::new(), Some(error))
        }
    }
}

fn registry_entries_from_json(
    raw_entries: &str,
    default_source_repository_url: &str,
) -> Result<Vec<LocaleRegistryEntry>, String> {
    let configs: Vec<LocaleRegistryEntryConfig> =
        serde_json::from_str(raw_entries).map_err(|err| err.to_string())?;

    Ok(configs
        .into_iter()
        .map(|config| {
            let registry_status = normalize_code(
                config.registry_status.as_deref().unwrap_or("unverified"),
                "unverified",
                REGISTRY_STATUS_CODES,
            );
            let trust_tier = normalize_code(
                config.trust_tier.as_deref().unwrap_or("unsigned"),
                "unsigned",
                TRUST_TIER_CODES,
            );
            let source_repository_url = config
                .source_repository_url
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| default_source_repository_url.to_string());
            let provenance_manifest_path = config
                .provenance_manifest_path
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "/.well-known/keystone-build.json".to_string());

            LocaleRegistryEntry {
                locale: config.locale,
                web_origin: config.web_origin,
                api_origin: config.api_origin,
                operator: config.operator.unwrap_or(OperatorInfo {
                    name: None,
                    contact: None,
                }),
                official_branding_allowed: official_branding_allowed(&registry_status),
                brand_claim: brand_claim(&registry_status),
                registry_status,
                trust_tier,
                release_id: config.release_id,
                source_repository_url,
                provenance_manifest_path,
                instance_public_key: config.instance_public_key,
                last_verified_at: config.last_verified_at,
            }
        })
        .collect())
}

fn source_repository_url() -> (String, bool) {
    match optional_env("CK_SOURCE_REPOSITORY_URL") {
        Some(value) => (value, true),
        None => (DEFAULT_SOURCE_REPOSITORY_URL.to_string(), false),
    }
}

fn env_or_default(key: &str, default: &str) -> String {
    optional_env(key).unwrap_or_else(|| default.to_string())
}

fn registry_status() -> String {
    let candidate = optional_env("CK_REGISTRY_STATUS")
        .or_else(|| optional_env("CK_DEPLOYMENT_STATUS"))
        .unwrap_or_else(|| "development".to_string());

    normalize_code(&candidate, "unverified", REGISTRY_STATUS_CODES)
}

fn trust_tier(signature_status: &str) -> String {
    let default = if signature_status == "signed" {
        "signed-release"
    } else {
        "development"
    };
    let candidate = optional_env("CK_TRUST_TIER").unwrap_or_else(|| default.to_string());

    normalize_code(&candidate, default, TRUST_TIER_CODES)
}

fn normalize_code(value: &str, default: &str, allowed: &[&str]) -> String {
    let normalized = value.trim().to_lowercase().replace('_', "-");
    if allowed.iter().any(|code| *code == normalized) {
        normalized
    } else {
        default.to_string()
    }
}

fn official_branding_allowed(registry_status: &str) -> bool {
    matches!(
        registry_status,
        "canonical" | "official" | "authorized" | "verified"
    )
}

fn brand_claim(registry_status: &str) -> String {
    match registry_status {
        "canonical" => "canonical-global".to_string(),
        "official" | "authorized" | "verified" => "authorized-locale".to_string(),
        "community" => "community-fork".to_string(),
        "development" => "development-instance".to_string(),
        _ => "no-official-brand-claim".to_string(),
    }
}

fn registry_status_definitions() -> Vec<RegistryStatusDefinition> {
    vec![
        registry_status_definition(
            "canonical",
            "Canonical",
            "The project-operated global entry point and reference deployment.",
            true,
        ),
        registry_status_definition(
            "official",
            "Official",
            "A project-operated or project-authorized deployment using official branding.",
            true,
        ),
        registry_status_definition(
            "authorized",
            "Authorized",
            "A locale operator accepted by the project and allowed to use approved locale branding.",
            true,
        ),
        registry_status_definition(
            "verified",
            "Verified",
            "A locale deployment whose operator agreement, release provenance, and registry check-in are current.",
            true,
        ),
        registry_status_definition(
            "stale",
            "Stale",
            "Previously verified, but the registry has not seen a fresh check-in or release check.",
            false,
        ),
        registry_status_definition(
            "warning",
            "Warning",
            "Listed, but users should review a visible warning before relying on this deployment.",
            false,
        ),
        registry_status_definition(
            "suspended",
            "Suspended",
            "Temporarily removed from official trust until an operator or compliance issue is resolved.",
            false,
        ),
        registry_status_definition(
            "compromised",
            "Compromised",
            "Evidence suggests the instance, keys, operator account, or deployment has been compromised.",
            false,
        ),
        registry_status_definition(
            "abandoned",
            "Abandoned",
            "No responsible operator is maintaining the locale deployment.",
            false,
        ),
        registry_status_definition(
            "community",
            "Community",
            "An independent fork or deployment that may run AGPL code but is not official.",
            false,
        ),
        registry_status_definition(
            "unverified",
            "Unverified",
            "The registry has not verified this deployment's release, operator, or brand status.",
            false,
        ),
        registry_status_definition(
            "development",
            "Development",
            "A local or test deployment with no public trust claim.",
            false,
        ),
    ]
}

fn registry_status_definition(
    code: &str,
    label: &str,
    description: &str,
    official_branding_allowed: bool,
) -> RegistryStatusDefinition {
    RegistryStatusDefinition {
        code: code.to_string(),
        label: label.to_string(),
        description: description.to_string(),
        official_branding_allowed,
    }
}

#[cfg(test)]
mod tests {
    use super::registry_entries_from_json;

    #[test]
    fn registry_entries_from_json_normalizes_trust_fields() {
        let entries = registry_entries_from_json(
            r#"[{
                "locale": {
                    "slug": "castle-rock",
                    "name": "Castle Rock",
                    "locale_type": "municipality"
                },
                "web_origin": "https://castle-rock.example.test",
                "api_origin": "https://api.castle-rock.example.test",
                "registry_status": "Verified",
                "trust_tier": "signed_release"
            }]"#,
            "https://example.test/source",
        )
        .expect("registry entry json should parse");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].locale.slug, "castle-rock");
        assert_eq!(entries[0].registry_status, "verified");
        assert_eq!(entries[0].trust_tier, "signed-release");
        assert!(entries[0].official_branding_allowed);
        assert_eq!(entries[0].brand_claim, "authorized-locale");
        assert_eq!(
            entries[0].source_repository_url,
            "https://example.test/source"
        );
    }

    #[test]
    fn registry_entries_from_json_accepts_type_alias() {
        let entries = registry_entries_from_json(
            r#"[{
                "locale": {
                    "slug": "world",
                    "name": "World",
                    "type": "world"
                }
            }]"#,
            "https://example.test/source",
        )
        .expect("registry entry json should accept type alias");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].locale.locale_type, "world");
    }

    #[test]
    fn registry_entries_from_json_rejects_malformed_json() {
        assert!(registry_entries_from_json("not json", "https://example.test/source").is_err());
    }
}

fn optional_env(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
