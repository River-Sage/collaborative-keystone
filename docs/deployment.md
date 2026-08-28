# Deployment Notes

This project is moving toward a first working production prototype. The current deployment target is a single small production machine exposed through Cloudflare Tunnel.

## Recommended Shape

- Web public origin: `https://worldkeystone.com`
- API public origin: `https://worldkeystone.com/api`
- API local bind: `127.0.0.1:8080`
- Web local service: a static web server serving `site/web/dist`
- Database: PostgreSQL reachable only from the machine or private network

Keeping the API bound to `127.0.0.1` lets nginx proxy same-origin `/api/*` requests to the API without opening the API port directly to the public internet.

Recommended first server:

- one Linux VPS with 2 shared vCPU, 4 GB RAM, and 80 GB+ SSD
- Ubuntu LTS or Debian stable
- local PostgreSQL 17/18 on the same private server
- Caddy or nginx serving `site/web/dist` on `127.0.0.1`
- Rust API release binary on `127.0.0.1:8080`
- Cloudflare Tunnel for `worldkeystone.com`, with nginx proxying `/api/*` to the local API
- daily database backups plus off-server encrypted backup copies

This is enough for an early public launch because the app's hot path is mostly PostgreSQL-backed form submission, queue loading, voting, and indexed lookups. Do not start with Kubernetes, autoscaling, or multi-region databases. Move only when measured load says to move.

Scale-up path:

- first: resize the VPS vertically
- then: move PostgreSQL to a managed or dedicated database with automated backups and point-in-time recovery
- then: add PgBouncer or provider pooling if connection pressure becomes visible
- then: split API and web onto separate machines or containers
- later: shard by locale, because the product model already treats each locale as an independently configured deployment

Database expectations:

- Keep one production database per locale deployment unless there is a deliberate multi-tenant operator reason not to.
- Keep proposal, vote, review, comment, and audit-history tables append-friendly. Archive rather than delete.
- Watch indexes on `locale_id`, `cycle_id`, `board_id`, proposal state, vote user/proposal uniqueness, comment proposal, and audit event time.
- Keep migrations explicit and tested before production rollout.
- Use off-server backups before launch. A public governance app without tested restores is not live-ready.

## Environment Files

API local development:

1. Copy `site/api/.env.example` to `site/api/.env`.
2. Set `DATABASE_URL` to the local PostgreSQL database.
3. Run the API from `site/api` with `cargo run`.

Web local development:

1. Copy `site/web/.env.example` to `site/web/.env.local` if the API is not on `http://localhost:8080`.
2. Run the web app from `site/web` with `npm run dev`.

Production should set environment variables through the service manager rather than committing real secrets into `.env` files.

## Dev-Machine Move Hygiene

When moving the project folder to another machine, copy source files and recreate local state there:

- Do not commit or publish `site/api/.env`, logs, `target`, `target-codex`, `node_modules`, or `site/web/dist`.
- Copy `site/api/.env.example` to a fresh `site/api/.env` on the new machine and set a new database password.
- Reinstall web dependencies with `npm install` from `site/web`.
- Rebuild the web app on the target machine instead of carrying an old `dist` folder forward.
- Build or run the Rust API on the target machine instead of carrying `target` or `target-codex` forward.
- If Git reports dubious ownership after the move, only mark the new folder as safe after confirming the path is the trusted project checkout.

The development accounts use a known prototype password for local testing. They are seeded only in debug builds and are disabled when `APP_ENV=production` or `RUST_ENV=production` is set. Production should still use a release build and real user registration rather than seeded accounts.

## Required Production Variables

API:

- `DATABASE_URL`
- `HOST=127.0.0.1`
- `PORT=8080`
- `APP_ENV=production`
- `WEB_ORIGIN=https://worldkeystone.com`
- `PUBLIC_WEB_ORIGIN=https://worldkeystone.com`
- `PUBLIC_API_ORIGIN=https://worldkeystone.com/api`
- `MAIL_MODE=resend`
- `MAIL_FROM_EMAIL=no-reply@worldkeystone.com`
- `MAIL_RESEND_API_KEY`
- `CF_TURNSTILE_SECRET_KEY`

Web build:

- `VITE_API_BASE_URL=https://worldkeystone.com/api`
- `VITE_TURNSTILE_SITE_KEY`
- `VITE_PATREON_URL`

If the API is exposed on more than one allowed web origin, use `CORS_ALLOWED_ORIGINS` instead of `WEB_ORIGIN`.

Production `WEB_ORIGIN`, `PUBLIC_WEB_ORIGIN`, and all `CORS_ALLOWED_ORIGINS` entries must use `https://`.

Do not enable `VITE_SHOW_PROTOTYPE_ACCOUNTS` on public deployments. Prototype account buttons are explicit local-demo opt-in only.

Public account creation and password-reset request forms should use Cloudflare Turnstile when the site is live. The frontend site key is public. The API secret key must stay server-side in `CF_TURNSTILE_SECRET_KEY`. The API requires and validates Turnstile tokens whenever the secret is configured.

## Localized Deployments

The project supports a simple configured path for operators to launch a locale-specific instance from the public repository.

The current working shape is:

- clone the repository
- choose an official signed release or community source checkout
- choose a locale slug and display name
- set the public web and API origins
- run migrations and initial seeding
- create the first moderator through a one-time bootstrap flow
- build the web app
- publish the deployment with clear official/community status

Localized deployments should use the in-app brand pattern `{Locale Display Name} Keystone`, such as `World Keystone`, `Castle Rock Keystone`, or `Douglas County Keystone`.

Locale setup is driven by configuration and helper scripts rather than source edits. Required configuration includes the locale slug, display name, locale type, operator/contact identity, web origin, API origin, deployment status, registry status, and trust tier.

Current locale identity variables:

- `CK_LOCALE_SLUG`
- `CK_LOCALE_NAME`
- `CK_LOCALE_TYPE`
- `PUBLIC_WEB_ORIGIN`
- `PUBLIC_API_ORIGIN`
- `CK_GLOBAL_REGISTRY_ORIGIN`
- `CK_REGISTRY_STATUS`
- `CK_DEPLOYMENT_KIND`
- `CK_DEPLOYMENT_STATUS`
- `CK_TRUST_TIER`
- `CK_OPERATOR_NAME`
- `CK_OPERATOR_CONTACT`

On startup, the API creates or updates the configured locale row and opens that locale's active cycle if none exists. Core proposal, review, voting, archive, outcome, appeal, reconsideration, discussion, and implementation surfaces are scoped to the configured locale.

The central global Keystone site should be the user-facing entry point for accessing locale deployments. Users should choose or search for locale instances from the global site rather than having to discover separate locale URLs directly.

The current global site can expose configured registry entries through `CK_LOCALE_REGISTRY_JSON`. This is suitable for development, staging, and early operator setup. The long-term global site should maintain signed locale registry entries containing each listed locale's display name, type, origins, operator/contact identity, deployment status, latest verified release, and provenance verification state.

Direct locale origins may still exist for hosting, operations, and deep links, but a locale deployment should be considered trusted only through the global registry path.

Community deployments must clearly distinguish themselves from the canonical official instance unless they are explicitly authorized to present as official.

See `docs/locale-registry-statuses.md` for the registry status contract and `docs/locale-operator-quickstart.md` for World/Castle Rock local launch commands.

## Locale Distribution Package

The intended easy-spin-up product is a signed Locale Keystone Distribution.

That distribution may take the form of a signed release archive, signed container image, or installer script that fetches signed artifacts. It should include:

- web build artifact or build instructions
- API binary or container image
- migration set
- setup script or wizard
- locale configuration template
- production environment template
- smoke test script
- provenance manifest
- source offer / repository link

The distribution must not include production secrets.

The setup flow should ask for:

- locale slug and display name
- locale type
- web origin
- API origin
- operator/contact identity
- database URL or managed database connection
- SMTP settings
- initial moderator email

After setup, the instance should expose health and provenance metadata so the global site can verify what it is.

## Secrets, Keys, and Encryption

The verification method should be public. The keys that sign or protect a specific deployment should be private.

Private deployment material belongs in environment variables, a secret manager, or an ignored local secret file. Examples include:

- database credentials
- session secret
- mail credentials
- instance signing private key
- backup encryption key
- one-time first-moderator bootstrap token

Encryption should protect private runtime material, backups, and operator handoff bundles. It should not be treated as a way to hide the AGPL application source or prevent forks from modifying their own deployments.

The trust model is:

- official releases are signed
- instances prove which release/configuration they run
- global registry marks verified, authorized, community, or unverified status
- modified forks remain allowed under the software license but are visibly not the canonical official deployment

## First Moderator Bootstrap

A fresh locale deployment should support a one-time bootstrap path for the first moderator.

Acceptable bootstrap shapes include:

- a local-console command on the deployment machine
- a temporary one-time token printed only during setup
- a setup wizard reachable only before the first moderator exists

After the first moderator is created, bootstrap access must be disabled and the action must be auditable.

The first moderator becomes the initial moderator-steward for that locale. They do not receive ownership of the official brand, the global registry, or the software license.

Current implementation:

- set `CK_BOOTSTRAP_MODERATOR_TOKEN` to a random 32+ character secret before first launch
- call `POST /bootstrap/first-moderator` with `email`, `password`, and `bootstrap_token`
- the endpoint refuses use when the token is missing, too short, wrong, or when any verified moderator already exists
- the created account is email-verified and assigned the `moderator` role
- the action is recorded in `deployment_audit_events`
- `/.well-known/keystone-build.json` reports `bootstrap.first_moderator_bootstrap_complete`
- remove `CK_BOOTSTRAP_MODERATOR_TOKEN` from the runtime environment after bootstrap

## Official Build Provenance

The canonical deployment should eventually expose a signed build provenance manifest from a stable public path such as:

```text
/.well-known/keystone-build.json
```

The current application exposes this manifest path from the API. Development builds may be unsigned, but production releases should populate the manifest from release/build environment variables.

The manifest should identify the source repository, git commit SHA, build timestamp, artifact digests, migration set digest, release identifier, and signature.

A digest proves that bytes match a known value. A signature proves that the known value was published by the official project key. The canonical deployment should therefore use signed manifests, not unsigned hashes alone.

Future deployment tooling should be able to verify:

- the manifest signature is valid
- the public key matches the official repository and official domain record
- the running build matches the signed artifact digests
- the migration set matches the signed migration digest
- the deployment declares whether it is canonical, authorized, or community-operated

Forks may run modified code under the project license, but the UI and deployment metadata should not let modified community instances masquerade as the canonical official deployment.

The current application also exposes:

```text
/source-info
/.well-known/keystone-locales.json
```

The first endpoint exposes source, license, and brand policy metadata. The second endpoint exposes a local registry self-report for the running locale. The signed global registry remains the future trust authority; a local self-report is inspection data.

See `docs/release-signing-and-provenance.md` for the signing/provenance target and `.github/workflows/release-provenance.yml` for the current Sigstore workflow scaffold.

## Current Decisions

- **License/product shape:** Keystone Core remains AGPL-only for now. Managed hosting, certified locale operation, and trademark authorization may be added later. Dual licensing is not part of the default path unless a concrete need appears.
- **Operator authorization:** Operators must sign or accept an operator agreement before using official/authorized `{Locale} Keystone` branding or appearing as verified in the global registry.
- **Signing system:** Target Sigstore Cosign for release/container signing and SLSA-style provenance from CI. Registry entries and deployment attestations should also be signed.
- **Key custody:** Official signing keys stay under project-controlled CI/KMS or hardware-backed custody. Locale operators do not receive official signing keys; they generate instance keys and register public keys with the global registry.
- **Reproducible builds:** Reproducible builds are desirable but not a launch blocker. Signed releases, lockfiles, pinned dependencies, and CI provenance come first. Reproducible builds can become a higher trust tier.
- **Registry status:** The global registry should visibly mark deployments as canonical, official, authorized, verified, stale, warning, suspended, compromised, abandoned, community, unverified, or development.
- **Source/license UI:** Every running web UI must offer visible Source, AGPL, Build Info, and Registry links. The current app shows them on the public login surface and Account view, backed by `/source-info`, `/.well-known/keystone-build.json`, and `/.well-known/keystone-locales.json`.
- **Visual identity:** The master name, logo, official seal/checkmark, global domain, and official wording are reserved. Verified locales may use approved `{Locale} Keystone` branding. Community forks may truthfully say they are forks or are powered by Keystone Core, but must not look official.

Concrete working references:

- `docs/operator-agreement-template.md`
- `docs/release-signing-and-provenance.md`
- `docs/locale-registry-statuses.md`
- `TRADEMARKS.md`

## Remaining Legal Review

Before large-scale public distribution, counsel should review the operator agreement, trademark policy, certification language, source-offer UX, registry takedown/appeal policy, and any future managed-hosting or dual-license offering.

## Startup Behavior

The API runs database migrations on startup from `site/db/migrations`.

That keeps the dev-machine move simple, but the database user needs enough migration permissions. Later, production can split this into a separate migration role if desired.

## Email Delivery

The API has a pluggable mailer:

- `MAIL_MODE=log` logs verification and password reset emails in development instead of sending real inbox messages. In debug/dev mode, the web UI can also receive local-only verification and reset tokens for testing.
- `MAIL_MODE=resend` sends verification and password reset emails through the Resend HTTPS API.
- `MAIL_MODE=smtp` sends verification and password reset emails to an SMTP relay.

Production cannot rely on logged tokens. Account verification and password resets require deliverable email.

For the first World Keystone launch on DigitalOcean, prefer Resend's HTTPS API because DigitalOcean blocks outbound SMTP ports `25`, `465`, and `587` on Droplets by default:

- `MAIL_MODE=resend`
- `MAIL_RESEND_API_KEY=<Resend API key>`
- `MAIL_RESEND_API_URL=https://api.resend.com/emails`
- `MAIL_FROM_EMAIL=no-reply@worldkeystone.com`

Resend requires a verified domain before sending from that domain. Add and verify `worldkeystone.com` in Resend, then add the DNS records Resend gives you in Cloudflare.

The API also supports secure SMTP submission through `MAIL_SMTP_SECURITY` for providers/environments where SMTP ports are allowed:

- `implicit_tls`: encrypted SMTP from the first byte, normally port `465`
- `starttls`: connect first, then require STARTTLS before credentials or mail are sent, normally port `587`
- `none`: unencrypted SMTP, allowed in production only for a local relay such as `127.0.0.1:25`

Postmark and similar providers may use `MAIL_SMTP_SECURITY=starttls` on port `587` instead.

For a no-third-party production path, run a local mail transfer agent on the server and point the API at it with `MAIL_SMTP_HOST=127.0.0.1`, `MAIL_SMTP_PORT=25`, and `MAIL_SMTP_SECURITY=none`.

For public launch, verify SPF, DKIM, and DMARC for the sending domain and complete a real registration/password-reset test from the live domain before inviting users.

When using a local mail server, that mail server should handle outbound delivery, queueing, TLS to receiving mail servers, DKIM signing, bounce handling, and reputation. When using a hosted SMTP provider, that provider normally handles those delivery concerns after the API submits the message securely. DNS still needs SPF, DKIM, and DMARC for the sending domain.

## Cloudflare Tunnel Notes

The intended tunnel routing is:

- `worldkeystone.com` to the local web service
- nginx proxies `/api/*` from the web service to the local API service, stripping the `/api` prefix before forwarding

Before creating the tunnel, verify the current `cloudflared` commands against Cloudflare's docs. The target architecture above is the important part; exact commands can be confirmed during setup.

Because auth rate limiting trusts `CF-Connecting-IP` first, the API should not be directly reachable by public clients outside the tunnel or trusted proxy.

## Production Smoke Check

After the tunnel is live, run the smoke script from the repository root:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\smoke-production.ps1 `
  -WebOrigin https://worldkeystone.com `
  -ApiOrigin https://worldkeystone.com/api `
  -ExpectedLocaleSlug world `
  -ExpectedLocaleName World `
  -ExpectedRegistryStatus canonical
```

That verifies the web origin, API health endpoint, public source/build/registry metadata, expected locale identity, CORS credentials behavior, and the 1 MiB request body limit.

To also verify login cookies and CSRF behavior, create or choose a low-privilege smoke account, then run:

```powershell
$env:CK_SMOKE_EMAIL = "smoke@example.com"
$env:CK_SMOKE_PASSWORD = "use-a-real-smoke-password"
powershell -ExecutionPolicy Bypass -File scripts\smoke-production.ps1 `
  -WebOrigin https://worldkeystone.com `
  -ApiOrigin https://worldkeystone.com/api `
  -ExpectedLocaleSlug world `
  -ExpectedLocaleName World `
  -ExpectedRegistryStatus canonical
```

The smoke account should be a normal user, not a moderator. The script does not print the password.

## Pre-Launch Checklist

- Build the web app with the production `VITE_API_BASE_URL`, preferably same-origin `/api` for the canonical site.
- Start the API with `APP_ENV=production`.
- Confirm `/health` returns `200` through the tunnel.
- Confirm login cookies include `Secure`.
- Confirm login sets both `ck_session` and `ck_csrf`, and authenticated `POST` requests include `X-CSRF-Token`.
- Confirm CORS allows the web origin and rejects unrelated origins.
- Confirm oversized JSON requests are rejected; the API request body limit is 1 MiB.
- Confirm database migrations have run.
- Confirm development token exposure is disabled.
- Confirm development account seeding is disabled.
- Confirm `/source-info`, `/.well-known/keystone-build.json`, and `/.well-known/keystone-locales.json` return public metadata.
- Confirm build and registry metadata report the expected locale slug, display name, and registry status.
- Confirm the web UI shows Source, AGPL, Build Info, and Registry links on login and Account surfaces.
- Configure local SMTP relay delivery before public signups depend on email verification.

## Known Prototype Limits

- Auth rate limiting is in-memory and resets on API restart.
- Rate limits are per API process, which is acceptable for a single-machine prototype but not a multi-instance deployment.
- Session cookies, email verification tokens, and password reset tokens are stored as hashes in the database. Plaintext auth tokens exist only in browser cookies, email/log delivery output, and development-only API responses where explicitly enabled.
- Cookie-authenticated `POST` requests require the `ck_csrf` cookie value to be echoed in the `X-CSRF-Token` header. Public login, registration, and password reset entry points are exempt so stale sessions can recover cleanly.
