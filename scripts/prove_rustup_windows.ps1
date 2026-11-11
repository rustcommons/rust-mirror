param([Parameter(Mandatory=$true)][string]$Generations,
      [Parameter(Mandatory=$true)][string]$Evidence,
      [string]$HostTriple = "x86_64-pc-windows-msvc",
      [switch]$ValidateOnly,
      [string]$PublisherCli)
$ErrorActionPreference = "Stop"
if ($env:GITHUB_ACTIONS -ne "true" -or $env:RUNNER_OS -ne "Windows" -or $env:RUNNER_ENVIRONMENT -ne "github-hosted") {
    throw "Firewall proof is restricted to disposable GitHub Windows runners."
}
$evidenceRoot = [IO.Path]::GetFullPath($Evidence)
if (Test-Path -LiteralPath $evidenceRoot) { throw "Choose a new evidence directory." }
$parent = Split-Path -Parent $evidenceRoot
New-Item -ItemType Directory -Force -Path $parent | Out-Null
$proof = "$evidenceRoot-firewall.json"
if (Test-Path -LiteralPath $proof) { throw "Firewall record exists." }
$group = "rust-mirror-lifecycle-" + [guid]::NewGuid().ToString("N")
$rules = @()
$profiles = @(Get-NetFirewallProfile | Select-Object Name,Enabled)
try {
    Set-NetFirewallProfile -Profile Domain,Private,Public -Enabled True
    $programs = @("rustup-init.exe", "cargo/bin/rustup.exe", "cargo/bin/rustup-init.exe", "cargo/bin/cargo.exe", "cargo/bin/rustc.exe") | ForEach-Object { [IO.Path]::GetFullPath((Join-Path $evidenceRoot $_)) }
    if ($PublisherCli) { $programs += [IO.Path]::GetFullPath($PublisherCli) }
    foreach ($program in $programs) {
        $ruleName = $group + "-" + $rules.Count
        New-NetFirewallRule -Name $ruleName -DisplayName $ruleName -Group $group -Direction Outbound -Action Block -Enabled True -Profile Any -Program $program -RemoteAddress Any | Out-Null
        $rules += @{ name=$ruleName; program=$program; remote="Any (loopback access must pass the live lifecycle proof)" }
    }
    @{disposable_runner=$true; rules=$rules; profiles=@(Get-NetFirewallProfile | Select-Object Name,Enabled)} | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8 -LiteralPath $proof
    if ($ValidateOnly) {
        if ($PublisherCli) {
            & python "$PSScriptRoot/prove_rustup_lifecycle.py" --publisher-preflight --publisher-cli $PublisherCli --host $HostTriple --evidence $evidenceRoot --windows-firewall-proof $proof
            if ($LASTEXITCODE -ne 0) { throw "Windows publisher/TLS preflight failed." }
        }
        return
    }
    & python "$PSScriptRoot/prove_rustup_lifecycle.py" --old "$Generations/old/mirror" --new "$Generations/new/mirror" --host $HostTriple --evidence $evidenceRoot --windows-firewall-proof $proof --publisher-cli $PublisherCli
    if ($LASTEXITCODE -ne 0) { throw "Windows lifecycle failed; retained evidence: $evidenceRoot" }
} finally {
    foreach ($rule in $rules) { Remove-NetFirewallRule -Name $rule.name }
    foreach ($profile in $profiles) { Set-NetFirewallProfile -Profile $profile.Name -Enabled $profile.Enabled }
}
