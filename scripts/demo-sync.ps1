param([string]$Output)
$ErrorActionPreference = "Stop"
if (-not $Output) {
    $Output = Join-Path ([System.IO.Path]::GetTempPath()) ("rust-mirror-demo-" + [guid]::NewGuid().ToString("N"))
}
if (Test-Path -LiteralPath $Output) { throw "Choose a new demo output directory." }
$demoRoot = [System.IO.Path]::GetFullPath($Output)
New-Item -ItemType Directory -Path $demoRoot | Out-Null
$previousDemoRoot = $env:RUST_MIRROR_DEMO_ROOT
$env:RUST_MIRROR_DEMO_ROOT = $demoRoot
Push-Location (Split-Path -Parent $PSScriptRoot)
try {
    Write-Host "SYNTHETIC FIXTURE DEMO: no Rust distribution downloads or installs."
    Write-Host "Shows initial sync, explicit refusal, resume, unchanged repeat, and destructive force."
    & cargo test --locked --test sync demo_tiny_sync -- --exact --nocapture
    if ($LASTEXITCODE -ne 0) { throw "Demo failed; preserved output: $demoRoot" }
    Write-Host "Reports: $demoRoot/first/.sync/report.toml and $demoRoot/interrupted/.sync/report.toml"
} finally {
    Pop-Location
    $env:RUST_MIRROR_DEMO_ROOT = $previousDemoRoot
}
