param(
    [string]$AuraExe = "aura"
)

$ErrorActionPreference = "Stop"

function Invoke-Aura {
    param(
        [Parameter(ValueFromRemainingArguments = $true)]
        [string[]]$Args
    )

    Write-Host ">> $AuraExe $($Args -join ' ')" -ForegroundColor Cyan
    $output = & $AuraExe @Args 2>&1
    if ($LASTEXITCODE -ne 0) {
        $rendered = $output | Out-String
        throw "Command failed with exit code ${LASTEXITCODE}: $AuraExe $($Args -join ' ')`n$rendered"
    }
    $output
}

function Assert-True {
    param(
        [bool]$Condition,
        [string]$Message
    )

    if (-not $Condition) {
        throw $Message
    }
}

function Assert-Contains {
    param(
        [string]$Text,
        [string]$Needle,
        [string]$Message
    )

    if (-not $Text.Contains($Needle)) {
        throw "$Message`nExpected to find: $Needle`nActual output:`n$Text"
    }
}

function Get-FileNames {
    param([string]$Path)

    @(Get-ChildItem $Path -File | Select-Object -ExpandProperty Name | Sort-Object)
}

$repoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$tempRoot = Join-Path $env:TEMP ("aura-auto-" + [guid]::NewGuid().ToString())
$localRepo = Join-Path $tempRoot "local"
$remoteRepo = Join-Path $tempRoot "remote"
$cloneRepo = Join-Path $tempRoot "clone"

try {
    New-Item -ItemType Directory -Path $tempRoot -Force | Out-Null
    New-Item -ItemType Directory -Path $localRepo -Force | Out-Null
    New-Item -ItemType Directory -Path $remoteRepo -Force | Out-Null
    New-Item -ItemType Directory -Path $cloneRepo -Force | Out-Null

    Write-Host "== Init repositories ==" -ForegroundColor Yellow
    Invoke-Aura -Args @('init', $localRepo) | Out-Host
    Invoke-Aura -Args @('init', $remoteRepo) | Out-Host
    Invoke-Aura -Args @('init', $cloneRepo) | Out-Host

    Write-Host "== Initial commit ==" -ForegroundColor Yellow
    Set-Content (Join-Path $localRepo "main.c") ""
    $statusBeforeAdd = Invoke-Aura -Args @('-C', $localRepo, 'status') | Out-String
    Assert-Contains $statusBeforeAdd "untracked:" "Expected an untracked file before add"

    Invoke-Aura -Args @('-C', $localRepo, 'add', '-A') | Out-Host
    $commitOutput = Invoke-Aura -Args @('-C', $localRepo, 'commit', '-m', 'initial commit') | Out-String
    Assert-Contains $commitOutput "[main " "The first commit should create branch main"

    Write-Host "== Modify and diff ==" -ForegroundColor Yellow
    Set-Content (Join-Path $localRepo "main.c") "int main() { return 0; }"
    $diffOutput = Invoke-Aura -Args @('-C', $localRepo, 'diff') | Out-String
    Assert-Contains $diffOutput "+int main() { return 0; }" "Diff should show the added line"
    Invoke-Aura -Args @('-C', $localRepo, 'add', 'main.c') | Out-Host
    Invoke-Aura -Args @('-C', $localRepo, 'commit', '-m', 'update main.c') | Out-Host

    Write-Host "== Branch and switch ==" -ForegroundColor Yellow
    Invoke-Aura -Args @('-C', $localRepo, 'branch', 'feature') | Out-Host
    Invoke-Aura -Args @('-C', $localRepo, 'switch', 'feature') | Out-Host
    Set-Content (Join-Path $localRepo "feature.c") "void feature() {}"
    Invoke-Aura -Args @('-C', $localRepo, 'add', '-A') | Out-Host
    Invoke-Aura -Args @('-C', $localRepo, 'commit', '-m', 'add feature.c') | Out-Host

    $featureFiles = Get-FileNames $localRepo
    Assert-True ($featureFiles -contains "feature.c") "feature.c should exist on branch feature"

    Invoke-Aura -Args @('-C', $localRepo, 'switch', 'main') | Out-Host
    $mainFiles = Get-FileNames $localRepo
    Assert-True (-not ($mainFiles -contains "feature.c")) "feature.c should disappear after switching back to main"

    Write-Host "== Untracked protection ==" -ForegroundColor Yellow
    Set-Content (Join-Path $localRepo "feature.c") "local only"
    $statusWithUntracked = Invoke-Aura -Args @('-C', $localRepo, 'status') | Out-String
    Assert-Contains $statusWithUntracked "untracked:" "Status should report the local untracked file"
    Assert-True (-not $statusWithUntracked.Contains("unstaged:`n  - added: feature.c")) "An untracked file must not appear in unstaged as added"

    $switchError = ""
    try {
        Invoke-Aura -Args @('-C', $localRepo, 'switch', 'feature') | Out-Host
        throw "Expected switch to fail because it would overwrite an untracked file"
    } catch {
        $switchError = $_ | Out-String
    }
    Assert-Contains $switchError "untracked" "Expected an error about overwriting an untracked file"
    Remove-Item (Join-Path $localRepo "feature.c") -Force

    Write-Host "== Remote, push and pull ==" -ForegroundColor Yellow
    Invoke-Aura -Args @('-C', $localRepo, 'remote', 'add', 'origin', $remoteRepo) | Out-Host
    Invoke-Aura -Args @('-C', $localRepo, 'push', 'origin', 'main') | Out-Host

    Invoke-Aura -Args @('-C', $cloneRepo, 'remote', 'add', 'origin', $remoteRepo) | Out-Host
    Invoke-Aura -Args @('-C', $cloneRepo, 'pull', 'origin', 'main') | Out-Host
    $cloneFiles = Get-FileNames $cloneRepo
    Assert-True ($cloneFiles -contains "main.c") "main.c should appear in clone after pull"

    $pullAgain = Invoke-Aura -Args @('-C', $cloneRepo, 'pull', 'origin', 'main') | Out-String
    Assert-Contains $pullAgain "Already up to date." "A repeated pull should say the branch is already up to date"

    Write-Host ""
    Write-Host "All Aura automated checks passed." -ForegroundColor Green
    Write-Host "Temporary directories:" -ForegroundColor Green
    Write-Host "  local : $localRepo"
    Write-Host "  remote: $remoteRepo"
    Write-Host "  clone : $cloneRepo"
} finally {
    if (Test-Path $tempRoot) {
        Remove-Item $tempRoot -Recurse -Force
    }
}
