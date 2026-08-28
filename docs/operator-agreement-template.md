# Keystone Locale Operator Agreement Template

This is a working template for future official or authorized locale operators. It is product guidance, not legal advice. Before public rollout, counsel should convert it into a real agreement.

## Parties

- Project steward: the operator of Global Keystone and the official Collaborative Keystone brand.
- Locale operator: the person or organization approved to run a locale-specific Keystone deployment.
- Locale instance: the running `{Locale Display Name} Keystone` deployment listed in the global registry.

## Scope

The operator may run Keystone Core under the repository's AGPL license whether or not this agreement exists. This agreement only covers official status, registry listing, approved `{Locale} Keystone` branding, certification language, operator duties, and trust requirements.

The agreement does not transfer ownership of Keystone Core, Global Keystone, the registry, signing keys, trademarks, logos, official seals, or canonical domains.

## Authorization

An operator may use official or authorized locale branding only after the project approves the locale and records the operator in the global registry.

Approval should identify:

- locale slug and display name
- locale type
- public web origin
- public API origin
- operator legal/display name
- operator contact address
- registry status
- allowed brand treatment
- current signed release or approved source commit
- public instance verification key, when available

## Required Branding Rules

Authorized operators may use the `{Locale Display Name} Keystone` pattern only for the approved locale and approved deployment.

The following remain reserved:

- Collaborative Keystone as the canonical project identity
- Global Keystone as the canonical registry and entry point
- official seals, checkmarks, certification marks, and logo treatments
- official wording that implies project operation rather than authorized operation
- the canonical global domain and registry identity

Community forks may truthfully say they are forks of Collaborative Keystone or powered by Keystone Core. They must not look or sound like official or authorized Keystone deployments.

## Source, License, And Provenance Duties

Every authorized locale instance must visibly offer:

- Source link
- AGPL/license link
- Build Info link
- Registry/provenance link

The running API must expose:

- `/source-info`
- `/.well-known/keystone-build.json`
- `/.well-known/keystone-locales.json`

Operators must not use encryption, packaging, appliance deployment, private forks, or obfuscation to block source availability required by the AGPL.

## Release And Configuration Duties

Authorized operators should run official signed releases with only approved locale/environment configuration changes unless the project grants an exception.

Operators must not receive official project signing keys. They may generate instance keys for deployment attestations and register the public key with Global Keystone.

Operators should keep:

- release identifier
- git commit SHA
- artifact digests
- migration set digest
- configuration digest
- build timestamp
- provenance signature or bundle
- instance public key

## Registry Check-In

The locale instance should periodically prove liveness and release status to the global registry.

The registry may mark an instance:

- verified
- stale
- warning
- suspended
- compromised
- abandoned
- community
- unverified

Loss of verified status may remove official branding rights until resolved.

## Operations Duties

Operators are expected to:

- keep the instance online within reasonable limits
- apply security updates
- preserve audit logs
- maintain moderator accountability
- avoid manual database changes that bypass required flows
- protect backups and private runtime secrets
- publish accurate operator contact information
- respond to registry verification and incident requests

## Moderation And Audit Duties

Operators and moderators must follow the public requirements. They may not secretly change voting thresholds, cycle rules, archive behavior, unlock rules, anti-abuse thresholds, or implementation completion rules while claiming official status.

If a locale needs different policy, it should be documented as an approved locale rule variant or presented as a community fork.

## Security Incidents

Operators must promptly report:

- leaked credentials
- compromised hosting accounts
- compromised instance keys
- unauthorized database access
- modified release artifacts
- hidden rule changes
- loss of operational control

The project may mark the registry entry warning, suspended, compromised, or abandoned while the incident is investigated.

## Revocation

The project may revoke official or authorized status for:

- brand misuse
- hiding source/license information
- refusing provenance checks
- running modified code while claiming official status
- security compromise
- abandoned operations
- misleading users
- repeated failure to follow the requirements

Revocation affects official status and branding rights. It does not remove AGPL software rights.

## Open Legal Items

Before this template is used externally, counsel should review trademark language, operator liability, privacy obligations, moderation obligations, data retention, takedown process, revocation appeals, source-offer wording, and any managed-hosting or certified-appliance program.
