[CmdletBinding()]
param(
    [string]$ApiOrigin = $env:CK_API_ORIGIN,
    [string]$WebOrigin = $env:CK_WEB_ORIGIN,
    [string]$Email = $env:CK_SMOKE_EMAIL,
    [string]$Password = $env:CK_SMOKE_PASSWORD,
    [string]$ExpectedLocaleSlug = $env:CK_EXPECTED_LOCALE_SLUG,
    [string]$ExpectedLocaleName = $env:CK_EXPECTED_LOCALE_NAME,
    [string]$ExpectedRegistryStatus = $env:CK_EXPECTED_REGISTRY_STATUS,
    [switch]$SkipLogin,
    [switch]$SkipOversizedBody
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Write-Step {
    param([string]$Message)
    Write-Host "[check] $Message" -ForegroundColor Cyan
}

function Write-Pass {
    param([string]$Message)
    Write-Host "[pass]  $Message" -ForegroundColor Green
}

function Write-Info {
    param([string]$Message)
    Write-Host "[info]  $Message" -ForegroundColor Yellow
}

function Fail-Smoke {
    param([string]$Message)
    throw "[fail]  $Message"
}

function Assert-Smoke {
    param(
        [bool]$Condition,
        [string]$Message
    )

    if (-not $Condition) {
        Fail-Smoke $Message
    }
}

function Normalize-Origin {
    param(
        [string]$Value,
        [string]$Name,
        [string]$DefaultValue
    )

    if ([string]::IsNullOrWhiteSpace($Value)) {
        $Value = $DefaultValue
    }

    $Value = $Value.Trim().TrimEnd("/")
    Assert-Smoke ($Value.StartsWith("https://") -or $Value.StartsWith("http://localhost") -or $Value.StartsWith("http://127.0.0.1")) "$Name must be an origin such as https://example.com."

    return $Value
}

function Get-HeaderValues {
    param(
        $Headers,
        [string]$Name
    )

    foreach ($key in $Headers.Keys) {
        if ([string]::Equals([string]$key, $Name, [System.StringComparison]::OrdinalIgnoreCase)) {
            $value = $Headers[$key]
            if ($value -is [array]) {
                return @($value)
            }

            return @([string]$value)
        }
    }

    return @()
}

function Get-FirstHeaderValue {
    param(
        $Headers,
        [string]$Name
    )

    $values = @(Get-HeaderValues $Headers $Name)
    if ($values.Count -eq 0) {
        return ""
    }

    return [string]$values[0]
}

function Invoke-SmokeRequest {
    param(
        [string]$Uri,
        [string]$Method = "GET",
        [hashtable]$Headers = @{},
        [string]$Body = "",
        [Microsoft.PowerShell.Commands.WebRequestSession]$WebSession = $null
    )

    $parameters = @{
        Uri = $Uri
        Method = $Method
        Headers = $Headers
        UseBasicParsing = $true
        TimeoutSec = 30
    }

    if ($Body -ne "") {
        $parameters["Body"] = $Body
        $parameters["ContentType"] = "application/json"
    }

    if ($null -ne $WebSession) {
        $parameters["WebSession"] = $WebSession
    }

    return Invoke-WebRequest @parameters
}

function Get-SmokeStatusCode {
    param(
        [string]$Uri,
        [string]$Method,
        [hashtable]$Headers,
        [string]$Body
    )

    try {
        $response = Invoke-SmokeRequest -Uri $Uri -Method $Method -Headers $Headers -Body $Body
        return [int]$response.StatusCode
    } catch [System.Net.WebException] {
        if ($_.Exception.Response) {
            return [int]$_.Exception.Response.StatusCode
        }

        throw
    }
}

$ApiOrigin = Normalize-Origin $ApiOrigin "ApiOrigin" "https://api.collaborativekeystone.com"
$WebOrigin = Normalize-Origin $WebOrigin "WebOrigin" "https://collaborativekeystone.com"
$ApiUri = [Uri]$ApiOrigin

Write-Host "Collaborative Keystone production smoke" -ForegroundColor White
Write-Info "Web origin: $WebOrigin"
Write-Info "API origin: $ApiOrigin"

Write-Step "Web origin responds"
$webResponse = Invoke-SmokeRequest -Uri $WebOrigin
Assert-Smoke ($webResponse.StatusCode -ge 200 -and $webResponse.StatusCode -lt 400) "Web origin returned HTTP $($webResponse.StatusCode)."
Assert-Smoke ($webResponse.Content -match "<html|id=`"root`"|/assets/") "Web origin responded, but it does not look like the built app shell."
Write-Pass "Web origin returned HTTP $($webResponse.StatusCode)."

Write-Step "API health responds"
$healthResponse = Invoke-SmokeRequest -Uri "$ApiOrigin/health"
Assert-Smoke ($healthResponse.StatusCode -eq 200) "API health returned HTTP $($healthResponse.StatusCode)."
Assert-Smoke ($healthResponse.Content.Trim() -eq "ok") "API health body should be 'ok'."
Write-Pass "API /health returned 200 ok."

Write-Step "Public source and provenance metadata respond"
$sourceInfoResponse = Invoke-SmokeRequest -Uri "$ApiOrigin/source-info"
Assert-Smoke ($sourceInfoResponse.StatusCode -eq 200) "API /source-info returned HTTP $($sourceInfoResponse.StatusCode)."
$sourceInfo = $sourceInfoResponse.Content | ConvertFrom-Json
Assert-Smoke ($sourceInfo.ok -eq $true) "/source-info did not return ok=true."
Assert-Smoke (-not [string]::IsNullOrWhiteSpace([string]$sourceInfo.source_repository_url)) "/source-info did not include source_repository_url."
Assert-Smoke (-not [string]::IsNullOrWhiteSpace([string]$sourceInfo.license.name)) "/source-info did not include license.name."
Assert-Smoke ($sourceInfo.registry_manifest_path -eq "/.well-known/keystone-locales.json") "/source-info did not advertise the locale registry manifest path."

$provenanceResponse = Invoke-SmokeRequest -Uri "$ApiOrigin/.well-known/keystone-build.json"
Assert-Smoke ($provenanceResponse.StatusCode -eq 200) "API /.well-known/keystone-build.json returned HTTP $($provenanceResponse.StatusCode)."
$provenance = $provenanceResponse.Content | ConvertFrom-Json
Assert-Smoke ($provenance.schema_version -eq "keystone-build-provenance/v1") "Build provenance schema_version was '$($provenance.schema_version)'."
Assert-Smoke (-not [string]::IsNullOrWhiteSpace([string]$provenance.source_repository_url)) "Build provenance did not include source_repository_url."
Assert-Smoke (-not [string]::IsNullOrWhiteSpace([string]$provenance.signature_status)) "Build provenance did not include signature_status."
if (-not [string]::IsNullOrWhiteSpace($ExpectedLocaleSlug)) {
    Assert-Smoke ($provenance.locale.slug -eq $ExpectedLocaleSlug) "Build provenance locale slug was '$($provenance.locale.slug)', expected '$ExpectedLocaleSlug'."
}
if (-not [string]::IsNullOrWhiteSpace($ExpectedLocaleName)) {
    Assert-Smoke ($provenance.locale.name -eq $ExpectedLocaleName) "Build provenance locale name was '$($provenance.locale.name)', expected '$ExpectedLocaleName'."
}
if (-not [string]::IsNullOrWhiteSpace($ExpectedRegistryStatus)) {
    Assert-Smoke ($provenance.registry_status -eq $ExpectedRegistryStatus) "Build provenance registry status was '$($provenance.registry_status)', expected '$ExpectedRegistryStatus'."
}

$registryResponse = Invoke-SmokeRequest -Uri "$ApiOrigin/.well-known/keystone-locales.json"
Assert-Smoke ($registryResponse.StatusCode -eq 200) "API /.well-known/keystone-locales.json returned HTTP $($registryResponse.StatusCode)."
$registry = $registryResponse.Content | ConvertFrom-Json
Assert-Smoke ($registry.schema_version -eq "keystone-locale-registry/v1") "Locale registry schema_version was '$($registry.schema_version)'."
Assert-Smoke ($registry.entries.Count -ge 1) "Locale registry did not include any entries."
Assert-Smoke (-not [string]::IsNullOrWhiteSpace([string]$registry.entries[0].registry_status)) "Locale registry entry did not include registry_status."
if (-not [string]::IsNullOrWhiteSpace($ExpectedLocaleSlug)) {
    Assert-Smoke ($registry.generated_for.slug -eq $ExpectedLocaleSlug) "Locale registry generated_for slug was '$($registry.generated_for.slug)', expected '$ExpectedLocaleSlug'."
}
Write-Pass "Source, provenance, and registry metadata returned expected public fields."

Write-Step "CORS preflight allows the configured web origin"
$preflightHeaders = @{
    Origin = $WebOrigin
    "Access-Control-Request-Method" = "POST"
    "Access-Control-Request-Headers" = "content-type,x-csrf-token"
}
$preflightResponse = Invoke-SmokeRequest -Uri "$ApiOrigin/auth/login" -Method "OPTIONS" -Headers $preflightHeaders
$allowOrigin = Get-FirstHeaderValue $preflightResponse.Headers "Access-Control-Allow-Origin"
$allowCredentials = Get-FirstHeaderValue $preflightResponse.Headers "Access-Control-Allow-Credentials"
Assert-Smoke ($allowOrigin -eq $WebOrigin) "CORS allowed origin should be '$WebOrigin' but was '$allowOrigin'."
Assert-Smoke ($allowCredentials.ToLowerInvariant() -eq "true") "CORS must allow credentials for cookie auth."
Write-Pass "CORS preflight allows credentials for $WebOrigin."

if (-not $SkipOversizedBody) {
    Write-Step "Oversized request bodies are rejected"
    $bigPassword = "x" * (1024 * 1024 + 2048)
    $oversizedEmail = "oversized-smoke-$([DateTimeOffset]::UtcNow.ToUnixTimeSeconds())@example.com"
    $oversizedBody = @{ email = $oversizedEmail; password = $bigPassword } | ConvertTo-Json -Compress
    $oversizedStatus = Get-SmokeStatusCode -Uri "$ApiOrigin/auth/login" -Method "POST" -Headers @{ Origin = $WebOrigin } -Body $oversizedBody
    Assert-Smoke ($oversizedStatus -eq 413) "Oversized login request should return 413, but returned HTTP $oversizedStatus."
    Write-Pass "Oversized JSON request returned 413."
}

if ($SkipLogin) {
    Write-Info "Skipping login/cookie/CSRF checks because -SkipLogin was provided."
} elseif ([string]::IsNullOrWhiteSpace($Email) -or [string]::IsNullOrWhiteSpace($Password)) {
    Write-Info "Skipping login/cookie/CSRF checks. Set CK_SMOKE_EMAIL and CK_SMOKE_PASSWORD, or pass -Email and -Password, to enable them."
} else {
    Write-Step "Login sets secure session and CSRF cookies"
    $session = New-Object Microsoft.PowerShell.Commands.WebRequestSession
    $loginBody = @{ email = $Email; password = $Password } | ConvertTo-Json -Compress
    $loginResponse = Invoke-SmokeRequest -Uri "$ApiOrigin/auth/login" -Method "POST" -Headers @{ Origin = $WebOrigin } -Body $loginBody -WebSession $session
    Assert-Smoke ($loginResponse.StatusCode -eq 200) "Login returned HTTP $($loginResponse.StatusCode)."

    $setCookies = Get-HeaderValues $loginResponse.Headers "Set-Cookie"
    $sessionCookieHeader = $setCookies | Where-Object { $_ -like "ck_session=*" } | Select-Object -First 1
    $csrfCookieHeader = $setCookies | Where-Object { $_ -like "ck_csrf=*" } | Select-Object -First 1

    Assert-Smoke (-not [string]::IsNullOrWhiteSpace($sessionCookieHeader)) "Login did not set ck_session."
    Assert-Smoke (-not [string]::IsNullOrWhiteSpace($csrfCookieHeader)) "Login did not set ck_csrf."
    Assert-Smoke ($sessionCookieHeader -match "(?i);\s*Secure") "ck_session must include Secure in production."
    Assert-Smoke ($sessionCookieHeader -match "(?i);\s*HttpOnly") "ck_session must include HttpOnly."
    Assert-Smoke ($sessionCookieHeader -match "(?i);\s*SameSite=Lax") "ck_session must include SameSite=Lax."
    Assert-Smoke ($csrfCookieHeader -match "(?i);\s*Secure") "ck_csrf must include Secure in production."
    Assert-Smoke ($csrfCookieHeader -notmatch "(?i);\s*HttpOnly") "ck_csrf must remain readable by the frontend."
    Write-Pass "Login cookies include the expected production attributes."

    Write-Step "Authenticated request succeeds"
    $meResponse = Invoke-SmokeRequest -Uri "$ApiOrigin/auth/me" -Headers @{ Origin = $WebOrigin } -WebSession $session
    Assert-Smoke ($meResponse.StatusCode -eq 200) "Authenticated /auth/me returned HTTP $($meResponse.StatusCode)."
    Write-Pass "/auth/me returned 200 with the smoke session."

    Write-Step "CSRF-protected logout succeeds"
    $csrfCookie = $session.Cookies.GetCookies($ApiUri) | Where-Object { $_.Name -eq "ck_csrf" } | Select-Object -First 1
    Assert-Smoke ($null -ne $csrfCookie -and -not [string]::IsNullOrWhiteSpace($csrfCookie.Value)) "Could not read ck_csrf from the smoke session."
    $logoutHeaders = @{
        Origin = $WebOrigin
        "X-CSRF-Token" = $csrfCookie.Value
    }
    $logoutResponse = Invoke-SmokeRequest -Uri "$ApiOrigin/auth/logout" -Method "POST" -Headers $logoutHeaders -Body "{}" -WebSession $session
    Assert-Smoke ($logoutResponse.StatusCode -eq 200) "Logout returned HTTP $($logoutResponse.StatusCode)."
    Write-Pass "CSRF-protected logout returned 200."
}

Write-Host ""
Write-Host "Smoke checks complete." -ForegroundColor Green
