param(
    [Parameter(Mandatory = $true)]
    [string]$LocaleName,

    [string]$LocaleType = "municipality",
    [string]$CountryCode = "",
    [string]$RegionCode = "",
    [string]$RegionName = "",
    [string]$ParentName = "",
    [string]$ParentSlug = "",
    [string]$OperatorName = "",
    [string]$OperatorContact = "",
    [string]$PublicWebOrigin = "",
    [string]$PublicApiOrigin = "",
    [ValidateSet("canonical", "official", "authorized", "verified", "community", "unverified", "development")]
    [string]$RegistryStatus = "unverified",
    [ValidateSet("development", "unsigned", "signed-release", "signed-release-reproducible")]
    [string]$TrustTier = "unsigned"
)

$ErrorActionPreference = "Stop"

function ConvertTo-CkSlug {
    param([string]$Value)

    $normalized = $Value.Trim().ToLowerInvariant() -replace "[^a-z0-9]+", "-"
    $normalized = $normalized.Trim("-") -replace "-+", "-"
    if (-not $normalized) {
        throw "Value cannot be converted to a locale slug."
    }
    return $normalized
}

function ConvertTo-CkCountryCode {
    param([string]$Value)

    $trimmed = $Value.Trim().ToUpperInvariant()
    if (-not $trimmed) {
        return ""
    }
    if ($trimmed.Length -ne 2 -or $trimmed -notmatch "^[A-Z]{2}$") {
        throw "CountryCode must be an ISO 3166-1 alpha-2 code, such as US."
    }
    return $trimmed
}

$country = ConvertTo-CkCountryCode $CountryCode
$region = $RegionCode.Trim().ToUpperInvariant()
$nameSlug = ConvertTo-CkSlug $LocaleName
$typeSlug = ConvertTo-CkSlug $LocaleType
$parentKeyPart = if ($ParentName.Trim()) { ConvertTo-CkSlug $ParentName } elseif ($ParentSlug.Trim()) { ConvertTo-CkSlug $ParentSlug } else { "" }

$canonicalParts = @($country.ToLowerInvariant(), $region.ToLowerInvariant(), $parentKeyPart, $nameSlug) |
    Where-Object { $_ }
$canonicalKey = ($canonicalParts -join "-")
if (-not $canonicalKey) {
    $canonicalKey = $nameSlug
}

$slugParts = @($nameSlug, $region.ToLowerInvariant(), $country.ToLowerInvariant()) |
    Where-Object { $_ }
$slug = ($slugParts -join "-")

$qualifierParts = @()
if ($RegionName.Trim()) {
    $qualifierParts += $RegionName.Trim()
} elseif ($region) {
    $qualifierParts += $region
}
if ($country) {
    $qualifierParts += $country
}
$displayQualifier = $qualifierParts -join ", "

$locale = [ordered]@{
    slug = $slug
    name = $LocaleName.Trim()
    locale_type = $typeSlug
    canonical_key = $canonicalKey
}
if ($displayQualifier) { $locale.display_qualifier = $displayQualifier }
if ($country) { $locale.country_code = $country }
if ($region) { $locale.region_code = $region }
if ($RegionName.Trim()) { $locale.region_name = $RegionName.Trim() }
if ($ParentSlug.Trim()) { $locale.parent_locale_slug = ConvertTo-CkSlug $ParentSlug }

$registryEntry = [ordered]@{
    locale = $locale
    web_origin = if ($PublicWebOrigin.Trim()) { $PublicWebOrigin.Trim() } else { $null }
    api_origin = if ($PublicApiOrigin.Trim()) { $PublicApiOrigin.Trim() } else { $null }
    operator = [ordered]@{
        name = if ($OperatorName.Trim()) { $OperatorName.Trim() } else { $null }
        contact = if ($OperatorContact.Trim()) { $OperatorContact.Trim() } else { $null }
    }
    registry_status = $RegistryStatus
    trust_tier = $TrustTier
    release_id = $null
    source_repository_url = "https://github.com/River-Sage/collaborative-keystone"
    provenance_manifest_path = "/.well-known/keystone-build.json"
    instance_public_key = $null
    last_verified_at = $null
}

[pscustomobject]@{
    locale_slug = $slug
    locale_name = $LocaleName.Trim()
    locale_type = $typeSlug
    canonical_key = $canonicalKey
    display_qualifier = $displayQualifier
    api_environment = [ordered]@{
        CK_LOCALE_SLUG = $slug
        CK_LOCALE_NAME = $LocaleName.Trim()
        CK_LOCALE_TYPE = $typeSlug
        CK_LOCALE_CANONICAL_KEY = $canonicalKey
        CK_LOCALE_DISPLAY_QUALIFIER = $displayQualifier
        CK_LOCALE_COUNTRY_CODE = $country
        CK_LOCALE_REGION_CODE = $region
        CK_LOCALE_REGION_NAME = $RegionName.Trim()
        CK_LOCALE_PARENT_SLUG = if ($ParentSlug.Trim()) { ConvertTo-CkSlug $ParentSlug } else { "" }
    }
    web_environment = [ordered]@{
        VITE_LOCALE_NAME = $LocaleName.Trim()
    }
    registry_entry_json = ($registryEntry | ConvertTo-Json -Depth 10)
}
