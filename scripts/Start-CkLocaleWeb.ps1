param(
    [string]$ApiBaseUrl = "http://localhost:8080",
    [int]$Port = 5173,
    [string]$LocaleName = "World",
    [string]$BasePath = "/",
    [string]$CsrfCookieName = "ck_csrf"
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$webRoot = Join-Path $repoRoot "site\web"

$env:VITE_API_BASE_URL = $ApiBaseUrl
$env:VITE_LOCALE_NAME = $LocaleName
$env:VITE_BASE_PATH = $BasePath
$env:VITE_CSRF_COOKIE_NAME = $CsrfCookieName

Push-Location $webRoot
try {
    npm run dev -- --port $Port
}
finally {
    Pop-Location
}
