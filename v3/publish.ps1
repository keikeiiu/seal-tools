# Seal Tools v3.0 publish pipeline (CUTOVER.md item 1 / the packaging work item).
#
#   .\v3\publish.ps1 -Verify    (default) check every release input, print what WOULD be staged
#   .\v3\publish.ps1 -Stage     build core + shell, stage dist\SealTools-v3.0\
#   .\v3\publish.ps1 -Release   -Stage + allow-list audit + the two zips + SHA-256
#
# -Release refuses until -AcceptDirectMLRedistribution is passed: DirectML.dll is redistributed
# under Microsoft's terms, still an explicit release input (see v3/docs/records/OCR-RESEARCH.md and
# v3/docs/backlog/OPEN.md). Nothing here writes to a git worktree.

[CmdletBinding()]
param(
    [switch]$Verify,
    [switch]$Stage,
    [switch]$Release,
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
$StageDir = Join-Path $DistDir "SealTools-$Version"
$AppZip = "$StageDir.zip"
$FirmwareZip = Join-Path $DistDir "SealTools-$Version-firmware.zip"

$ModelNames = @(
    'ch_PP-OCRv4_det_infer.onnx',
    'ch_PP-OCRv4_rec_infer.onnx',
    'ch_ppocr_mobile_v2.0_cls_infer.onnx'
)
$GuideNames = @('USER_GUIDE.md', 'USER_GUIDE.zh-TW.md', 'INSTALL.md')

if ($ModelsDir -eq "") {
    $ModelsDir = Join-Path $V3 'models'
} elseif (-not [System.IO.Path]::IsPathRooted($ModelsDir)) {
    $ModelsDir = Join-Path $Repo $ModelsDir
}

$ModeCount = 0
if ($Verify) { $ModeCount++ }
if ($Stage) { $ModeCount++ }
if ($Release) { $ModeCount++ }
if ($ModeCount -eq 0) { $Verify = $true; $ModeCount = 1 }
if ($ModeCount -gt 1) {
    Write-Host '[!] Choose exactly one mode: -Verify, -Stage or -Release.'
    exit 1
}

function Fail([string]$message) {
    Write-Host ''
    Write-Host "[!] $message" -ForegroundColor Red
    exit 1
}

function Write-Environment([string]$mode) {
    Write-Host '=============================================================='
    Write-Host "  Seal Tools $Version - v3 publish ($mode)"
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
    # The inputs the BUILD consumes. The build outputs (core exe, DirectML, shell bundle) and the
    # docs are checked after their own steps, so a first -Stage does not fail for not-yet-built files.
    $problems = New-Object System.Collections.Generic.List[string]
    if (-not (Test-Path $ModelsDir)) {
        $hint = ''
        $v2Models = Join-Path $Repo 'v2\models'
        if (Test-Path $v2Models) {
            $hint = " (the same three files exist at v2\models; copy them into v3\models or pass -ModelsDir v2\models)"
        }
        $problems.Add("models directory missing: $ModelsDir$hint")
    } else {
        foreach ($name in $ModelNames) {
            if (-not (Test-Path (Join-Path $ModelsDir $name))) {
                $problems.Add("model missing: $(Join-Path $ModelsDir $name)")
            }
        }
    }
    foreach ($name in @('defaults.yaml', 'local.yaml.example', 'attributes.yaml', 'attributes.us.yaml')) {
        $path = Join-Path $ConfigDir $name
        if (-not (Test-Path $path)) { $problems.Add("config source missing: $path") }
    }
    if (-not (Test-Path $PetData)) {
        $problems.Add("pet feeding table missing: $PetData (copy it verbatim from v2\docs\pet-data.csv)")
    }
    if (-not (Test-Path $FirmwareIno)) {
        $problems.Add("firmware sketch missing: $FirmwareIno")
    }
    return $problems
}

function Read-SourceProblems {
    $problems = New-Object System.Collections.Generic.List[string]

    # The Flutter release bundle and the two files the payload exists for.
    if (-not (Test-Path $ShellBundle)) {
        $problems.Add("shell release bundle missing: $ShellBundle (run: flutter build windows --release in v3\shell)")
    } else {
        if (-not (Test-Path $ShellExe)) {
            $legacy = Join-Path $ShellBundle 'seal_shell.exe'
            if (Test-Path $legacy) {
                $problems.Add("shell executable is seal_shell.exe, not SealTools.exe (CUTOVER item 2 naming is not done yet): $ShellBundle")
            } else {
                $problems.Add("shell executable missing: $ShellExe")
            }
        }
        if (-not (Test-Path (Join-Path $ShellBundle 'data'))) {
            $problems.Add("Flutter data folder missing: $(Join-Path $ShellBundle 'data')")
        }
    }

    if (-not (Test-Path $CoreExe)) {
        $problems.Add("seal-core.exe missing: $CoreExe (run: cargo build --release -p seal-core in v3\core)")
    }
    if (-not (Test-Path $DirectMl)) {
        $hint = ''
        if (Test-Path $DebugDirectMl) {
            $hint = " (a debug copy exists at $DebugDirectMl; the release build must place its own)"
        }
        $problems.Add("DirectML.dll missing: $DirectMl$hint")
    }

    if (-not (Test-Path $ModelsDir)) {
        $hint = ''
        $v2Models = Join-Path $Repo 'v2\models'
        if (Test-Path $v2Models) {
            $hint = " (the same three files exist at v2\models; copy them into v3\models or pass -ModelsDir v2\models)"
        }
        $problems.Add("models directory missing: $ModelsDir$hint")
    } else {
        foreach ($name in $ModelNames) {
            $path = Join-Path $ModelsDir $name
            if (-not (Test-Path $path)) {
                $problems.Add("model missing: $path")
            }
        }
    }

    foreach ($name in @('defaults.yaml', 'local.yaml.example', 'attributes.yaml', 'attributes.us.yaml')) {
        $path = Join-Path $ConfigDir $name
        if (-not (Test-Path $path)) {
            $problems.Add("config source missing: $path")
        }
    }

    if (-not (Test-Path $PetData)) {
        $problems.Add("pet feeding table missing: $PetData (copy it verbatim from v2\docs\pet-data.csv)")
    }
    if (-not (Test-Path $FirmwareIno)) {
        $problems.Add("firmware sketch missing: $FirmwareIno")
    }
    foreach ($name in $GuideNames) {
        $path = Join-Path $DocsDir $name
        if (-not (Test-Path $path)) {
            $problems.Add("release doc missing: $path")
        }
    }
    return $problems
}

function New-StageDirectories {
    if (Test-Path $StageDir) {
        Remove-Item -Path $StageDir -Recurse -Force
    }
    New-Item -ItemType Directory -Path $StageDir -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $StageDir 'models') -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $StageDir 'config') -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $StageDir 'docs') -Force | Out-Null
}

function Write-StageSummary {
    Write-Host 'Payload staged in:'
    Get-ChildItem $StageDir -Recurse -File |
        ForEach-Object {
            $relative = $_.FullName.Substring($StageDir.Length + 1).Replace('\', '/')
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

    if (-not (Test-Path $CoreExe)) { Fail "the release build did not produce $CoreExe" }
    if (-not (Test-Path $DirectMl)) { Fail "the release build did not place DirectML.dll at $DirectMl" }
    if (-not (Test-Path $ShellExe)) {
        $legacy = Join-Path $ShellBundle 'seal_shell.exe'
        if (Test-Path $legacy) {
            Fail "the Flutter bundle carries seal_shell.exe, not SealTools.exe - apply CUTOVER item 2 (BINARY_NAME/window title/pubspec 3.0.0+1) first."
        }
        Fail "the Flutter build did not produce $ShellExe"
    }
    if (-not (Test-Path (Join-Path $ShellBundle 'data'))) { Fail "the Flutter build did not produce $(Join-Path $ShellBundle 'data')" }

    Write-Host '[3/4] Staging the payload...'
    New-StageDirectories

    # The shell bundle: the exe, its data folder, and the runtime DLLs Flutter builds beside it.
    # Debug symbols and import libraries are never part of a release payload.
    $skip = @('*.pdb', '*.lib', '*.exp', '*.ilk', '*.obj')
    Get-ChildItem $ShellBundle -Recurse -File | ForEach-Object {
        $relative = $_.FullName.Substring($ShellBundle.Length + 1)
        $skipIt = $false
        foreach ($pattern in $skip) {
            if ($relative -like $pattern) { $skipIt = $true }
        }
        if (-not $skipIt) {
            $destination = Join-Path $StageDir $relative
            $parent = Split-Path -Parent $destination
            if (-not (Test-Path $parent)) { New-Item -ItemType Directory -Path $parent -Force | Out-Null }
            Copy-Item $_.FullName $destination -Force
        }
    }
    Copy-Item $CoreExe (Join-Path $StageDir 'seal-core.exe') -Force
    Copy-Item $DirectMl (Join-Path $StageDir 'DirectML.dll') -Force

    foreach ($name in $ModelNames) {
        Copy-Item (Join-Path $ModelsDir $name) (Join-Path $StageDir "models\$name") -Force
    }

    # The shipped config is templates + release-maintained dictionaries only. A real local.yaml is
    # never copied (the core seeds it from local.yaml.example on first run).
    Copy-Item (Join-Path $ConfigDir 'defaults.yaml') (Join-Path $StageDir 'config\defaults.yaml.example') -Force
    Copy-Item (Join-Path $ConfigDir 'local.yaml.example') (Join-Path $StageDir 'config\local.yaml.example') -Force
    Copy-Item (Join-Path $ConfigDir 'attributes.yaml') (Join-Path $StageDir 'config\attributes.yaml') -Force
    Copy-Item (Join-Path $ConfigDir 'attributes.us.yaml') (Join-Path $StageDir 'config\attributes.us.yaml.example') -Force

    Copy-Item $PetData (Join-Path $StageDir 'docs\pet-data.csv') -Force
    foreach ($name in $GuideNames) {
        $source = Join-Path $DocsDir $name
        if (Test-Path $source) {
            Copy-Item $source (Join-Path $StageDir "docs\$name") -Force
        } else {
            Write-Warning "release doc not written yet, not staged: $source"
        }
    }

    Write-Host '[4/4] Stage complete.'
    Write-Host ''
    Write-StageSummary
    Write-Host ''
}

function Test-StageAudit {
    $problems = New-Object System.Collections.Generic.List[string]

    $required = @(
        'SealTools.exe',
        'seal-core.exe',
        'DirectML.dll',
        'data',
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
        if (-not (Test-Path (Join-Path $StageDir $name))) {
            $problems.Add("MISSING: $name")
        }
    }

    $allowedRoot = @('SealTools.exe', 'seal-core.exe', 'DirectML.dll', 'flutter_windows.dll')
    $allowedConfig = @(
        'config/defaults.yaml.example',
        'config/local.yaml.example',
        'config/attributes.yaml',
        'config/attributes.us.yaml.example'
    )
    $allowedDocs = @('docs/pet-data.csv') + ($GuideNames | ForEach-Object { "docs/$_" })
    $allowedModels = $ModelNames | ForEach-Object { "models/$_" }

    Get-ChildItem $StageDir -Recurse -File | ForEach-Object {
        $relative = $_.FullName.Substring($StageDir.Length + 1).Replace('\', '/')
        if ($relative -like 'data/*') { return }
        if ($allowedRoot -contains $relative) { return }
        if ($allowedConfig -contains $relative) { return }
        if ($allowedDocs -contains $relative) { return }
        if ($allowedModels -contains $relative) { return }
        $problems.Add("NOT ON THE ALLOW-LIST: $relative")
    }

    # Explicit player-state/debris belt-and-braces, even though the allow-list already refuses them.
    foreach ($prefix in @('config/local.yaml', 'logs/')) {
        if (Test-Path (Join-Path $StageDir $prefix)) { $problems.Add("FORBIDDEN: $prefix") }
    }
    Get-ChildItem $StageDir -Recurse -File |
        Where-Object { $_.Name -like '*.backup-*' -or $_.Name -like '*.corrupt-backup' -or $_.Name -like '*.pdb' } |
        ForEach-Object {
            $relative = $_.FullName.Substring($StageDir.Length + 1).Replace('\', '/')
            $problems.Add("FORBIDDEN: $relative")
        }

    if ($problems.Count -gt 0) {
        Write-Host '[!] The staged payload failed the release audit:'
        $problems | ForEach-Object { Write-Host "  - $_" }
        Fail 'Nothing zipped. The allow-list above is the rule: no player state, no debug debris, no unknown file.'
    }
    Write-Host 'Allow-list audit: PASS'
}

function Write-Hash([string]$path) {
    $hash = Get-FileHash -Path $path -Algorithm SHA256
    $size = (Get-Item $path).Length
    Write-Host ("  {0}" -f $path)
    Write-Host ("    bytes:  {0}" -f $size)
    Write-Host ("    sha256: {0}" -f $hash.Hash)
}

Write-Environment ($(if ($Verify) { 'verify' } elseif ($Stage) { 'stage' } else { 'release' }))

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
        if (Test-Path $row.Path) { $state = 'ok' }
        Write-Host ("  {0,-30} {1,-8} {2}" -f $row.Name, $state, $row.Path)
    }
    foreach ($name in $GuideNames) {
        $path = Join-Path $DocsDir $name
        $state = 'MISSING'
        if (Test-Path $path) { $state = 'ok' }
        Write-Host ("  {0,-30} {1,-8} {2}" -f "docs/$name", $state, $path)
    }
    Write-Host ''
    Write-Host "Planned payload -> $StageDir\ (one top folder):"
    Write-Host '  SealTools.exe + data/ + flutter_windows.dll  (the shell bundle)'
    Write-Host '  seal-core.exe'
    Write-Host '  DirectML.dll'
    Write-Host '  models/<the three PP-OCRv4 .onnx files>'
    Write-Host '  config/defaults.yaml.example, local.yaml.example, attributes.yaml, attributes.us.yaml.example'
    Write-Host '  docs/pet-data.csv, USER_GUIDE.md, USER_GUIDE.zh-TW.md, INSTALL.md'
    Write-Host "Firmware -> $FirmwareZip (unchanged sketch)"
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
    if (-not (Test-Path $DistDir)) { New-Item -ItemType Directory -Path $DistDir -Force | Out-Null }
    if (Test-Path $AppZip) { Remove-Item $AppZip -Force }
    Compress-Archive -Path $StageDir -DestinationPath $AppZip -Force
    Write-Host '[release] Zipping the unchanged firmware sketch...'
    if (Test-Path $FirmwareZip) { Remove-Item $FirmwareZip -Force }
    Compress-Archive -Path $FirmwareSketch -DestinationPath $FirmwareZip -Force
    Write-Host ''
    Write-Host 'Distributables (SHA-256):'
    Write-Hash $AppZip
    Write-Hash $FirmwareZip
    exit 0
}

Invoke-Stage
Write-Host 'Staged only (-Stage): nothing was zipped. Use -Release (with -AcceptDirectMLRedistribution) to package.'
exit 0
