param(
    [int]$Ticks = 1024,
    [int]$SnapshotEvery = 16,
    [int]$TargetPhase = 128,
    [string]$OutDir = "reports/audit_gpu_scale_probe",
    [string]$NodeScalesCsv = "512,2048,8192"
)

$ErrorActionPreference = "Stop"

if ($Ticks -le 0) {
    throw "Ticks must be > 0"
}
if ($SnapshotEvery -le 0) {
    throw "SnapshotEvery must be > 0"
}
$scaleList = @()
foreach ($value in ($NodeScalesCsv -split ',')) {
    $parsed = 0
    $trimmed = $value.Trim()
    if ($trimmed.Length -eq 0) {
        continue
    }
    if (-not [int]::TryParse($trimmed, [ref]$parsed) -or $parsed -le 0) {
        throw "Invalid node scale value: $value"
    }
    $scaleList += $parsed
}

if ($scaleList.Count -eq 0) {
    throw "No valid node scales parsed"
}

function Invoke-CargoChecked {
    param([string]$Command)

    Invoke-Expression $Command
    if ($LASTEXITCODE -ne 0) {
        throw "Command failed with exit code ${LASTEXITCODE}: $Command"
    }
}

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

$runs = @()
foreach ($nodes in $scaleList) {
    if ($nodes -le 0) {
        throw "Node scale values must be > 0"
    }

    $label = "scale_${nodes}"
    Invoke-CargoChecked "cargo run -q -p pi-sim --bin gpu_pingpong_audit -- --ticks $Ticks --snapshot-every $SnapshotEvery --nodes $nodes --target-phase $TargetPhase --segment-mb 256 --out-dir '$OutDir' --run-label $label"

    $runDir = Get-ChildItem -Path $OutDir -Directory |
        Where-Object { $_.Name -like "$label-*" } |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1

    if ($null -eq $runDir) {
        throw "Failed to locate run directory for label $label"
    }

    Invoke-CargoChecked "cargo run -q -p pi-sim --bin gpu_audit_decode_choke -- --run-dir '$($runDir.FullName)' --out decoded_choke_nodes.csv"

    $runs += [pscustomobject]@{
        nodes = $nodes
        run_dir = $runDir.FullName
        csv = (Join-Path $runDir.FullName "decoded_choke_nodes.csv")
    }
}

function Get-PhaseShare {
    param(
        [object[]]$Grouped,
        [string]$Name,
        [double]$Total
    )

    $group = $Grouped | Where-Object { $_.Name -eq $Name }
    if ($null -eq $group) {
        return 0.0
    }
    return [math]::Round(($group.Count / $Total), 6)
}

$summary = @()
foreach ($r in $runs) {
    $data = Import-Csv $r.csv
    $total = [double]$data.Count
    $phaseGroups = $data | Group-Object phase

    $summary += [pscustomobject]@{
        nodes = $r.nodes
        rows = [int]$total
        run_dir = $r.run_dir
        csv = $r.csv
        neg_energy = ($data | Where-Object { [int]$_.energy -lt 0 }).Count
        neg_coherence = ($data | Where-Object { [int]$_.coherence -lt 0 }).Count
        mean_energy = [math]::Round([double](($data | Measure-Object -Property energy -Average).Average), 6)
        mean_coherence = [math]::Round([double](($data | Measure-Object -Property coherence -Average).Average), 6)
        max_energy = [int](($data | Measure-Object -Property energy -Maximum).Maximum)
        max_coherence = [int](($data | Measure-Object -Property coherence -Maximum).Maximum)
        p_free = Get-PhaseShare -Grouped $phaseGroups -Name "free" -Total $total
        p_formation = Get-PhaseShare -Grouped $phaseGroups -Name "formation" -Total $total
        p_liftoff = Get-PhaseShare -Grouped $phaseGroups -Name "liftoff" -Total $total
        p_coherence = Get-PhaseShare -Grouped $phaseGroups -Name "coherence" -Total $total
        p_drift = Get-PhaseShare -Grouped $phaseGroups -Name "drift" -Total $total
        p_dissolution = Get-PhaseShare -Grouped $phaseGroups -Name "dissolution" -Total $total
    }
}

$summaryPath = Join-Path $OutDir "scale_summary.json"
$summary | ConvertTo-Json -Depth 4 | Set-Content -Path $summaryPath

Write-Host "gpu scale probe complete"
Write-Host "summary=$summaryPath"
$summary | Format-Table -AutoSize | Out-String | Write-Host
