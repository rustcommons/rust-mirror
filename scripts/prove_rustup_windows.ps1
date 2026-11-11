param([Parameter(Mandatory=$true)][string]$Generations,
      [Parameter(Mandatory=$true)][string]$Evidence,
      [string]$HostTriple = "x86_64-pc-windows-msvc",
      [switch]$ValidateOnly)
$ErrorActionPreference = "Stop"
if ($env:GITHUB_ACTIONS -ne "true" -or $env:RUNNER_OS -ne "Windows") {
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
    foreach ($relative in @("rustup-init.exe", "cargo/bin/rustup.exe", "cargo/bin/rustup-init.exe", "cargo/bin/cargo.exe", "cargo/bin/rustc.exe")) {
        $program = [IO.Path]::GetFullPath((Join-Path $evidenceRoot $relative))
        $ruleName = $group + "-" + $rules.Count
        New-NetFirewallRule -Name $ruleName -DisplayName $ruleName -Group $group -Direction Outbound -Action Block -Enabled True -Profile Any -Program $program -RemoteAddress @("0.0.0.0/2", "64.0.0.0/3", "96.0.0.0/4", "112.0.0.0/5", "120.0.0.0/6", "124.0.0.0/7", "126.0.0.0/8", "128.0.0.0/1", "::/0") | Out-Null
        $rules += @{ name=$ruleName; program=$program; remote="all except IPv4 loopback" }
    }
    @{disposable_runner=$true; rules=$rules; profiles=@(Get-NetFirewallProfile | Select-Object Name,Enabled)} | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8 -LiteralPath $proof
    if ($ValidateOnly) { return }
    & python "$PSScriptRoot/prove_rustup_lifecycle.py" --old "$Generations/old/mirror" --new "$Generations/new/mirror" --host $HostTriple --evidence $evidenceRoot --windows-firewall-proof $proof
    if ($LASTEXITCODE -ne 0) { throw "Windows lifecycle failed; retained evidence: $evidenceRoot" }
} finally {
    foreach ($rule in $rules) { Remove-NetFirewallRule -Name $rule.name }
    foreach ($profile in $profiles) { Set-NetFirewallProfile -Profile $profile.Name -Enabled $profile.Enabled }
}
