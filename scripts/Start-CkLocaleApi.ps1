param(
    [Parameter(Mandatory = $true)]
    [string]$DatabaseUrl,

    [string]$LocaleSlug = "world",
    [string]$LocaleName = "World",
    [string]$LocaleType = "world",
    [int]$Port = 8080,
    [string]$HostName = "127.0.0.1",
    [string]$WebOrigin = "http://localhost:5173",
    [string]$ApiOrigin = "http://localhost:8080",
    [string]$RegistryStatus = "development",
    [string]$DeploymentKind = "local",
    [string]$DeploymentStatus = "development",
    [string]$TrustTier = "development",
    [string]$GlobalRegistryOrigin = "http://localhost:5173",
    [string]$OperatorName = "",
    [string]$OperatorContact = "",
    [string]$BootstrapModeratorToken = "",
    [string]$LocaleRegistryJson = ""
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$apiRoot = Join-Path $repoRoot "site\api"

$env:DATABASE_URL = $DatabaseUrl
$env:HOST = $HostName
$env:PORT = "$Port"
$env:PUBLIC_WEB_ORIGIN = $WebOrigin
$env:PUBLIC_API_ORIGIN = $ApiOrigin
$env:WEB_ORIGIN = $WebOrigin
$env:CK_LOCALE_SLUG = $LocaleSlug
$env:CK_LOCALE_NAME = $LocaleName
$env:CK_LOCALE_TYPE = $LocaleType
$env:CK_REGISTRY_STATUS = $RegistryStatus
$env:CK_DEPLOYMENT_KIND = $DeploymentKind
$env:CK_DEPLOYMENT_STATUS = $DeploymentStatus
$env:CK_TRUST_TIER = $TrustTier
$env:CK_GLOBAL_REGISTRY_ORIGIN = $GlobalRegistryOrigin

if ($OperatorName.Trim()) {
    $env:CK_OPERATOR_NAME = $OperatorName
}

if ($OperatorContact.Trim()) {
    $env:CK_OPERATOR_CONTACT = $OperatorContact
}

if ($BootstrapModeratorToken.Trim()) {
    $env:CK_BOOTSTRAP_MODERATOR_TOKEN = $BootstrapModeratorToken
}

if ($LocaleRegistryJson.Trim()) {
    $env:CK_LOCALE_REGISTRY_JSON = $LocaleRegistryJson
}

Push-Location $apiRoot
try {
    cargo run
}
finally {
    Pop-Location
}
