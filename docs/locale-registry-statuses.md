# Locale Registry Statuses

Global Keystone is the trusted entry point for locale discovery. Direct locale URLs may exist, but users should treat a locale as trusted only through the global registry path.

Every running locale API should expose a local self-report at:

```text
/.well-known/keystone-locales.json
```

The global registry should eventually sign its own registry entries. A locale's self-report is useful for inspection, but the global registry's signed status is the trust source.

Current v1 bridge: the World API can publish additional entries from `CK_LOCALE_REGISTRY_JSON`. This gives the canonical site a working locale directory before the full signed registry service exists. Operators and users should still treat unsigned configured entries as lower trust than future signed registry entries.

## Statuses

### canonical

The project-operated global entry point and reference deployment. Official branding is allowed.

### official

A project-operated or project-authorized deployment using official branding. Official branding is allowed.

### authorized

A locale operator accepted by the project and allowed to use approved `{Locale} Keystone` branding. Official branding is allowed.

### verified

A locale deployment whose operator agreement, release provenance, registry check-in, and public metadata are current. Official branding is allowed while verification remains current.

### stale

Previously verified, but the registry has not seen a fresh check-in, release check, or operator confirmation. Official branding should be paused or warned until refreshed.

### warning

Listed, but users should see a warning before relying on the deployment. Use for policy concerns, minor provenance drift, pending operator review, or non-critical operational problems.

### suspended

Temporarily removed from official trust while an operator, compliance, security, or brand issue is resolved.

### compromised

Evidence suggests the instance, keys, operator account, database, deployment, or artifacts have been compromised. The registry should warn users and remove official trust immediately.

### abandoned

No responsible operator is maintaining the locale deployment. The registry may keep historical references but should not present it as active or official.

### community

An independent fork or deployment. It may use AGPL code, but it is not official and may not use official branding.

### unverified

The registry has not verified the deployment's release, operator, source, or brand status.

### development

A local or test deployment with no public trust claim.

## Minimum Registry Entry

Each registry entry should include:

- locale slug
- locale display name
- locale type
- public web origin
- public API origin
- operator name/contact
- registry status
- trust tier
- release ID
- source repository URL
- provenance manifest path
- public instance verification key, if present
- last verified timestamp
- official branding allowance
- brand claim

## Trust Tiers

- `development`: local or test instance
- `unsigned`: public or staged instance without signed release provenance
- `signed-release`: signed release/build manifest is available
- `signed-release-reproducible`: signed release plus reproducible-build verification

## Takedown And Recovery

The global registry should be able to:

- mark stale deployments automatically after missed registry check-ins
- mark warning/suspended/compromised after operator or security review
- remove abandoned deployments from normal discovery
- preserve historical audit records after removal
- accept operator recovery evidence
- rotate or revoke public instance keys
- require upgrade away from revoked releases

Registry changes should be auditable and, once signing exists, signed by the global registry authority.
