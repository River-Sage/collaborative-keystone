param(
    [string]$ApiBaseUrl = "http://localhost:8080",
    [int]$Port = 5173
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$webRoot = Join-Path $repoRoot "site\web"

$env:VITE_API_BASE_URL = $ApiBaseUrl

Push-Location $webRoot
try {
    npm run dev -- --port $Port
}
finally {
    Pop-Location
}
