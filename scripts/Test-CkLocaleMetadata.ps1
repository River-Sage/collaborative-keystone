param(
    [string]$ApiOrigin = "http://localhost:8080",
    [string]$ExpectedLocaleSlug = "world",
    [string]$ExpectedLocaleName = "World",
    [string]$ExpectedRegistryStatus = "development",
    [string]$ExpectedRegistryEntrySlug = ""
)

$ErrorActionPreference = "Stop"

function Assert-CkCondition {
    param(
        [bool]$Condition,
        [string]$Message
    )

    if (-not $Condition) {
        throw $Message
    }
}

$trimmedApiOrigin = $ApiOrigin.TrimEnd("/")

$sourceInfo = Invoke-RestMethod -Uri "$trimmedApiOrigin/source-info" -Method Get
$buildInfo = Invoke-RestMethod -Uri "$trimmedApiOrigin/.well-known/keystone-build.json" -Method Get
$registry = Invoke-RestMethod -Uri "$trimmedApiOrigin/.well-known/keystone-locales.json" -Method Get

Assert-CkCondition $sourceInfo.ok "source-info did not return ok=true."
Assert-CkCondition ($buildInfo.locale.slug -eq $ExpectedLocaleSlug) "build locale slug was '$($buildInfo.locale.slug)', expected '$ExpectedLocaleSlug'."
Assert-CkCondition ($buildInfo.locale.name -eq $ExpectedLocaleName) "build locale name was '$($buildInfo.locale.name)', expected '$ExpectedLocaleName'."
Assert-CkCondition ($buildInfo.registry_status -eq $ExpectedRegistryStatus) "build registry status was '$($buildInfo.registry_status)', expected '$ExpectedRegistryStatus'."
Assert-CkCondition ($registry.generated_for.slug -eq $ExpectedLocaleSlug) "registry generated_for slug was '$($registry.generated_for.slug)', expected '$ExpectedLocaleSlug'."
Assert-CkCondition ($registry.entries.Count -ge 1) "registry did not include any entries."
Assert-CkCondition (-not $registry.registry_config_error) "registry config error: $($registry.registry_config_error)"

if ($ExpectedRegistryEntrySlug.Trim()) {
    $matchingEntry = $registry.entries | Where-Object { $_.locale.slug -eq $ExpectedRegistryEntrySlug } | Select-Object -First 1
    Assert-CkCondition ($null -ne $matchingEntry) "registry did not include expected entry '$ExpectedRegistryEntrySlug'."
}

[pscustomobject]@{
    ok = $true
    api_origin = $trimmedApiOrigin
    locale_slug = $buildInfo.locale.slug
    locale_name = $buildInfo.locale.name
    registry_status = $buildInfo.registry_status
    trust_tier = $buildInfo.trust_tier
    registry_entry_count = $registry.entries.Count
}
