$ErrorActionPreference = 'Stop'

$auraExe = Join-Path $env:USERPROFILE '.cargo\bin\aura.exe'
if (-not (Test-Path $auraExe)) {
    throw "Aura executable was not found: $auraExe"
}

function Set-AuraProfileBlock {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$ProfilePaths
    )

    $begin = '# >>> aura shim >>>'
    $end = '# <<< aura shim <<<'
    $block = @"
# >>> aura shim >>>
function aura {
    `$auraExe = Join-Path `$env:USERPROFILE '.cargo\bin\aura.exe'
    if (-not (Test-Path `$auraExe)) {
        throw "Aura executable was not found: `$auraExe"
    }
    & `$auraExe @args
}
# <<< aura shim <<<
"@.TrimEnd()

    foreach ($profilePath in $ProfilePaths | Select-Object -Unique) {
        $directory = Split-Path -Parent $profilePath
        if (-not (Test-Path $directory)) {
            New-Item -ItemType Directory -Path $directory -Force | Out-Null
        }

        $content = if (Test-Path $profilePath) {
            Get-Content -Path $profilePath -Raw
        } else {
            ''
        }

        $pattern = '(?s)' + [regex]::Escape($begin) + '.*?' + [regex]::Escape($end)
        if ($content -match [regex]::Escape($begin)) {
            $updated = [regex]::Replace($content, $pattern, $block)
        } else {
            if ($content.Length -gt 0 -and -not $content.EndsWith("`r`n")) {
                $content += "`r`n"
            }
            $updated = $content + $block + "`r`n"
        }

        Set-Content -Path $profilePath -Value $updated -Encoding UTF8
    }
}

function Test-WritableDirectory {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    try {
        if (-not (Test-Path $Path)) {
            New-Item -ItemType Directory -Path $Path -Force | Out-Null
        }

        $probe = Join-Path $Path '.aura-write-test'
        Set-Content -Path $probe -Value 'ok' -Encoding Ascii
        Remove-Item -Path $probe -Force
        return $true
    } catch {
        return $false
    }
}

function Set-AuraCmdShim {
    param(
        [Parameter(Mandatory = $true)]
        [string]$AuraExe
    )

    $sessionPaths = @(
        ($env:PATH -split ';' | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
    )

    $preferred = @(
        (Join-Path $env:APPDATA 'npm'),
        (Join-Path $env:USERPROFILE '.dotnet\tools'),
        (Join-Path $env:LOCALAPPDATA 'Microsoft\WindowsApps')
    )

    $candidates = @($preferred + $sessionPaths) | Select-Object -Unique
    $cargoBin = Split-Path -Parent $AuraExe

    foreach ($candidate in $candidates) {
        if ([string]::IsNullOrWhiteSpace($candidate)) {
            continue
        }
        if ($candidate -ieq $cargoBin) {
            continue
        }
        if (-not ($sessionPaths -contains $candidate)) {
            continue
        }
        if (-not $candidate.StartsWith($env:USERPROFILE, [System.StringComparison]::OrdinalIgnoreCase)) {
            continue
        }
        if (-not (Test-WritableDirectory -Path $candidate)) {
            continue
        }

        $shimPath = Join-Path $candidate 'aura.cmd'
        $shimContent = "@echo off`r`n`"$AuraExe`" %*`r`n"
        Set-Content -Path $shimPath -Value $shimContent -Encoding Ascii
        return $shimPath
    }

    throw 'Could not create aura.cmd in a writable directory already visible in PATH.'
}

$profileTargets = @(
    (Join-Path $env:USERPROFILE 'Documents\WindowsPowerShell\profile.ps1'),
    (Join-Path $env:USERPROFILE 'Documents\WindowsPowerShell\Microsoft.PowerShell_profile.ps1'),
    (Join-Path $env:USERPROFILE 'Documents\PowerShell\profile.ps1'),
    (Join-Path $env:USERPROFILE 'Documents\PowerShell\Microsoft.PowerShell_profile.ps1')
)

Set-AuraProfileBlock -ProfilePaths $profileTargets
$shimPath = Set-AuraCmdShim -AuraExe $auraExe

Write-Output "PROFILES_UPDATED=1"
Write-Output "CMD_SHIM=$shimPath"
