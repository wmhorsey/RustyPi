param(
  [Parameter(Mandatory = $true)]
  [string]$NewRemoteUrl
)

# Run from repository root that currently contains ./pi-unitary
$ErrorActionPreference = 'Stop'

if (-not (Test-Path "./pi-unitary")) {
  throw "Expected ./pi-unitary in current directory."
}

Write-Host "Creating subtree split for pi-unitary..."
$splitSha = git subtree split --prefix=pi-unitary -b pi-unitary-split

Write-Host "Creating worktree for split branch..."
if (Test-Path "./_pi-unitary-release") {
  Remove-Item -Recurse -Force "./_pi-unitary-release"
}

git worktree add ./_pi-unitary-release pi-unitary-split
Push-Location ./_pi-unitary-release

if (-not (Test-Path "./.git")) {
  throw "Worktree not initialized correctly."
}

Write-Host "Setting remote and pushing..."
if (git remote | Select-String -SimpleMatch "origin") {
  git remote remove origin
}

git remote add origin $NewRemoteUrl
git push -u origin pi-unitary-split:main

Pop-Location
Write-Host "Done. New repo pushed to main."
