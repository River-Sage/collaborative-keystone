# Locale Operator Quickstart

Keystone is intended to run one configured locale per deployment.

Examples:

- `World Keystone` is the canonical global entry point and locale registry.
- `Castle Rock Keystone` is a locale instance with its own locale identity, cycle history, audit trail, moderator bootstrap record, and implementation records.

Users should discover locale instances through World Keystone. Direct locale URLs may exist, but the trusted path is the World registry.

For the identity and duplicate-prevention model, start with `docs/locale-identity-and-registry.md`.

## Fast Path

1. Generate locale identity with `scripts/New-CkLocaleIdentity.ps1`.
2. Check the World registry for the generated `canonical_key`.
3. Choose hosting:
   - World-operated path: `https://worldkeystone.com/locales/{slug}/`
   - World subdomain: `https://{slug}.worldkeystone.com`
   - independent public origin
4. Configure API env from the generated locale identity.
5. Configure web env with `VITE_LOCALE_NAME`, `VITE_API_BASE_URL`, and optional `VITE_BASE_PATH`.
6. Start the API and web build.
7. Bootstrap the first moderator.
8. Run the metadata smoke check.
9. Add the locale to the World registry.
10. Confirm it appears in the World `Locales` dropdown.

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
$env:CK_LOCALE_CANONICAL_KEY = "us-co-douglas-county-castle-rock"
$env:CK_LOCALE_DISPLAY_QUALIFIER = "Colorado, US"
$env:CK_LOCALE_COUNTRY_CODE = "US"
$env:CK_LOCALE_REGION_CODE = "CO"
$env:CK_LOCALE_REGION_NAME = "Colorado"
$env:PUBLIC_WEB_ORIGIN = "https://castle-rock.example.org"
$env:PUBLIC_API_ORIGIN = "https://api.castle-rock.example.org"
$env:CK_GLOBAL_REGISTRY_ORIGIN = "https://collaborativekeystone.com"
$env:CK_REGISTRY_STATUS = "authorized"
```

The API upserts the configured locale on startup and opens that locale's active UTC calendar-month cycle if none exists.

Registry JSON should use `locale_type` for locale entries. The API also accepts `type` as a convenience alias for operator-written config.

Generate a starter identity:

```powershell
.\scripts\New-CkLocaleIdentity.ps1 `
  -LocaleName "Castle Rock" `
  -LocaleType "municipality" `
  -CountryCode "US" `
  -RegionCode "CO" `
  -RegionName "Colorado" `
  -ParentName "Douglas County"
```

Use the generated `canonical_key` to check for duplicate registry entries before creating the locale.

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
    "locale_type": "municipality",
    "canonical_key": "us-co-douglas-county-castle-rock",
    "display_qualifier": "Colorado, US",
    "country_code": "US",
    "region_code": "CO",
    "region_name": "Colorado"
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
  -LocaleCanonicalKey "us-co-douglas-county-castle-rock" `
  -LocaleDisplayQualifier "Colorado, US" `
  -LocaleCountryCode "US" `
  -LocaleRegionCode "CO" `
  -LocaleRegionName "Colorado" `
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
.\scripts\Start-CkLocaleWeb.ps1 `
  -ApiBaseUrl "http://localhost:8081" `
  -LocaleName "Castle Rock" `
  -Port 5174
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

- a production database plan: shared World identity/session database for World-operated SSO, or an isolated database for independent operators
- a unique `canonical_key` in the World registry
- a visible `display_qualifier` when the display name can be confused with another place
- production `https://` web and API origins
- `APP_ENV=production`
- development helper env vars disabled
- first moderator bootstrap completed and token removed
- a visible Source & Trust entry point with Source Code, AGPL License, Build Details, and Locale Data links
- public `/source-info`, `/.well-known/keystone-build.json`, and `/.well-known/keystone-locales.json`
- operator identity/contact configured
- an operator agreement accepted before official branding
- signed release provenance before strong verification claims

Unsigned or modified community deployments remain allowed under AGPL, but they should be listed or presented as `community`, `unverified`, or `development`, not official.

## One-Server Production Clone Pattern

The current production-friendly clone path is one code checkout with one runtime per locale:

- one API systemd service per locale
- one local nginx origin port per locale
- one public hostname per locale, routed through Cloudflare Tunnel
- one public registry entry on World Keystone for each active locale

For World-operated locales that should share sign-in, use one shared identity/session database and scope civic records by `locale_id`. For independently operated locales, use an isolated database until a formal World Keystone sign-in federation flow exists.

Example production layout:

| Locale | Public origin | Local web origin | Local API | Database |
| --- | --- | --- | --- | --- |
| World | `https://worldkeystone.com` | `127.0.0.1:8088` | `127.0.0.1:8080` | `collaborative_keystone_prod` |
| Castle Rock, World-operated SSO | `https://worldkeystone.com/locales/castle-rock/` | path under `127.0.0.1:8088` | `127.0.0.1:8081` | `collaborative_keystone_prod` |
| Independent Castle Rock operator | `https://castle-rock.worldkeystone.com` | `127.0.0.1:8089` | `127.0.0.1:8081` | `collaborative_keystone_castle_rock` |

If a new public hostname is not available yet, an early locale can also be proxied through the World hostname at a path such as `https://worldkeystone.com/locales/castle-rock/`.

For a World-operated path-hosted locale that should share login with World Keystone, use the same database, same cookie names, and root cookie path:

```bash
DATABASE_URL=postgres://.../collaborative_keystone_prod
CK_SESSION_COOKIE_NAME=ck_session
CK_CSRF_COOKIE_NAME=ck_csrf
CK_COOKIE_PATH=/
VITE_API_BASE_URL=/locales/castle-rock/api
VITE_BASE_PATH=/locales/castle-rock/
VITE_LOCALE_NAME='Castle Rock'
VITE_CSRF_COOKIE_NAME=ck_csrf
```

For an isolated path-hosted locale with separate accounts, use a separate database and distinct path-scoped cookies:

```bash
CK_SESSION_COOKIE_NAME=ck_cr_session
CK_CSRF_COOKIE_NAME=ck_cr_csrf
CK_COOKIE_PATH=/locales/castle-rock
VITE_API_BASE_URL=/locales/castle-rock/api
VITE_BASE_PATH=/locales/castle-rock/
VITE_LOCALE_NAME='Castle Rock'
VITE_CSRF_COOKIE_NAME=ck_cr_csrf
```

Each locale service should use the same release commit, but each locale gets its own environment file:

```text
/etc/world-keystone/api.env
/etc/world-keystone/web.env
/etc/castle-rock-keystone/api.env
/etc/castle-rock-keystone/web.env
```

For web builds, prefer same-origin API routing:

```text
VITE_API_BASE_URL=/api
```

That lets the same web app pattern work under each hostname. Nginx then routes `/api/*` for that hostname to the matching locale API port.

The locale API environment must identify the locale and public origins:

```bash
HOST=127.0.0.1
PORT=8081
APP_ENV=production
WEB_ORIGIN=https://castle-rock.worldkeystone.com
CORS_ALLOWED_ORIGINS=https://castle-rock.worldkeystone.com
PUBLIC_WEB_ORIGIN=https://castle-rock.worldkeystone.com
PUBLIC_API_ORIGIN=https://castle-rock.worldkeystone.com/api
CK_LOCALE_SLUG=castle-rock
CK_LOCALE_NAME='Castle Rock'
CK_LOCALE_TYPE=municipality
CK_DEPLOYMENT_KIND=locale
CK_DEPLOYMENT_STATUS=authorized
CK_REGISTRY_STATUS=verified
CK_TRUST_TIER=unsigned
CK_GLOBAL_REGISTRY_ORIGIN=https://worldkeystone.com
CK_OPERATOR_NAME='World Keystone'
CK_OPERATOR_CONTACT=ops@worldkeystone.com
```

Use the same mail and Turnstile provider settings as the canonical deployment unless the locale has its own verified mail domain and Turnstile widget.

After the service starts:

1. Confirm `https://castle-rock.worldkeystone.com/api/health` returns `ok`.
2. Confirm `https://castle-rock.worldkeystone.com/api/.well-known/keystone-build.json` reports `locale.slug = castle-rock`.
3. Confirm `https://castle-rock.worldkeystone.com/api/.well-known/keystone-locales.json` reports the Castle Rock locale.
4. Create or import the first verified moderator and record it in `deployment_audit_events`.
5. Extend the first cycle only if there is a launch exception, and record the old/new deadlines in `deployment_audit_events`.
6. Add Castle Rock to World Keystone's `CK_LOCALE_REGISTRY_JSON` and redeploy World.
7. Confirm World Keystone's public **Locales** dropdown lists Castle Rock and links to its canonical public origin. Path-hosted locale origins should include a trailing slash.

Cloudflare Tunnel must include one published application route per public hostname:

| Hostname | Service |
| --- | --- |
| `worldkeystone.com` | `http://localhost:8088` |
| `castle-rock.worldkeystone.com` | `http://localhost:8089` |

If the tunnel is dashboard-managed, this hostname route is created in the Cloudflare dashboard under the existing tunnel's **Published application routes**. If the tunnel is locally configured, add a matching ingress rule and restart `cloudflared`.
