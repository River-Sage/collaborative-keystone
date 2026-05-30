# Deployment Notes

This project is moving toward a first working production prototype. The current deployment target is a single dev/prod machine exposed through Cloudflare Tunnel.

## Recommended Shape

- Web public origin: `https://collaborativekeystone.com`
- API public origin: `https://api.collaborativekeystone.com`
- API local bind: `127.0.0.1:8080`
- Web local service: a static web server serving `site/web/dist`
- Database: PostgreSQL reachable only from the machine or private network

Keeping the API bound to `127.0.0.1` lets the tunnel expose it without opening the API port directly to the public internet.

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
- `WEB_ORIGIN=https://collaborativekeystone.com`
- `PUBLIC_WEB_ORIGIN=https://collaborativekeystone.com`
- `MAIL_MODE=smtp`
- `MAIL_FROM_EMAIL=no-reply@collaborativekeystone.com`
- `MAIL_SMTP_HOST=127.0.0.1`
- `MAIL_SMTP_PORT=25`

Web build:

- `VITE_API_BASE_URL=https://api.collaborativekeystone.com`

If the API is exposed on more than one allowed web origin, use `CORS_ALLOWED_ORIGINS` instead of `WEB_ORIGIN`.

Production `WEB_ORIGIN`, `PUBLIC_WEB_ORIGIN`, and all `CORS_ALLOWED_ORIGINS` entries must use `https://`.

## Startup Behavior

The API runs database migrations on startup from `site/db/migrations`.

That keeps the dev-machine move simple, but the database user needs enough migration permissions. Later, production can split this into a separate migration role if desired.

## Email Delivery

The API has a pluggable mailer:

- `MAIL_MODE=log` logs verification and password reset emails in development.
- `MAIL_MODE=smtp` sends verification and password reset emails to an SMTP relay.

For a no-third-party production path, run a local mail transfer agent on the server and point the API at it with `MAIL_SMTP_HOST=127.0.0.1` and `MAIL_SMTP_PORT=25`.

The local mail server, not the web app, should handle outbound delivery, queueing, TLS to receiving mail servers, DKIM signing, bounce handling, and reputation. DNS still needs SPF, DKIM, DMARC, and reverse DNS/PTR configured for the sending host.

The API does not implement STARTTLS itself in this prototype. Use a local trusted relay for production rather than sending credentials over the network.

## Cloudflare Tunnel Notes

The intended tunnel routing is:

- `collaborativekeystone.com` to the local web service
- `api.collaborativekeystone.com` to the local API service

Before creating the tunnel, verify the current `cloudflared` commands against Cloudflare's docs. The target architecture above is the important part; exact commands can be confirmed during setup.

Because auth rate limiting trusts `CF-Connecting-IP` first, the API should not be directly reachable by public clients outside the tunnel or trusted proxy.

## Production Smoke Check

After the tunnel is live, run the smoke script from the repository root:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\smoke-production.ps1 `
  -WebOrigin https://collaborativekeystone.com `
  -ApiOrigin https://api.collaborativekeystone.com
```

That verifies the web origin, API health endpoint, CORS credentials behavior, and the 1 MiB request body limit.

To also verify login cookies and CSRF behavior, create or choose a low-privilege smoke account, then run:

```powershell
$env:CK_SMOKE_EMAIL = "smoke@example.com"
$env:CK_SMOKE_PASSWORD = "use-a-real-smoke-password"
powershell -ExecutionPolicy Bypass -File scripts\smoke-production.ps1 `
  -WebOrigin https://collaborativekeystone.com `
  -ApiOrigin https://api.collaborativekeystone.com
```

The smoke account should be a normal user, not a moderator. The script does not print the password.

## Pre-Launch Checklist

- Build the web app with the production `VITE_API_BASE_URL`.
- Start the API with `APP_ENV=production`.
- Confirm `/health` returns `200` through the tunnel.
- Confirm login cookies include `Secure`.
- Confirm login sets both `ck_session` and `ck_csrf`, and authenticated `POST` requests include `X-CSRF-Token`.
- Confirm CORS allows the web origin and rejects unrelated origins.
- Confirm oversized JSON requests are rejected; the API request body limit is 1 MiB.
- Confirm database migrations have run.
- Confirm development token exposure is disabled.
- Confirm development account seeding is disabled.
- Configure local SMTP relay delivery before public signups depend on email verification.

## Known Prototype Limits

- Auth rate limiting is in-memory and resets on API restart.
- Rate limits are per API process, which is acceptable for a single-machine prototype but not a multi-instance deployment.
- Session cookies, email verification tokens, and password reset tokens are stored as hashes in the database. Plaintext auth tokens exist only in browser cookies, email/log delivery output, and development-only API responses where explicitly enabled.
- Cookie-authenticated `POST` requests require the `ck_csrf` cookie value to be echoed in the `X-CSRF-Token` header. Public login, registration, and password reset entry points are exempt so stale sessions can recover cleanly.
