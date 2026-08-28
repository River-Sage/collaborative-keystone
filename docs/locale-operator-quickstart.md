# Locale Operator Quickstart

Keystone is intended to run one configured locale per deployment.

Examples:

- `World Keystone` is the canonical global entry point and locale registry.
- `Castle Rock Keystone` is a separate locale instance with its own database, domains, moderator bootstrap, audit trail, cycle history, and implementation records.

Users should discover locale instances through World Keystone. Direct locale URLs may exist, but the trusted path is the World registry.

## What Can Be Verified

The practical trust claim is:

> This instance is running an official signed release with only approved locale/environment configuration changes.

That is different from saying the open-source code cannot be changed. Keystone Core is AGPL software. Forks can modify it, but modified or unsigned deployments must not claim official or verified status unless the global registry and release provenance support that claim.

## Required Runtime Identity

Every locale API should set:

```powershell
$env:CK_LOCALE_SLUG = "castle-rock"
$env:CK_LOCALE_NAME = "Castle Rock"
$env:CK_LOCALE_TYPE = "municipality"
$env:PUBLIC_WEB_ORIGIN = "https://castle-rock.example.org"
$env:PUBLIC_API_ORIGIN = "https://api.castle-rock.example.org"
$env:CK_GLOBAL_REGISTRY_ORIGIN = "https://collaborativekeystone.com"
$env:CK_REGISTRY_STATUS = "authorized"
```

The API upserts the configured locale on startup and opens that locale's active UTC calendar-month cycle if none exists.

Registry JSON should use `locale_type` for locale entries. The API also accepts `type` as a convenience alias for operator-written config.

## Local Two-Locale Dev Run

Create two databases, using your local PostgreSQL setup:

```powershell
createdb ck_world_dev
createdb ck_castle_rock_dev
```

Start World API:

```powershell
cd C:\Dev\Sites\collaborative-keystone\collaborative-keystone

$castleRockEntry = @'
[{
  "locale": {
    "slug": "castle-rock",
    "name": "Castle Rock",
    "locale_type": "municipality"
  },
  "web_origin": "http://localhost:5174",
  "api_origin": "http://localhost:8081",
  "operator": {
    "name": "Castle Rock operator",
    "contact": "ops@example.test"
  },
  "registry_status": "development",
  "trust_tier": "development",
  "release_id": null,
  "source_repository_url": "https://github.com/River-Sage/collaborative-keystone",
  "provenance_manifest_path": "/.well-known/keystone-build.json",
  "instance_public_key": null,
  "last_verified_at": null
}]
'@

.\scripts\Start-CkLocaleApi.ps1 `
  -DatabaseUrl "postgres://ck_app:change-me@localhost:5432/ck_world_dev" `
  -LocaleSlug "world" `
  -LocaleName "World" `
  -LocaleType "world" `
  -Port 8080 `
  -WebOrigin "http://localhost:5173" `
  -ApiOrigin "http://localhost:8080" `
  -RegistryStatus "canonical" `
  -DeploymentKind "canonical" `
  -DeploymentStatus "canonical" `
  -TrustTier "development" `
  -GlobalRegistryOrigin "http://localhost:5173" `
  -LocaleRegistryJson $castleRockEntry
```

Start World web:

```powershell
cd C:\Dev\Sites\collaborative-keystone\collaborative-keystone
.\scripts\Start-CkLocaleWeb.ps1 -ApiBaseUrl "http://localhost:8080" -Port 5173
```

Start Castle Rock API:

```powershell
cd C:\Dev\Sites\collaborative-keystone\collaborative-keystone

.\scripts\Start-CkLocaleApi.ps1 `
  -DatabaseUrl "postgres://ck_app:change-me@localhost:5432/ck_castle_rock_dev" `
  -LocaleSlug "castle-rock" `
  -LocaleName "Castle Rock" `
  -LocaleType "municipality" `
  -Port 8081 `
  -WebOrigin "http://localhost:5174" `
  -ApiOrigin "http://localhost:8081" `
  -RegistryStatus "development" `
  -DeploymentKind "locale" `
  -DeploymentStatus "development" `
  -TrustTier "development" `
  -GlobalRegistryOrigin "http://localhost:5173"
```

Start Castle Rock web:

```powershell
cd C:\Dev\Sites\collaborative-keystone\collaborative-keystone
.\scripts\Start-CkLocaleWeb.ps1 -ApiBaseUrl "http://localhost:8081" -Port 5174
```

World should expose Castle Rock from:

```text
http://localhost:8080/.well-known/keystone-locales.json
```

The World login screen also shows a compact locale directory when the registry contains more than one locale with a web origin.

## Seed A Locale

Set the same locale env used by the API, then run the seeder:

```powershell
cd C:\Dev\Sites\collaborative-keystone\collaborative-keystone\site\api

$env:DATABASE_URL = "postgres://ck_app:change-me@localhost:5432/ck_castle_rock_dev"
$env:CK_LOCALE_SLUG = "castle-rock"
$env:CK_LOCALE_NAME = "Castle Rock"
$env:CK_LOCALE_TYPE = "municipality"
cargo run --bin seed_demo
```

The seeder creates or updates the configured locale and seeds demo proposals into that locale's active cycle.

## First Moderator Bootstrap

On a fresh deployment, set a one-time token before the first API launch:

```powershell
$env:CK_BOOTSTRAP_MODERATOR_TOKEN = "replace-with-a-random-32-character-minimum-token"
```

Then call:

```powershell
Invoke-RestMethod `
  -Uri "http://localhost:8081/bootstrap/first-moderator" `
  -Method Post `
  -ContentType "application/json" `
  -Body (@{
    email = "moderator@example.org"
    password = "replace-with-a-real-password"
    bootstrap_token = $env:CK_BOOTSTRAP_MODERATOR_TOKEN
  } | ConvertTo-Json)
```

Remove `CK_BOOTSTRAP_MODERATOR_TOKEN` from the runtime environment after bootstrap.

## Metadata Smoke Check

Check World:

```powershell
.\scripts\Test-CkLocaleMetadata.ps1 `
  -ApiOrigin "http://localhost:8080" `
  -ExpectedLocaleSlug "world" `
  -ExpectedLocaleName "World" `
  -ExpectedRegistryStatus "canonical" `
  -ExpectedRegistryEntrySlug "castle-rock"
```

Check Castle Rock:

```powershell
.\scripts\Test-CkLocaleMetadata.ps1 `
  -ApiOrigin "http://localhost:8081" `
  -ExpectedLocaleSlug "castle-rock" `
  -ExpectedLocaleName "Castle Rock" `
  -ExpectedRegistryStatus "development"
```

## Production Minimum

Before a locale is listed as `authorized`, `official`, or `verified`, it should have:

- a separate production database
- production `https://` web and API origins
- `APP_ENV=production`
- development helper env vars disabled
- first moderator bootstrap completed and token removed
- visible Source, AGPL, Build Info, and Registry links
- public `/source-info`, `/.well-known/keystone-build.json`, and `/.well-known/keystone-locales.json`
- operator identity/contact configured
- an operator agreement accepted before official branding
- signed release provenance before strong verification claims

Unsigned or modified community deployments remain allowed under AGPL, but they should be listed or presented as `community`, `unverified`, or `development`, not official.
