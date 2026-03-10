param(
  [switch]$NoTests
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot

try {
  Write-Host "[1/2] Additive-operator guard"
  python scripts/check_additive_kernel_ops.py

  if (-not $NoTests) {
    Write-Host "[2/2] Workspace tests"
    cargo test --workspace
  }

  Write-Host "Additive check complete."
}
finally {
  Pop-Location
}
