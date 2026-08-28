# Release Signing And Provenance

This document records the practical trust model for Keystone releases and locale distribution.

## Product Decision

Keystone Core remains AGPL-only for now.

The project may later add managed hosting, certified locale operation, a trademark license, a certified appliance, or dual licensing, but those are separate product/legal decisions. They should not be assumed by the v1 implementation.

## Trust Layers

- Keystone Core: public AGPL source, requirements, migrations, and docs.
- Global Keystone: project-operated canonical entry point and trusted locale registry.
- Locale Keystone Distribution: official signed release package for operators.
- Locale Keystone Instance: a running locale deployment with its own database, domains, secrets, moderators, and records.
- Community Fork: an independent deployment that may use AGPL code but is not official.

## Signing Target

The release signing target is:

- Sigstore Cosign for release archive, manifest, and container signatures
- GitHub Actions OIDC or an equivalent CI identity for keyless signing
- SLSA-style provenance from CI as the long-term standard
- signed registry entries and deployment attestations

The repository includes workflow scaffolding in `.github/workflows/release-provenance.yml`. The workflow builds the API and web app, computes artifact digests, emits `keystone-build.json`, signs it with Cosign when run in GitHub Actions, and uploads the manifest/signature bundle as an artifact.

## Public Manifest Paths

Every running API should expose:

```text
/source-info
/.well-known/keystone-build.json
/.well-known/keystone-locales.json
```

The web UI should expose a visible Source & Trust entry point on public/login surfaces and in the Account view. That surface should provide Source Code, AGPL License, Build Details, and Locale Data links while keeping raw registry/status vocabulary behind technical links.

## Build Manifest Fields

The build provenance manifest should include:

- schema version
- product
- deployment kind
- deployment status
- registry status
- trust tier
- source repository URL
- git commit SHA
- release ID
- build timestamp
- build environment
- web artifact digest
- API artifact digest
- migration set digest
- configuration digest
- signature or Sigstore bundle reference
- public verification key or keyless identity
- signature status
- locale identity
- operator identity

## Locale Registry Manifest

The locale registry manifest should include:

- schema version
- registry origin
- locale slug, display name, and type
- web origin
- API origin
- operator/contact identity
- registry status
- trust tier
- release ID
- source repository URL
- provenance manifest path
- public instance verification key
- last verified timestamp
- official branding allowance
- brand claim

Direct locale URLs may exist, but users should discover and trust locale deployments through Global Keystone and its signed registry.

Current v1 bridge:

- every locale API self-reports one entry for its configured locale
- the World API may publish additional configured entries from `CK_LOCALE_REGISTRY_JSON`
- configured entries are useful for launch, testing, and operator setup
- configured entries are not equivalent to signed registry authority
- once the signed registry exists, Global Keystone's signed registry entries become the trust source

## Key Custody

Official release signing authority stays under project control.

Acceptable custody models:

- Sigstore keyless signing through project-controlled CI identity
- project-controlled cloud KMS
- hardware-backed signing key with limited operator access

Locale operators do not receive official signing keys. They may generate their own instance keys for deployment attestations. Only public instance keys should be registered with Global Keystone.

## Rotation And Revocation

The project should be able to:

- rotate official signing identities
- revoke compromised keys or CI identities
- publish old and new verification material during transition
- mark affected releases or registry entries as warning, suspended, or compromised
- require operators to upgrade away from revoked releases

Registry entries should record enough release and instance metadata to warn users when a deployment is stale, compromised, or abandoned.

## Reproducible Builds

Reproducible builds are a later trust tier, not a launch blocker.

The initial public path is signed release artifacts, pinned dependencies, lockfiles, and CI provenance. Once deterministic builds are practical, the project can add a `signed-release-reproducible` trust tier for builds that independent auditors can reproduce byte-for-byte.

## Brand And License Boundary

AGPL rights let people use, study, modify, and redistribute Keystone Core. Those rights do not grant the official name, official visual identity, canonical registry status, or official locale branding.

Official branding is reserved for Global Keystone and authorized locale instances that satisfy the operator agreement and registry requirements. Community forks must clearly identify themselves as independent.

## Launch Minimum

Before encouraging broad locale distribution, the project should have:

- visible source/license/build/registry UI
- public build and registry metadata endpoints
- signed release manifest workflow
- operator agreement template reviewed for real use
- trademark boundaries documented
- registry status definitions documented
- smoke tests that verify public metadata endpoints

Before presenting locale instances as strongly verified, the project should add:

- signed global registry entries
- registry check-in verification
- incident/takedown process
- formal key rotation/revocation process
- reproducible build tier, if practical
