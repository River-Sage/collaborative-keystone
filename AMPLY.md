# Collaborative Keystone Deployment Brief

Paste this into a deployment assistant/chat that already knows the Cloudflare/server setup. The goal is to deploy the current Collaborative Keystone prototype safely, with the web app at `https://collaborativekeystone.com` and the API at `https://api.collaborativekeystone.com`.

## Project Summary

Collaborative Keystone is a Rust API plus Vite/React web app.

Repository root:

```text
C:\Dev\Sites\collaborative-keystone\collaborative-keystone
```

Important paths:

```text
site/api                 Rust API
site/db/migrations       PostgreSQL migrations
site/web                 Vite/React web app
site/web/dist            Production web build output
docs/deployment.md       Deployment notes
scripts/smoke-production.ps1
```

The deployment target is a single machine exposed through Cloudflare Tunnel.

Recommended public origins:

```text
Web: https://collaborativekeystone.com
API: https://api.collaborativekeystone.com
```

Recommended local services:

```text
API local bind: 127.0.0.1:8080
Web local service: static server serving site/web/dist
Database: PostgreSQL reachable only from local/private network
```

The API should not be directly reachable from the public internet outside the tunnel/trusted proxy, because rate limiting trusts Cloudflare proxy headers first.

## Runtime Architecture

Cloudflare Tunnel should route:

```text
collaborativekeystone.com      -> local static web service serving site/web/dist
api.collaborativekeystone.com  -> http://127.0.0.1:8080
```

The exact `cloudflared` commands/config should be verified against current Cloudflare docs or the server's existing tunnel configuration. The important behavior is:

- Browser loads the React app from `collaborativekeystone.com`.
- React calls the API at `api.collaborativekeystone.com`.
- API sets cookie auth with `Secure` cookies in production.
- API allows credentialed CORS only from the configured web origin.

## API Production Environment

Set production API env vars through the service manager, not committed `.env` files.

Required:

```text
DATABASE_URL=postgres://...
HOST=127.0.0.1
PORT=8080
APP_ENV=production
WEB_ORIGIN=https://collaborativekeystone.com
PUBLIC_WEB_ORIGIN=https://collaborativekeystone.com
MAIL_MODE=smtp
MAIL_FROM_EMAIL=no-reply@collaborativekeystone.com
MAIL_SMTP_HOST=127.0.0.1
MAIL_SMTP_PORT=25
```

Optional/conditional:

```text
CORS_ALLOWED_ORIGINS=https://collaborativekeystone.com
MAIL_FROM_NAME=Collaborative Keystone
MAIL_SMTP_HELO_NAME=collaborativekeystone.com
MAIL_SMTP_USERNAME=...
MAIL_SMTP_PASSWORD=...
RUST_LOG=api=info,tower_http=info
```

If only one web origin is allowed, `WEB_ORIGIN` is enough. If multiple origins are needed, use `CORS_ALLOWED_ORIGINS` as a comma-separated list.

Production guards currently enforced by the API:

- `WEB_ORIGIN`, `PUBLIC_WEB_ORIGIN`, and `CORS_ALLOWED_ORIGINS` entries must use `https://` in production.
- Production requires either `CORS_ALLOWED_ORIGINS` or `WEB_ORIGIN`.
- `MAIL_MODE=log` is forbidden in production.
- Session cookies include `Secure` when `APP_ENV=production` or `RUST_ENV=production`.
- Development auth token exposure is disabled in production.
- Development account seeding is disabled in production.
- Request body limit is 1 MiB.

## Web Production Environment

Before building the web app for production:

```text
VITE_API_BASE_URL=https://api.collaborativekeystone.com
```

Build from `site/web`:

```powershell
npm install
npm run build
```

Serve `site/web/dist` with the local static web service that Cloudflare Tunnel points to.

Do not serve Vite dev server publicly for production.

## Database

The API runs SQL migrations on startup from:

```text
site/db/migrations
```

Production database considerations:

- Use PostgreSQL.
- Use a fresh production database/password.
- Do not reuse the local development `.env` blindly.
- Ensure the database user has enough permissions to run migrations at API startup.
- Later, migration permissions can be split into a separate deployment role, but the current prototype expects startup migrations to work.

## Email

The API has a pluggable mailer:

```text
MAIL_MODE=log   development only
MAIL_MODE=smtp  production
```

For the no-third-party path, run a local mail transfer agent/SMTP relay on the server and point the API at it:

```text
MAIL_SMTP_HOST=127.0.0.1
MAIL_SMTP_PORT=25
```

The local mail server should handle:

- outbound delivery
- queueing
- TLS to receiving mail servers
- DKIM signing
- bounce handling
- sender reputation

DNS/email deliverability still needs:

- SPF
- DKIM
- DMARC
- reverse DNS/PTR if sending directly from the server

If SMTP/DNS is not ready, the app can be deployed as a private prototype, but public registration should not be treated as production-ready until email verification reliably delivers.

## Auth And Security Behavior To Verify

The API currently uses:

- cookie-based sessions
- `ck_session` HttpOnly cookie
- `ck_csrf` readable CSRF cookie
- `X-CSRF-Token` required for authenticated non-GET requests
- SHA-256 hashed session tokens in the database
- SHA-256 hashed email verification tokens
- SHA-256 hashed password reset tokens
- Argon2 password hashing
- in-memory auth rate limiting
- Cloudflare-aware client identity using `CF-Connecting-IP` first

Public unauthenticated POST paths:

```text
/auth/login
/auth/register
/auth/password-reset/request
/auth/password-reset/confirm
```

Authenticated POST requests should include:

```text
X-CSRF-Token: <value of ck_csrf cookie>
```

Because rate limiting trusts proxy headers, do not expose the API directly to arbitrary public clients outside the Cloudflare tunnel/trusted proxy.

## Development Accounts

Local development has seeded debug accounts:

```text
user@example.com       User
moderator@example.com  Moderator
test2@example.com      User
```

Development password:

```text
SuperSecurePass123
```

Important:

- These are for local debug builds only.
- Backend seeding is compiled only for debug builds and also disabled when `APP_ENV=production` or `RUST_ENV=production`.
- The frontend dev account helper is hidden in production builds.
- Production should use real registration/users, not seeded accounts.

## Files And Folders Not To Ship As Source

Do not commit/publish/copy as trusted production source:

```text
site/api/.env
site/api/target
site/api/target-codex
site/api/*.log
site/api/.codex-api-logs
site/web/node_modules
site/web/dist, unless intentionally copying only the built static output
site/web/*.log
```

Recreate local state on the target machine instead:

- new API env vars
- fresh DB password
- `npm install`
- `npm run build`
- Rust build/run on target

## Suggested Deployment Flow

1. Confirm the project folder is on the target machine.
2. Create/configure production PostgreSQL.
3. Set API production env vars in the service manager.
4. Build the web app with `VITE_API_BASE_URL=https://api.collaborativekeystone.com`.
5. Start the API bound to `127.0.0.1:8080`.
6. Serve `site/web/dist` locally.
7. Configure Cloudflare Tunnel routing:
   - `collaborativekeystone.com` to local web static server
   - `api.collaborativekeystone.com` to `http://127.0.0.1:8080`
8. Confirm the API is not publicly reachable except through the tunnel.
9. Run the smoke script.
10. Create/register a normal smoke user and run the login smoke check.
11. Do a manual UX pass.

## Smoke Script

From the repo root, after the tunnel is live:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\smoke-production.ps1 `
  -WebOrigin https://collaborativekeystone.com `
  -ApiOrigin https://api.collaborativekeystone.com
```

This verifies:

- web origin responds
- API `/health` responds with `ok`
- CORS allows credentialed requests from the web origin
- oversized JSON requests return `413`

To also verify login cookies and CSRF, create or choose a normal non-moderator smoke account:

```powershell
$env:CK_SMOKE_EMAIL = "smoke@example.com"
$env:CK_SMOKE_PASSWORD = "use-a-real-smoke-password"
powershell -ExecutionPolicy Bypass -File scripts\smoke-production.ps1 `
  -WebOrigin https://collaborativekeystone.com `
  -ApiOrigin https://api.collaborativekeystone.com
```

The login smoke check verifies:

- login works
- `ck_session` is set
- `ck_session` has `Secure`, `HttpOnly`, and `SameSite=Lax`
- `ck_csrf` is set
- `ck_csrf` has `Secure`
- `ck_csrf` is readable by the frontend, so it must not be `HttpOnly`
- `/auth/me` works with the session
- CSRF-protected logout succeeds with `X-CSRF-Token`

## Manual UX Pass After Smoke

Run through these as a normal user and moderator:

- register account
- verify email
- request password reset
- confirm password reset
- log in/log out
- submit issue
- submit solution only when allowed by cycle state
- review enough items to unlock voting
- vote support/not fit/unclear/unsafe
- cast targeted merge vote
- submit distinction note only when merge relationship exists
- moderator review queue behavior
- freeze/unfreeze/archive/reviewed-active behavior
- appeal archived item
- reconsider cycle-closed archived item
- execute merge and confirm lower total count item archives into higher total count item
- confirm transferred sentiment votes behave correctly
- close/resolve cycle outcome
- confirm all cycle items archive at cycle close
- copy archived item into a new draft/submission
- create/update execution record for winning solution

## Current Prototype Limits

- Auth rate limiting is in-memory and resets when the API restarts.
- Rate limits are per API process, acceptable for single-machine prototype but not multi-instance production.
- Email delivery depends on the local SMTP relay and DNS reputation.
- Moderation and process mechanics are intentionally v1/prototype scope.
- Cloudflare Tunnel command syntax/config should be checked against the target machine's current Cloudflare setup.

## Important Product Requirements To Preserve

The app's legitimacy depends on process integrity:

- No admin role should be able to override the democratic process.
- Moderators should only act inside the intended moderation/review flows.
- Items that no longer meet moderation criteria after the 72-hour window should leave moderator control and return to the proper public bucket.
- Merges should archive the lower-total-count item into the higher-total-count item when either side meets merge threshold with the other as the target.
- Sentiment votes from the archived merge source transfer only when that voter has not already voted on the surviving target.
- If a voter voted on both merged items, the source-side vote is discarded instead of overriding the target-side vote.
- At cycle close, all cycle items archive; users can copy archived items into new submissions for a future cycle.
- First cycle has issue selection first; solution flow depends on a prior winning issue.

