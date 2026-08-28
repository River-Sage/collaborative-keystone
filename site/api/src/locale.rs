use std::{env, sync::OnceLock};

use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

const DEFAULT_LOCALE_SLUG: &str = "world";
const DEFAULT_LOCALE_NAME: &str = "World";
const DEFAULT_LOCALE_TYPE: &str = "world";
const MAX_LOCALE_SLUG_CHARS: usize = 80;
const MAX_LOCALE_NAME_CHARS: usize = 120;
const MAX_LOCALE_TYPE_CHARS: usize = 80;

static CONFIGURED_LOCALE: OnceLock<LocaleConfig> = OnceLock::new();

#[derive(Clone, Debug, Serialize)]
pub struct LocaleConfig {
    pub slug: String,
    pub name: String,
    pub locale_type: String,
}

impl LocaleConfig {
    pub fn from_env() -> Result<Self, String> {
        let raw_slug =
            env::var("CK_LOCALE_SLUG").unwrap_or_else(|_| DEFAULT_LOCALE_SLUG.to_string());
        let slug = normalize_locale_slug(&raw_slug)?;
        let raw_name = env::var("CK_LOCALE_NAME").unwrap_or_else(|_| default_name_for_slug(&slug));
        let name = validate_locale_name(&raw_name)?;
        let raw_type =
            env::var("CK_LOCALE_TYPE").unwrap_or_else(|_| DEFAULT_LOCALE_TYPE.to_string());
        let locale_type = normalize_locale_type(&raw_type)?;

        Ok(Self {
            slug,
            name,
            locale_type,
        })
    }
}

pub fn initialize_from_env() -> Result<LocaleConfig, String> {
    let config = LocaleConfig::from_env()?;
    let _ = CONFIGURED_LOCALE.set(config.clone());
    Ok(config)
}

pub fn configured_locale() -> LocaleConfig {
    CONFIGURED_LOCALE
        .get()
        .cloned()
        .unwrap_or_else(|| LocaleConfig::from_env().unwrap_or_else(|_| default_locale()))
}

pub fn configured_locale_slug() -> String {
    configured_locale().slug
}

pub async fn ensure_configured_locale(
    db: &PgPool,
    config: &LocaleConfig,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO locales (slug, name, is_active)
        VALUES ($1, $2, TRUE)
        ON CONFLICT (slug)
        DO UPDATE SET
            name = EXCLUDED.name,
            is_active = TRUE
        RETURNING id
        "#,
    )
    .bind(&config.slug)
    .bind(&config.name)
    .fetch_one(db)
    .await?
    .try_get("id")
}

fn default_locale() -> LocaleConfig {
    LocaleConfig {
        slug: DEFAULT_LOCALE_SLUG.to_string(),
        name: DEFAULT_LOCALE_NAME.to_string(),
        locale_type: DEFAULT_LOCALE_TYPE.to_string(),
    }
}

fn default_name_for_slug(slug: &str) -> String {
    if slug == DEFAULT_LOCALE_SLUG {
        DEFAULT_LOCALE_NAME.to_string()
    } else {
        slug.split('-')
            .filter(|part| !part.is_empty())
            .map(|part| {
                let mut chars = part.chars();
                match chars.next() {
                    Some(first) => {
                        first.to_ascii_uppercase().to_string()
                            + chars.as_str().to_ascii_lowercase().as_str()
                    }
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn validate_locale_name(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("CK_LOCALE_NAME must not be empty.".to_string());
    }
    if trimmed.chars().count() > MAX_LOCALE_NAME_CHARS {
        return Err(format!(
            "CK_LOCALE_NAME must be {MAX_LOCALE_NAME_CHARS} characters or fewer."
        ));
    }

    Ok(trimmed.to_string())
}

fn normalize_locale_type(value: &str) -> Result<String, String> {
    let normalized = normalize_code(value);
    if normalized.is_empty() {
        return Err("CK_LOCALE_TYPE must not be empty.".to_string());
    }
    if normalized.chars().count() > MAX_LOCALE_TYPE_CHARS {
        return Err(format!(
            "CK_LOCALE_TYPE must be {MAX_LOCALE_TYPE_CHARS} characters or fewer."
        ));
    }

    Ok(normalized)
}

fn normalize_locale_slug(value: &str) -> Result<String, String> {
    let normalized = normalize_code(value);
    if normalized.is_empty() {
        return Err("CK_LOCALE_SLUG must not be empty.".to_string());
    }
    if normalized.chars().count() > MAX_LOCALE_SLUG_CHARS {
        return Err(format!(
            "CK_LOCALE_SLUG must be {MAX_LOCALE_SLUG_CHARS} characters or fewer."
        ));
    }
    if normalized.starts_with('-') || normalized.ends_with('-') {
        return Err("CK_LOCALE_SLUG must not start or end with '-'.".to_string());
    }

    Ok(normalized)
}

fn normalize_code(value: &str) -> String {
    let mut output = String::new();
    let mut previous_was_dash = false;

    for ch in value.trim().chars().flat_map(char::to_lowercase) {
        let next = if ch.is_ascii_alphanumeric() {
            Some(ch)
        } else if ch == '-' || ch == '_' || ch.is_whitespace() {
            Some('-')
        } else {
            None
        };

        if let Some(next) = next {
            if next == '-' {
                if output.is_empty() || previous_was_dash {
                    continue;
                }
                previous_was_dash = true;
            } else {
                previous_was_dash = false;
            }
            output.push(next);
        }
    }

    output.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::{default_name_for_slug, normalize_locale_slug, normalize_locale_type};

    #[test]
    fn locale_slug_normalizes_for_operator_input() {
        assert_eq!(
            normalize_locale_slug(" Castle Rock ").expect("slug should normalize"),
            "castle-rock"
        );
        assert_eq!(
            normalize_locale_slug("Douglas_County").expect("slug should normalize"),
            "douglas-county"
        );
    }

    #[test]
    fn locale_slug_rejects_empty_values() {
        assert!(normalize_locale_slug(" !!! ").is_err());
    }

    #[test]
    fn locale_type_uses_same_public_code_rules() {
        assert_eq!(
            normalize_locale_type("Municipal Instance").expect("type should normalize"),
            "municipal-instance"
        );
    }

    #[test]
    fn display_name_can_be_derived_from_slug() {
        assert_eq!(default_name_for_slug("castle-rock"), "Castle Rock");
        assert_eq!(default_name_for_slug("world"), "World");
    }
}
