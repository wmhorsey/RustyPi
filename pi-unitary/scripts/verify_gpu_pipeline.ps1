param(
  [int]$Ticks = 512,
  [int]$SnapshotEvery = 8,
  [int]$Nodes = 2048,
  [int]$TargetPhase = 128,
  [int]$BoundaryRateNum = 1,
  [int]$BoundaryRateDen = 1,
  [string]$OutDir = "reports/audit_gpu_verify_harness",
  [string]$RunLabel = "pingpong_choke_verify"
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot

try {
  New-Item -ItemType Directory -Path $OutDir -Force | Out-Null

  Write-Host "[1/3] GPU audit capture"
  $captureOutput = & cargo run -p pi-sim --bin gpu_pingpong_audit -- `
    --ticks $Ticks `
    --snapshot-every $SnapshotEvery `
    --nodes $Nodes `
    --target-phase $TargetPhase `
    --boundary-rate-num $BoundaryRateNum `
    --boundary-rate-den $BoundaryRateDen `
    --out-dir $OutDir `
    --run-label $RunLabel 2>&1

  $captureOutput | ForEach-Object { Write-Host $_ }
  if ($LASTEXITCODE -ne 0) {
    Write-Error "gpu_pingpong_audit failed with exit code $LASTEXITCODE"
  }

  $runDirLine = $captureOutput | Where-Object { $_ -match '^run_dir=' } | Select-Object -Last 1
  if (-not $runDirLine) {
    Write-Error "Could not find run_dir in gpu_pingpong_audit output"
  }

  $runDir = $runDirLine -replace '^run_dir=', ''
  if (-not (Test-Path $runDir)) {
    Write-Error "Reported run_dir does not exist: $runDir"
  }

  Write-Host "[2/3] Decode audit to CSV"
  & cargo run -p pi-sim --bin gpu_audit_decode_choke -- `
    --run-dir $runDir `
    --out decoded_choke_nodes.csv
  if ($LASTEXITCODE -ne 0) {
    Write-Error "gpu_audit_decode_choke failed with exit code $LASTEXITCODE"
  }

  $decodedCsv = Join-Path $runDir 'decoded_choke_nodes.csv'
  if (-not (Test-Path $decodedCsv)) {
    Write-Error "Decoded CSV missing: $decodedCsv"
  }

  Write-Host "[3/3] Analyze physics invariants"
  & python scripts/analyze_choke_physics.py `
    --csv $decodedCsv `
    --out-prefix (Join-Path $runDir 'physics_report') `
    --boundary-rate-num $BoundaryRateNum `
    --boundary-rate-den $BoundaryRateDen

  $analysisExit = $LASTEXITCODE
  if ($analysisExit -eq 0) {
    Write-Host "VERIFY PASS"
    Write-Host "run_dir=$runDir"
    exit 0
  }

  if ($analysisExit -eq 2) {
    Write-Host "VERIFY FAIL"
    Write-Host "run_dir=$runDir"
    exit 2
  }

  Write-Error "analyze_choke_physics.py failed with unexpected exit code $analysisExit"
}
finally {
  Pop-Location
}
