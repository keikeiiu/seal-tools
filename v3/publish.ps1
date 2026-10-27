# Seal Tools v3.0 publish pipeline (CUTOVER.md item 1 / the packaging work item).
#
#   .\v3\publish.ps1 -Verify    (default) check every release input, print what WOULD be staged
#   .\v3\publish.ps1 -Stage     build core + shell, stage dist\SealTools-v3.0\
#   .\v3\publish.ps1 -Release   -Stage + allow-list audit + the two zips + SHA-256
#   .\v3\publish.ps1 -Selftest  run the packaging guards without a release build (task-58)
#
# -Release refuses until -AcceptDirectMLRedistribution is passed: DirectML.dll is redistributed
# under Microsoft's terms, still an explicit release input (see v3/docs/records/OCR-RESEARCH.md and
# v3/docs/backlog/OPEN.md). Nothing here writes to a git worktree.
#
# Security/robustness rules (task-58):
#   * $Version is validated against a fixed schema BEFORE any path is built, and every stage/zip
#     path must resolve directly under <repo>\dist; destructive calls use -LiteralPath.
#   * A global deny runs on every normalized relative path (player state, backups, logs, captures,
#     calibration images, env/log files) BEFORE the allow-list; data\ is narrowed to the Flutter
#     runtime entries instead of a blanket tree.
#   * The audit runs on the STAGE and then again on the actual ZIP entries (single top folder, no
#     absolute/traversal paths, exact entry set, deny rules); hidden/system and reparse-point items
#     are refused; the firmware zip is audited against its intended file set.
#   * -Selftest exercises these guards with temp directories, a temp zip and no build.

[CmdletBinding()]
param(
    [switch]$Verify,
    [switch]$Stage,
    [switch]$Release,
    [switch]$Selftest,
    [switch]$AcceptDirectMLRedistribution,
    [string]$ModelsDir = "",
    [string]$Version = "v3.0"
)

$ErrorActionPreference = 'Stop'

$V3 = Split-Path -Parent $MyInvocation.MyCommand.Path
$Repo = Split-Path -Parent $V3
$CoreDir = Join-Path $V3 'core'
$ShellDir = Join-Path $V3 'shell'
$ConfigDir = Join-Path $V3 'config'
$DocsDir = Join-Path $V3 'docs'
$ShellBundle = Join-Path $ShellDir 'build\windows\x64\runner\Release'
$ShellExe = Join-Path $ShellBundle 'SealTools.exe'
$CoreExe = Join-Path $CoreDir 'target\release\seal-core.exe'
$DirectMl = Join-Path $CoreDir 'target\release\DirectML.dll'
$DebugDirectMl = Join-Path $CoreDir 'target\debug\DirectML.dll'
$FirmwareSketch = Join-Path $Repo 'arduino\seal_mouse'
$FirmwareIno = Join-Path $FirmwareSketch 'seal_mouse.ino'
$PetData = Join-Path $DocsDir 'pet-data.csv'
$DistDir = Join-Path $Repo 'dist'

$VersionPattern = '^v\d+\.\d+(\.\d+)?$'
$ModelNames = @(
    'ch_PP-OCRv4_det_infer.onnx',
    'ch_PP-OCRv4_rec_infer.onnx',
    'ch_ppocr_mobile_v2.0_cls_infer.onnx'
)
# Release docs. THIRD-PARTY-NOTICES.md is a licence obligation, not an optional guide: it is
# required by -Verify and exact-name allow-listed exactly like the guides.
$GuideNames = @('USER_GUIDE.md', 'USER_GUIDE.zh-TW.md', 'INSTALL.md', 'THIRD-PARTY-NOTICES.md')
$FirmwareFiles = @('seal_mouse.ino')
# The Flutter release bundle's exact data\ root file set (checked against
# v3\shell\build\windows\x64\runner\Release\data): app.so is the AOT snapshot, icudtl.dat the ICU
# data. flutter_assets\ is the asset tree. No data\ directory wildcard is allowed.
$FlutterDataFiles = @('app.so', 'icudtl.dat')
$FlutterRuntimeFiles = @('flutter_windows.dll') + ($FlutterDataFiles | ForEach-Object { "data\$_" })

function Fail([string]$message) {
    Write-Host ''
    Write-Host "[!] $message" -ForegroundColor Red
    exit 1
}

function Test-VersionValue([string]$value) {
    if ($value -notmatch $VersionPattern) {
        return "Version must match $VersionPattern (got '$value'); no path was built."
    }
    return $null
}

function Test-DistChild([string]$path, [string]$label) {
    $full = [IO.Path]::GetFullPath($path)
    $parent = [IO.Path]::GetDirectoryName($full)
    $dist = [IO.Path]::GetFullPath($DistDir)
    if (-not [string]::Equals($parent.TrimEnd('\'), $dist.TrimEnd('\'), [StringComparison]::OrdinalIgnoreCase)) {
        return "$label must be directly under $dist (got $full)."
    }
    return $null
}

function Get-ReleasePaths([string]$version) {
    $errorLine = Test-VersionValue $version
    if ($errorLine) { return @{ Error = $errorLine } }
    $stage = Join-Path $DistDir "SealTools-$version"
    $appZip = "$stage.zip"
    $firmwareZip = Join-Path $DistDir "SealTools-$version-firmware.zip"
    foreach ($pair in @(
        @{ Label = 'stage directory'; Path = $stage },
        @{ Label = 'app zip'; Path = $appZip },
        @{ Label = 'firmware zip'; Path = $firmwareZip }
    )) {
        $errorLine = Test-DistChild $pair.Path $pair.Label
        if ($errorLine) { return @{ Error = $errorLine } }
    }
    return @{ Error = $null; Stage = $stage; AppZip = $appZip; FirmwareZip = $firmwareZip }
}

function Assert-RegularSource([string]$path, [string]$label) {
    if (-not (Test-Path -LiteralPath $path)) { Fail "$label missing: $path" }
    $item = Get-Item -LiteralPath $path -Force
    if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
        Fail "$label is a reparse point, refusing to stage it: $path"
    }
    if ($item.Attributes -band [IO.FileAttributes]::Hidden -or $item.Attributes -band [IO.FileAttributes]::System) {
        Fail "$label has Hidden/System attributes, refusing to stage it: $path"
    }
}

function Get-RelativeTo([string]$full, [string]$root) {
    $rootFull = [IO.Path]::GetFullPath($root).TrimEnd('\')
    $fullPath = [IO.Path]::GetFullPath($full)
    if ($fullPath.Length -le $rootFull.Length) { return $null }
    if (-not $fullPath.StartsWith($rootFull + '\', [StringComparison]::OrdinalIgnoreCase)) { return $null }
    return $fullPath.Substring($rootFull.Length + 1).Replace('\', '/')
}

function Test-DenyRelative([string]$relative) {
    $relative = $relative.Replace('\', '/')
    if ($relative.StartsWith('/') -or $relative -match '^[A-Za-z]:') {
        return 'absolute path'
    }
    $segments = @($relative.Split('/'))
    foreach ($segment in $segments) {
        if ($segment -eq '..' -or $segment -eq '.') { return 'path traversal segment' }
    }
    if ($segments.Count -eq 0) { return 'empty path' }
    # The one release template whose name starts with local.yaml is shipped on purpose; it is still
    # allow-listed only at config/local.yaml.example, so an asset copy elsewhere is refused.
    if ($relative -eq 'config/local.yaml.example') { return $null }
    $name = $segments[$segments.Count - 1]
    if ($name -eq 'local.yaml' -or
        $name -like 'local.yaml.*' -or
        $name -like '*.backup-*' -or
        $name -like '*.corrupt-backup' -or
        $name -like '*.local.*' -or
        $name -like '*.env' -or
        $name -like '*.log' -or
        $name -like '*.pdb' -or
        $name -like '*.lib' -or
        $name -like '*.exp' -or
        $name -like '*.ilk' -or
        $name -like '*.obj') {
        return "player-state/debug name '$name'"
    }
    foreach ($segment in $segments) {
        if ($segment -eq 'logs' -or
            $segment -eq 'captures' -or
            $segment -eq 'slot_captures' -or
            $segment -eq '.git' -or
            $segment -eq '.scratch' -or
            $segment -like 'calib_*') {
            return "player-state/debris segment '$segment'"
        }
    }
    return $null
}

function Get-StageSafetyProblems([string]$stageDir) {
    $problems = New-Object System.Collections.Generic.List[string]
    if (-not (Test-Path -LiteralPath $stageDir)) {
        $problems.Add("stage directory missing: $stageDir")
        return $problems
    }
    Get-ChildItem -LiteralPath $stageDir -Recurse -Force | ForEach-Object {
        $item = $_
        $relative = Get-RelativeTo $item.FullName $stageDir
        if ($null -eq $relative) { $relative = $item.FullName }
        if ($item.Attributes -band [IO.FileAttributes]::Hidden) {
            $problems.Add("FORBIDDEN attributes (Hidden): $relative")
        }
        if ($item.Attributes -band [IO.FileAttributes]::System) {
            $problems.Add("FORBIDDEN attributes (System): $relative")
        }
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
            $problems.Add("FORBIDDEN reparse point: $relative")
        }
        if (-not $item.PSIsContainer) {
            $deny = Test-DenyRelative $relative
            if ($deny) { $problems.Add("FORBIDDEN: $relative ($deny)") }
        }
    }
    return $problems
}

function Test-AllowedStageRelative([string]$relative) {
    # native_assets.json is the Flutter tool's native-asset manifest: a legitimate part of the
    # release bundle (the engine reads it when the app has no native assets too), and NOT player state.
    $allowedRoot = @(
        'SealTools.exe',
        'seal-core.exe',
        'DirectML.dll',
        'flutter_windows.dll',
        'native_assets.json'
    )
    $allowedConfig = @(
        'config/defaults.yaml.example',
        'config/local.yaml.example',
        'config/attributes.yaml',
        'config/attributes.us.yaml.example'
    )
    $allowedDocs = @('docs/pet-data.csv') + ($GuideNames | ForEach-Object { "docs/$_" })
    $allowedModels = $ModelNames | ForEach-Object { "models/$_" }
    if ($allowedRoot -contains $relative) { return $true }
    if ($allowedConfig -contains $relative) { return $true }
    if ($allowedDocs -contains $relative) { return $true }
    if ($allowedModels -contains $relative) { return $true }
    foreach ($name in $FlutterDataFiles) {
        if ($relative -eq "data/$name") { return $true }
    }
    if ($relative -like 'data/flutter_assets/*') { return $true }
    return $false
}

function Get-StageAuditProblems([string]$stageDir) {
    $problems = New-Object System.Collections.Generic.List[string]
    foreach ($safetyProblem in @(Get-StageSafetyProblems $stageDir)) {
        $problems.Add($safetyProblem)
    }

    $required = @('SealTools.exe', 'seal-core.exe', 'DirectML.dll')
    foreach ($runtime in $FlutterRuntimeFiles) { $required += $runtime.Replace('\', '/') }
    $required += @(
        'models/ch_PP-OCRv4_det_infer.onnx',
        'models/ch_PP-OCRv4_rec_infer.onnx',
        'models/ch_ppocr_mobile_v2.0_cls_infer.onnx',
        'config/defaults.yaml.example',
        'config/local.yaml.example',
        'config/attributes.yaml',
        'config/attributes.us.yaml.example',
        'docs/pet-data.csv'
    )
    foreach ($name in $GuideNames) { $required += "docs/$name" }
    foreach ($name in $required) {
        if (-not (Test-Path -LiteralPath (Join-Path $stageDir $name) -PathType Leaf)) {
            $problems.Add("MISSING: $name")
        }
    }

    $assets = Join-Path $stageDir 'data\flutter_assets'
    if (-not (Test-Path -LiteralPath $assets -PathType Container)) {
        $problems.Add('MISSING: data/flutter_assets (directory)')
    } else {
        $assetFiles = @(Get-ChildItem -LiteralPath $assets -Recurse -File -Force)
        if ($assetFiles.Count -eq 0) { $problems.Add('MISSING: data/flutter_assets is empty') }
    }

    Get-ChildItem -LiteralPath $stageDir -Recurse -Force | Where-Object { -not $_.PSIsContainer } | ForEach-Object {
        $relative = Get-RelativeTo $_.FullName $stageDir
        if ($null -eq $relative) { $relative = $_.FullName }
        if (-not (Test-AllowedStageRelative $relative)) {
            $problems.Add("NOT ON THE ALLOW-LIST: $relative")
        }
    }
    return $problems
}

function Test-StageAudit {
    $problems = @(Get-StageAuditProblems $StageDir)
    if ($problems.Count -gt 0) {
        Write-Host '[!] The staged payload failed the release audit:'
        $problems | ForEach-Object { Write-Host "  - $_" }
        Fail 'Nothing zipped. The allow-list above is the rule: no player state, no debug debris, no unknown file.'
    }
    Write-Host 'Allow-list audit (stage): PASS'
}

function Get-ZipProblems([string]$zipPath, [string]$expectedTop, [string[]]$expectedEntries) {
    $problems = New-Object System.Collections.Generic.List[string]
    if (-not (Test-Path -LiteralPath $zipPath)) {
        $problems.Add("zip missing: $zipPath")
        return $problems
    }
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $actual = New-Object System.Collections.Generic.List[string]
    $archive = $null
    try {
        $archive = [IO.Compression.ZipFile]::OpenRead($zipPath)
        foreach ($entry in $archive.Entries) {
            $name = $entry.FullName.Replace('\', '/')
            if ($name.EndsWith('/')) { continue }
            if ($name.StartsWith('/') -or $name -match '^[A-Za-z]:') {
                $problems.Add("FORBIDDEN zip path: $name")
                continue
            }
            $segments = @($name.Split('/'))
            $badSegment = $false
            foreach ($segment in $segments) {
                if ($segment -eq '..' -or $segment -eq '.') { $badSegment = $true }
            }
            if ($badSegment) {
                $problems.Add("FORBIDDEN zip path segment: $name")
                continue
            }
            if (-not $name.StartsWith($expectedTop + '/', [StringComparison]::OrdinalIgnoreCase)) {
                $problems.Add("ZIP ENTRY outside the single top folder ${expectedTop}/: $name")
                continue
            }
            $relative = $name.Substring($expectedTop.Length + 1)
            $deny = Test-DenyRelative $relative
            if ($deny) { $problems.Add("FORBIDDEN zip entry: $name ($deny)") }
            if ($actual -contains $name) {
                $problems.Add("DUPLICATE ZIP ENTRY: $name")
            } else {
                $actual.Add($name)
            }
        }
    } finally {
        if ($archive) { $archive.Dispose() }
    }
    foreach ($entry in $actual) {
        if ($expectedEntries -notcontains $entry) {
            $problems.Add("UNEXPECTED ZIP ENTRY: $entry")
        }
    }
    foreach ($entry in $expectedEntries) {
        if ($actual -notcontains $entry) {
            $problems.Add("MISSING ZIP ENTRY: $entry")
        }
    }
    return $problems
}

function Get-StageRelativeFiles([string]$stageDir) {
    $files = New-Object System.Collections.Generic.List[string]
    Get-ChildItem -LiteralPath $stageDir -Recurse -Force | Where-Object { -not $_.PSIsContainer } | ForEach-Object {
        $relative = Get-RelativeTo $_.FullName $stageDir
        if ($null -ne $relative) { $files.Add($relative) }
    }
    return $files
}

function Test-AppZipAudit {
    $top = "SealTools-$Version"
    $stageFiles = Get-StageRelativeFiles $StageDir
    $expected = New-Object System.Collections.Generic.List[string]
    foreach ($relative in $stageFiles) { $expected.Add("$top/$relative") }
    $problems = Get-ZipProblems $AppZip $top ([string[]]$expected)
    if ($problems.Count -gt 0) {
        Write-Host '[!] The app zip failed the post-zip audit:'
        $problems | ForEach-Object { Write-Host "  - $_" }
        Fail 'Nothing published. The archive must contain exactly the audited payload, under one top folder.'
    }
    Write-Host 'Allow-list audit (app zip): PASS'
}

function Get-FirmwareSourceProblems([string]$sketchDir) {
    $problems = New-Object System.Collections.Generic.List[string]
    if (-not (Test-Path -LiteralPath $sketchDir)) {
        $problems.Add("firmware sketch missing: $sketchDir")
        return $problems
    }
    Get-ChildItem -LiteralPath $sketchDir -Recurse -Force | ForEach-Object {
        $item = $_
        $relative = Get-RelativeTo $item.FullName $sketchDir
        if ($null -eq $relative) { $relative = $item.FullName }
        if ($item.Attributes -band [IO.FileAttributes]::Hidden -or
            $item.Attributes -band [IO.FileAttributes]::System -or
            $item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
            $problems.Add("FORBIDDEN firmware attributes: $relative")
            return
        }
        if (-not $item.PSIsContainer -and ($FirmwareFiles -notcontains $relative)) {
            $problems.Add("UNEXPECTED firmware file: $relative")
        }
        if ($item.PSIsContainer -and $relative -ne '') {
            $problems.Add("UNEXPECTED firmware directory: $relative")
        }
    }
    foreach ($name in $FirmwareFiles) {
        if (-not (Test-Path -LiteralPath (Join-Path $sketchDir $name) -PathType Leaf)) {
            $problems.Add("MISSING firmware file: $name")
        }
    }
    return $problems
}

function Test-FirmwareZip {
    $problems = New-Object System.Collections.Generic.List[string]
    foreach ($sourceProblem in @(Get-FirmwareSourceProblems $FirmwareSketch)) {
        $problems.Add($sourceProblem)
    }
    if ($problems.Count -gt 0) {
        Write-Host '[!] The firmware sketch failed its audit:'
        $problems | ForEach-Object { Write-Host "  - $_" }
        Fail 'Nothing published. The firmware zip only carries the unchanged sketch files.'
    }
    $expected = New-Object System.Collections.Generic.List[string]
    foreach ($name in $FirmwareFiles) { $expected.Add("seal_mouse/$name") }
    $zipProblems = Get-ZipProblems $FirmwareZip 'seal_mouse' ([string[]]$expected)
    if ($zipProblems.Count -gt 0) {
        Write-Host '[!] The firmware zip failed the post-zip audit:'
        $zipProblems | ForEach-Object { Write-Host "  - $_" }
        Fail 'Nothing published. The firmware zip must contain exactly seal_mouse/ and the unchanged sketch file(s).'
    }
    Write-Host 'Firmware audit (source + zip): PASS'
}

function Write-Environment([string]$mode) {
    Write-Host '=============================================================='
    Write-Host "  Seal Tools v3 - v3 publish ($mode)"
    Write-Host '=============================================================='
    Write-Host "repo:   $Repo"
    $branch = & git -C $Repo rev-parse --abbrev-ref HEAD 2>$null
    $commit = & git -C $Repo rev-parse --short HEAD 2>$null
    if ($LASTEXITCODE -eq 0) {
        Write-Host "git:    branch $branch  commit $commit"
    } else {
        Write-Host 'git:    (not a git worktree; reproducibility line unavailable)'
    }
    Write-Host 'git status --short:'
    & git -C $Repo status --short
    Write-Host ''
}

function Read-StageInputProblems {
    $problems = New-Object System.Collections.Generic.List[string]
    if (-not (Test-Path -LiteralPath $ModelsDir)) {
        $hint = ''
        $v2Models = Join-Path $Repo 'v2\models'
        if (Test-Path -LiteralPath $v2Models) {
            $hint = " (the same three files exist at v2\models; copy them into v3\models or pass -ModelsDir v2\models)"
        }
        $problems.Add("models directory missing: $ModelsDir$hint")
    } else {
        foreach ($name in $ModelNames) {
            if (-not (Test-Path -LiteralPath (Join-Path $ModelsDir $name))) {
                $problems.Add("model missing: $(Join-Path $ModelsDir $name)")
            }
        }
    }
    foreach ($name in @('defaults.yaml', 'local.yaml.example', 'attributes.yaml', 'attributes.us.yaml')) {
        $path = Join-Path $ConfigDir $name
        if (-not (Test-Path -LiteralPath $path)) { $problems.Add("config source missing: $path") }
    }
    if (-not (Test-Path -LiteralPath $PetData)) {
        $problems.Add("pet feeding table missing: $PetData (copy it verbatim from v2\docs\pet-data.csv)")
    }
    if (-not (Test-Path -LiteralPath $FirmwareIno)) {
        $problems.Add("firmware sketch missing: $FirmwareIno")
    }
    return $problems
}

function Read-SourceProblems {
    $problems = New-Object System.Collections.Generic.List[string]

    # The Flutter release bundle and the files the payload exists for.
    if (-not (Test-Path -LiteralPath $ShellBundle)) {
        $problems.Add("shell release bundle missing: $ShellBundle (run: flutter build windows --release in v3\shell)")
    } else {
        if (-not (Test-Path -LiteralPath $ShellExe)) {
            $legacy = Join-Path $ShellBundle 'seal_shell.exe'
            if (Test-Path -LiteralPath $legacy) {
                $problems.Add("shell executable is seal_shell.exe, not SealTools.exe (CUTOVER item 2 naming is not done yet): $ShellBundle")
            } else {
                $problems.Add("shell executable missing: $ShellExe")
            }
        }
        foreach ($relative in $FlutterRuntimeFiles) {
            if (-not (Test-Path -LiteralPath (Join-Path $ShellBundle $relative) -PathType Leaf)) {
                $problems.Add("Flutter runtime file missing: $(Join-Path $ShellBundle $relative)")
            }
        }
        $assets = Join-Path $ShellBundle 'data\flutter_assets'
        if (-not (Test-Path -LiteralPath $assets -PathType Container)) {
            $problems.Add("Flutter assets missing: $assets")
        } elseif (@(Get-ChildItem -LiteralPath $assets -Recurse -File -Force).Count -eq 0) {
            $problems.Add("Flutter assets empty: $assets")
        }
    }

    if (-not (Test-Path -LiteralPath $CoreExe)) {
        $problems.Add("seal-core.exe missing: $CoreExe (run: cargo build --release -p seal-core in v3\core)")
    }
    if (-not (Test-Path -LiteralPath $DirectMl)) {
        $hint = ''
        if (Test-Path -LiteralPath $DebugDirectMl) {
            $hint = " (a debug copy exists at $DebugDirectMl; the release build must place its own)"
        }
        $problems.Add("DirectML.dll missing: $DirectMl$hint")
    }

    if (-not (Test-Path -LiteralPath $ModelsDir)) {
        $hint = ''
        $v2Models = Join-Path $Repo 'v2\models'
        if (Test-Path -LiteralPath $v2Models) {
            $hint = " (the same three files exist at v2\models; copy them into v3\models or pass -ModelsDir v2\models)"
        }
        $problems.Add("models directory missing: $ModelsDir$hint")
    } else {
        foreach ($name in $ModelNames) {
            $path = Join-Path $ModelsDir $name
            if (-not (Test-Path -LiteralPath $path)) {
                $problems.Add("model missing: $path")
            }
        }
    }

    foreach ($name in @('defaults.yaml', 'local.yaml.example', 'attributes.yaml', 'attributes.us.yaml')) {
        $path = Join-Path $ConfigDir $name
        if (-not (Test-Path -LiteralPath $path)) {
            $problems.Add("config source missing: $path")
        }
    }

    if (-not (Test-Path -LiteralPath $PetData)) {
        $problems.Add("pet feeding table missing: $PetData (copy it verbatim from v2\docs\pet-data.csv)")
    }
    if (-not (Test-Path -LiteralPath $FirmwareIno)) {
        $problems.Add("firmware sketch missing: $FirmwareIno")
    }
    foreach ($name in $GuideNames) {
        $path = Join-Path $DocsDir $name
        if (-not (Test-Path -LiteralPath $path)) {
            $problems.Add("release doc missing: $path")
        }
    }
    return $problems
}

function New-StageDirectories {
    if (Test-Path -LiteralPath $StageDir) {
        $item = Get-Item -LiteralPath $StageDir -Force
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
            Fail "refusing to remove a reparse-point stage directory: $StageDir"
        }
        if (-not $item.PSIsContainer) {
            Fail "refusing to remove a non-directory at the stage path: $StageDir"
        }
        Remove-Item -LiteralPath $StageDir -Recurse -Force
    }
    New-Item -ItemType Directory -Path $StageDir -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $StageDir 'models') -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $StageDir 'config') -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $StageDir 'docs') -Force | Out-Null
}

function Write-StageSummary {
    Write-Host 'Payload staged in:'
    Get-ChildItem -LiteralPath $StageDir -Recurse -File -Force |
        ForEach-Object {
            $relative = Get-RelativeTo $_.FullName $StageDir
            if ($null -eq $relative) { $relative = $_.FullName }
            Write-Host ("  {0,10}  {1}" -f $_.Length, $relative)
        }
}

function Invoke-Stage {
    $problems = Read-StageInputProblems
    if ($problems.Count -gt 0) {
        Write-Host '[!] Missing release inputs:'
        $problems | ForEach-Object { Write-Host "  - $_" }
        Fail 'Nothing staged. Fix the inputs above (or run -Verify for the full picture).'
    }
    foreach ($tool in @('cargo', 'flutter')) {
        if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
            Fail "$tool is not on PATH; -Stage needs it to build the payload."
        }
    }

    Write-Host '[1/4] Building seal-core (release)...'
    Push-Location $CoreDir
    try {
        & cargo build --release -p seal-core
        if ($LASTEXITCODE -ne 0) { Fail 'cargo build --release -p seal-core failed (see the log above).' }
    } finally {
        Pop-Location
    }

    Write-Host '[2/4] Building the Flutter Windows release bundle...'
    Push-Location $ShellDir
    try {
        & flutter build windows --release
        if ($LASTEXITCODE -ne 0) { Fail 'flutter build windows --release failed (see the log above).' }
    } finally {
        Pop-Location
    }

    if (-not (Test-Path -LiteralPath $CoreExe)) { Fail "the release build did not produce $CoreExe" }
    if (-not (Test-Path -LiteralPath $DirectMl)) { Fail "the release build did not place DirectML.dll at $DirectMl" }
    if (-not (Test-Path -LiteralPath $ShellExe)) {
        $legacy = Join-Path $ShellBundle 'seal_shell.exe'
        if (Test-Path -LiteralPath $legacy) {
            Fail "the Flutter bundle carries seal_shell.exe, not SealTools.exe - apply CUTOVER item 2 (BINARY_NAME/window title/pubspec 3.0.0+1) first."
        }
        Fail "the Flutter build did not produce $ShellExe"
    }
    foreach ($relative in $FlutterRuntimeFiles) {
        if (-not (Test-Path -LiteralPath (Join-Path $ShellBundle $relative) -PathType Leaf)) {
            Fail "the Flutter build did not produce the required runtime file: $(Join-Path $ShellBundle $relative)"
        }
    }
    $assets = Join-Path $ShellBundle 'data\flutter_assets'
    if (-not (Test-Path -LiteralPath $assets -PathType Container)) {
        Fail "the Flutter build did not produce $assets"
    }
    if (@(Get-ChildItem -LiteralPath $assets -Recurse -File -Force).Count -eq 0) {
        Fail "the Flutter build produced an empty $assets"
    }

    Write-Host '[3/4] Staging the payload...'
    New-StageDirectories

    # Copying a hidden/System/reparse directory would launder its attribute in the stage (the
    # destination directory is created normally), so the bundle is preflighted as a tree, not just
    # as the files the copy loop enumerates.
    Get-ChildItem -LiteralPath $ShellBundle -Recurse -Force | ForEach-Object {
        $relative = Get-RelativeTo $_.FullName $ShellBundle
        if ($null -eq $relative) { Fail "refusing a shell-bundle path outside the bundle: $($_.FullName)" }
        if ($_.Attributes -band [IO.FileAttributes]::ReparsePoint -or
            $_.Attributes -band [IO.FileAttributes]::Hidden -or
            $_.Attributes -band [IO.FileAttributes]::System) {
            Fail "refusing a hidden/System/reparse shell-bundle entry: $relative"
        }
    }

    # The shell bundle: the exe, its data folder, and the runtime DLLs Flutter builds beside it.
    # Debug symbols are never part of a release payload.
    $skip = @('*.pdb', '*.lib', '*.exp', '*.ilk', '*.obj')
    Get-ChildItem -LiteralPath $ShellBundle -Recurse -File -Force | ForEach-Object {
        $item = $_
        $relative = Get-RelativeTo $item.FullName $ShellBundle
        if ($null -eq $relative) { Fail "refusing a shell-bundle path outside the bundle: $($item.FullName)" }
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint -or
            $item.Attributes -band [IO.FileAttributes]::Hidden -or
            $item.Attributes -band [IO.FileAttributes]::System) {
            Fail "refusing a hidden/reparse shell-bundle file: $relative"
        }
        $skipIt = $false
        foreach ($pattern in $skip) {
            if ($relative -like $pattern) { $skipIt = $true }
        }
        if (-not $skipIt) {
            $destination = Join-Path $StageDir $relative.Replace('/', '\')
            $parent = Split-Path -Parent $destination
            if (-not (Test-Path -LiteralPath $parent)) { New-Item -ItemType Directory -Path $parent -Force | Out-Null }
            Copy-Item -LiteralPath $item.FullName -Destination $destination -Force
        }
    }
    Assert-RegularSource $CoreExe 'seal-core.exe'
    Assert-RegularSource $DirectMl 'DirectML.dll'
    Copy-Item -LiteralPath $CoreExe -Destination (Join-Path $StageDir 'seal-core.exe') -Force
    Copy-Item -LiteralPath $DirectMl -Destination (Join-Path $StageDir 'DirectML.dll') -Force

    foreach ($name in $ModelNames) {
        $source = Join-Path $ModelsDir $name
        Assert-RegularSource $source "model $name"
        Copy-Item -LiteralPath $source -Destination (Join-Path $StageDir "models\$name") -Force
    }

    # The shipped config is templates + release-maintained dictionaries only. A real local.yaml is
    # never copied (the core seeds it from local.yaml.example on first run).
    foreach ($pair in @(
        @{ Source = 'defaults.yaml'; Destination = 'config\defaults.yaml.example' },
        @{ Source = 'local.yaml.example'; Destination = 'config\local.yaml.example' },
        @{ Source = 'attributes.yaml'; Destination = 'config\attributes.yaml' },
        @{ Source = 'attributes.us.yaml'; Destination = 'config\attributes.us.yaml.example' }
    )) {
        $source = Join-Path $ConfigDir $pair.Source
        Assert-RegularSource $source "config source $($pair.Source)"
        Copy-Item -LiteralPath $source -Destination (Join-Path $StageDir $pair.Destination) -Force
    }

    Assert-RegularSource $PetData 'docs/pet-data.csv'
    Copy-Item -LiteralPath $PetData -Destination (Join-Path $StageDir 'docs\pet-data.csv') -Force
    foreach ($name in $GuideNames) {
        $source = Join-Path $DocsDir $name
        if (Test-Path -LiteralPath $source) {
            Assert-RegularSource $source "release doc $name"
            Copy-Item -LiteralPath $source -Destination (Join-Path $StageDir "docs\$name") -Force
        } else {
            Write-Warning "release doc not written yet, not staged: $source"
        }
    }

    Write-Host '[4/4] Stage complete.'
    Write-Host ''
    Write-StageSummary
    Write-Host ''
}

function Write-Hash([string]$path) {
    $hash = Get-FileHash -LiteralPath $path -Algorithm SHA256
    $size = (Get-Item -LiteralPath $path).Length
    Write-Host ("  {0}" -f $path)
    Write-Host ("    bytes:  {0}" -f $size)
    Write-Host ("    sha256: {0}" -f $hash.Hash)
}

function Assert-Selftest([bool]$condition, [string]$label) {
    if ($condition) {
        Write-Host "[ok] $label"
    } else {
        Write-Host "[FAIL] $label"
        $script:SelftestFailures++
    }
}

function Invoke-Selftest {
    Write-Host '=============================================================='
    Write-Host '  Seal Tools v3 - publish selftest (no build)'
    Write-Host '=============================================================='
    $script:SelftestFailures = 0
    $temp = Join-Path ([IO.Path]::GetTempPath()) ('seal-publish-selftest-' + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $temp -Force | Out-Null
    try {
        Write-Host 'Version schema:'
        $badVersions = @('', 'v3', '3.0', 'v3.0.0.0', '..', '*', '\..\..\v3\config', 'v3.0\..\..\v3\config', 'v3.0;')
        foreach ($candidate in $badVersions) {
            $errorLine = Test-VersionValue $candidate
            Assert-Selftest ([bool]$errorLine) "refuses Version '$candidate'"
        }
        Assert-Selftest ($null -eq (Test-VersionValue 'v3.0')) 'accepts Version v3.0'
        $validPaths = Get-ReleasePaths 'v3.0'
        Assert-Selftest (-not $validPaths.Error) 'v3.0 release paths resolve'
        $traversal = Join-Path $DistDir 'SealTools-\..\..\v3\config'
        Assert-Selftest ([bool](Test-DistChild $traversal 'stage directory')) 'refuses a stage path outside dist (traversal proof)'
        Assert-Selftest ([bool](Test-DistChild (Join-Path $DistDir 'SealTools-v3.0\..\..\v3\config') 'stage directory')) 'refuses a normalized stage path outside dist'

        Write-Host ''
        Write-Host 'Forbidden files (global deny):'
        $stage = Join-Path $temp 'stage'
        $forbidden = @(
            'data/flutter_assets/assets/config/local.yaml',
            'data/local.yaml',
            'data/logs/tool.log',
            'data/calib_gem_result.png',
            'data/.env'
        )
        foreach ($relative in $forbidden) {
            $path = Join-Path $stage $relative.Replace('/', '\')
            $parent = Split-Path -Parent $path
            if (-not (Test-Path -LiteralPath $parent)) { New-Item -ItemType Directory -Path $parent -Force | Out-Null }
            Set-Content -LiteralPath $path -Value 'planted' -Encoding ASCII
        }
        $safety = @(Get-StageSafetyProblems $stage)
        foreach ($relative in $forbidden) {
            $hit = @($safety | Where-Object { $_ -like "*$relative*" }).Count -gt 0
            Assert-Selftest $hit "refuses $relative"
        }

        Write-Host ''
        Write-Host 'Hidden/System attributes:'
        $hidden = Join-Path $stage 'hidden.txt'
        Set-Content -LiteralPath $hidden -Value 'planted' -Encoding ASCII
        (Get-Item -LiteralPath $hidden).Attributes = [IO.FileAttributes]::Hidden
        $safety = @(Get-StageSafetyProblems $stage)
        Assert-Selftest (@($safety | Where-Object { $_ -like '*Hidden*' }).Count -gt 0) 'refuses a hidden file'

        Write-Host ''
        Write-Host 'Reparse points:'
        $link = Join-Path $stage 'link'
        $linkMade = $false
        try {
            New-Item -ItemType Junction -Path $link -Target $temp -ErrorAction Stop | Out-Null
            $linkMade = $true
        } catch {
            Write-Host '[skip] this environment cannot create a junction; reparse refusal is code-reviewed'
        }
        if ($linkMade) {
            $safety = @(Get-StageSafetyProblems $stage)
            Assert-Selftest (@($safety | Where-Object { $_ -like '*reparse*' }).Count -gt 0) 'refuses a reparse point'
        }

        Write-Host ''
        Write-Host 'Stage audit:'
        $auditStage = Join-Path $temp 'stage-audit'
        $requiredFiles = @('SealTools.exe', 'seal-core.exe', 'DirectML.dll')
        foreach ($runtime in $FlutterRuntimeFiles) { $requiredFiles += $runtime.Replace('\', '/') }
        $requiredFiles += @(
            'models/ch_PP-OCRv4_det_infer.onnx',
            'models/ch_PP-OCRv4_rec_infer.onnx',
            'models/ch_ppocr_mobile_v2.0_cls_infer.onnx',
            'config/defaults.yaml.example',
            'config/local.yaml.example',
            'config/attributes.yaml',
            'config/attributes.us.yaml.example',
            'docs/pet-data.csv'
        )
        foreach ($name in $GuideNames) { $requiredFiles += "docs/$name" }
        foreach ($relative in $requiredFiles) {
            $path = Join-Path $auditStage $relative.Replace('/', '\')
            $parent = Split-Path -Parent $path
            if (-not (Test-Path -LiteralPath $parent)) { New-Item -ItemType Directory -Path $parent -Force | Out-Null }
            Set-Content -LiteralPath $path -Value 'placeholder' -Encoding ASCII
        }
        $allowedAsset = Join-Path $auditStage 'data\flutter_assets\app.so'
        New-Item -ItemType Directory -Path (Split-Path -Parent $allowedAsset) -Force | Out-Null
        Set-Content -LiteralPath $allowedAsset -Value 'placeholder' -Encoding ASCII
        $auditProblems = @(Get-StageAuditProblems $auditStage)
        if ($auditProblems.Count -gt 0) { $auditProblems | ForEach-Object { Write-Host "  audit: $_" } }
        Assert-Selftest ($auditProblems.Count -eq 0) 'accepts a complete fake stage'
        $plantedLocal = Join-Path $auditStage 'data\flutter_assets\assets\config\local.yaml'
        New-Item -ItemType Directory -Path (Split-Path -Parent $plantedLocal) -Force | Out-Null
        Set-Content -LiteralPath $plantedLocal -Value 'planted' -Encoding ASCII
        $auditProblems = @(Get-StageAuditProblems $auditStage)
        Assert-Selftest (@($auditProblems | Where-Object { $_ -like '*local.yaml*' }).Count -gt 0) 'stage audit refuses a planted local.yaml under data'

        Write-Host ''
        Write-Host 'Post-zip audit:'
        $zipRoot = Join-Path $temp 'ziproot'
        $top = Join-Path $zipRoot 'Tiny'
        New-Item -ItemType Directory -Path $top -Force | Out-Null
        Set-Content -LiteralPath (Join-Path $top 'expected.txt') -Value 'ok' -Encoding ASCII
        Set-Content -LiteralPath (Join-Path $top 'stray.txt') -Value 'stray' -Encoding ASCII
        $zip = Join-Path $temp 'stray.zip'
        Compress-Archive -LiteralPath $top -DestinationPath $zip -Force
        $zipProblems = Get-ZipProblems $zip 'Tiny' @('Tiny/expected.txt')
        Assert-Selftest (@($zipProblems | Where-Object { $_ -like '*UNEXPECTED ZIP ENTRY*stray*' }).Count -gt 0) 'post-zip audit rejects a stray entry'
        $strayFiles = @($zipProblems | Where-Object { $_ -like '*stray.txt*' })
        Assert-Selftest ($strayFiles.Count -gt 0) 'post-zip audit names the stray entry'

        Write-Host ''
        Write-Host 'Firmware audit:'
        $firmware = Join-Path $temp 'firmware\seal_mouse'
        New-Item -ItemType Directory -Path $firmware -Force | Out-Null
        Set-Content -LiteralPath (Join-Path $firmware 'seal_mouse.ino') -Value '// sketch' -Encoding ASCII
        $cleanFirmware = @(Get-FirmwareSourceProblems $firmware)
        Assert-Selftest ($cleanFirmware.Count -eq 0) 'accepts the intended firmware file set'
        Set-Content -LiteralPath (Join-Path $firmware 'notes.txt') -Value 'stray' -Encoding ASCII
        $dirtyFirmware = @(Get-FirmwareSourceProblems $firmware)
        Assert-Selftest (@($dirtyFirmware | Where-Object { $_ -like '*notes.txt*' }).Count -gt 0) 'rejects a planted firmware stray'
    } finally {
        if (Test-Path -LiteralPath $temp) {
            [GC]::Collect()
            Start-Sleep -Milliseconds 100
            Remove-Item -LiteralPath $temp -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
    Write-Host ''
    if ($script:SelftestFailures -gt 0) {
        Write-Host "[!] Selftest failed: $($script:SelftestFailures) check(s) failed."
        exit 1
    }
    Write-Host 'Selftest passed: version schema, denial rules, hidden/reparse, post-zip and firmware guards.'
    exit 0
}

# ---------------------------------------------------------------------------------------------
# Command mode and validated release paths (version schema BEFORE any path is built).
# ---------------------------------------------------------------------------------------------
$ModeCount = 0
if ($Verify) { $ModeCount++ }
if ($Stage) { $ModeCount++ }
if ($Release) { $ModeCount++ }
if ($Selftest) { $ModeCount++ }
if ($ModeCount -eq 0) { $Verify = $true; $ModeCount = 1 }
if ($ModeCount -gt 1) {
    Write-Host '[!] Choose exactly one mode: -Verify, -Stage, -Release or -Selftest.'
    exit 1
}

$releasePaths = Get-ReleasePaths $Version
if ($releasePaths.Error) { Fail $releasePaths.Error }
$StageDir = $releasePaths.Stage
$AppZip = $releasePaths.AppZip
$FirmwareZip = $releasePaths.FirmwareZip

if ($ModelsDir -eq "") {
    $ModelsDir = Join-Path $V3 'models'
} elseif (-not [System.IO.Path]::IsPathRooted($ModelsDir)) {
    $ModelsDir = Join-Path $Repo $ModelsDir
}

if ($Selftest) {
    Invoke-Selftest
}

$mode = 'verify'
if ($Stage) { $mode = 'stage' }
if ($Release) { $mode = 'release' }
Write-Environment $mode

if ($Verify) {
    $problems = Read-SourceProblems
    Write-Host 'Release sources:'
    $rows = @(
        @{ Name = 'shell bundle'; Path = $ShellBundle },
        @{ Name = 'shell exe'; Path = $ShellExe },
        @{ Name = 'seal-core.exe'; Path = $CoreExe },
        @{ Name = 'DirectML.dll'; Path = $DirectMl },
        @{ Name = 'models dir'; Path = $ModelsDir },
        @{ Name = 'config/local.yaml.example'; Path = (Join-Path $ConfigDir 'local.yaml.example') },
        @{ Name = 'config/defaults.yaml'; Path = (Join-Path $ConfigDir 'defaults.yaml') },
        @{ Name = 'config/attributes.yaml'; Path = (Join-Path $ConfigDir 'attributes.yaml') },
        @{ Name = 'config/attributes.us.yaml'; Path = (Join-Path $ConfigDir 'attributes.us.yaml') },
        @{ Name = 'docs/pet-data.csv'; Path = $PetData },
        @{ Name = 'arduino/seal_mouse'; Path = $FirmwareIno }
    )
    foreach ($row in $rows) {
        $state = 'MISSING'
        if (Test-Path -LiteralPath $row.Path) { $state = 'ok' }
        Write-Host ("  {0,-30} {1,-8} {2}" -f $row.Name, $state, $row.Path)
    }
    foreach ($name in $GuideNames) {
        $path = Join-Path $DocsDir $name
        $state = 'MISSING'
        if (Test-Path -LiteralPath $path) { $state = 'ok' }
        Write-Host ("  {0,-30} {1,-8} {2}" -f "docs/$name", $state, $path)
    }
    Write-Host ''
    Write-Host "Planned payload -> $StageDir\ (one top folder):"
    Write-Host '  SealTools.exe + data/ (app.so, icudtl.dat, flutter_assets/) + flutter_windows.dll'
    Write-Host '  native_assets.json (the Flutter native-asset manifest)'
    Write-Host '  seal-core.exe, DirectML.dll'
    Write-Host '  models/<the three PP-OCRv4 .onnx files>'
    Write-Host '  config/defaults.yaml.example, local.yaml.example, attributes.yaml, attributes.us.yaml.example'
    Write-Host '  docs/pet-data.csv, USER_GUIDE.md, USER_GUIDE.zh-TW.md, INSTALL.md, THIRD-PARTY-NOTICES.md'
    Write-Host "Firmware -> $FirmwareZip (seal_mouse/ + seal_mouse.ino)"
    Write-Host "App zip  -> $AppZip"
    Write-Host ''
    if ($problems.Count -gt 0) {
        Write-Host '[!] Missing release inputs:'
        $problems | ForEach-Object { Write-Host "  - $_" }
        Write-Host ''
        Write-Host "[!] -Verify failed: $($problems.Count) release input(s) missing. Nothing was built or staged."
        exit 1
    }
    Write-Host 'All release sources are present.'
    exit 0
}

if ($Release) {
    if (-not $AcceptDirectMLRedistribution) {
        Fail @'
DirectML redistribution is an OPEN release input. DirectML.dll is redistributed under Microsoft's
terms (the OCR research record cites the static distribution); confirm those terms before publishing,
then pass -AcceptDirectMLRedistribution. Refusing to zip a public payload without that acknowledgement.
'@
    }
    Invoke-Stage
    Test-StageAudit
    Write-Host '[release] Zipping the app payload (one top folder)...'
    if (-not (Test-Path -LiteralPath $DistDir)) { New-Item -ItemType Directory -Path $DistDir -Force | Out-Null }
    if (Test-Path -LiteralPath $AppZip) { Remove-Item -LiteralPath $AppZip -Force }
    Compress-Archive -LiteralPath $StageDir -DestinationPath $AppZip -Force
    Test-AppZipAudit
    Write-Host '[release] Auditing and zipping the unchanged firmware sketch...'
    if (Test-Path -LiteralPath $FirmwareZip) { Remove-Item -LiteralPath $FirmwareZip -Force }
    $firmwareProblems = Get-FirmwareSourceProblems $FirmwareSketch
    if ($firmwareProblems.Count -gt 0) {
        Write-Host '[!] The firmware sketch failed its audit:'
        $firmwareProblems | ForEach-Object { Write-Host "  - $_" }
        Fail 'Nothing published. The firmware zip only carries the unchanged sketch files.'
    }
    Compress-Archive -LiteralPath $FirmwareSketch -DestinationPath $FirmwareZip -Force
    Test-FirmwareZip
    Write-Host ''
    Write-Host 'Distributables (SHA-256):'
    Write-Hash $AppZip
    Write-Hash $FirmwareZip
    exit 0
}

Invoke-Stage
Write-Host 'Staged only (-Stage): nothing was zipped. Use -Release (with -AcceptDirectMLRedistribution) to package.'
exit 0
