# Locale Identity And Registry

World Keystone is the trusted place to discover locale sites.

The goal is not to restrict which communities can use Keystone. The goal is to make sure one real-world place has one official registry identity.

## Identity Fields

Every locale has:

- `name`: the simple display name, such as `Castle Rock`
- `locale_type`: the kind of locale, such as `municipality`, `county`, `state`, `country`, `region`, `campus`, or `community`
- `slug`: the URL-safe public identifier for this registry entry
- `canonical_key`: the stable identity for the real-world place
- `display_qualifier`: the disambiguating label shown in the directory, such as `Colorado, US`
- optional jurisdiction fields: `country_code`, `region_code`, `region_name`, and `parent_locale_slug`

The brand can stay simple:

```text
Castle Rock Keystone
```

The directory should disambiguate:

```text
Castle Rock Keystone
Colorado, US
```

## Canonical Key

The `canonical_key` is what prevents accidental duplicate locales.

For civic locales, use a normalized place path:

```text
{country}-{region}-{parent-place-if-needed}-{place-name}
```

Examples:

```text
us-co-douglas-county-castle-rock
us-wi-adams-county-castle-rock
us-co-denver
us-co-douglas-county
world
```

`Castle Rock`, `Castle Rock, CO`, and `Castle Rock, Colorado, USA` should resolve to the same canonical key when they refer to the same place.

If two places genuinely share a display name, they must have different canonical keys and visible qualifiers.

## Registry Rule

The World registry must not list two active entries with the same canonical key.

If someone requests a locale that matches an existing canonical key:

- do not create a second official locale
- treat the request as a duplicate or alias
- point the requester to the existing locale
- optionally record the new spelling as an alias for search later

If someone requests a locale with the same display name but a different jurisdiction:

- allow it
- require a different canonical key
- require a visible display qualifier
- show the qualifier in locale search and the Locales dropdown

## No Artificial Locale Restriction

Keystone should not limit people to only cities, counties, or countries.

Allowed locale types can include civic places, regions, neighborhoods, campuses, communities, organizations, or other clearly defined scopes.

The requirement is clarity, not a specific government category:

- the locale must have a unique canonical key
- the public directory must make it clear what scope the site covers
- official or verified branding requires World registry approval

## Standard Launch Flow

1. Choose the locale name and scope.
2. Generate a canonical key with `scripts/New-CkLocaleIdentity.ps1`.
3. Search the World registry for the canonical key and likely aliases.
4. If the key exists, use the existing locale instead of creating a duplicate.
5. If the display name exists in another jurisdiction, keep the name but add a qualifier.
6. Deploy the locale as either a World-hosted path, a World subdomain, or an independent community deployment.
7. Verify health, provenance, source/license links, and moderator bootstrap.
8. Add the locale to the World registry with its canonical key and trust status.

## Hosting Choices

World-operated locales should use the main World Keystone identity/session store so one login works across World-operated locales.

Independent operators should use isolated databases until a formal World Keystone sign-in federation flow exists.

Supported public routing shapes:

- path-hosted: `https://worldkeystone.com/locales/castle-rock/`
- subdomain-hosted: `https://castle-rock.worldkeystone.com`
- independent public origin: `https://example-community-keystone.org`

All official or verified locales must still be discoverable through World Keystone.
