# Seal Tools v3.0 publish pipeline (CUTOVER.md item 1 / the packaging work item).
#
#   .\v3\publish.ps1 -Verify    (default) check every release input, print what WOULD be staged
#   .\v3\publish.ps1 -Stage     gates + build core + shell, stage dist\SealTools-vX.Y.Z\
#   .\v3\publish.ps1 -Release   -Stage + allow-list audit + the two zips + checksums
#   .\v3\publish.ps1 -Selftest  run the packaging guards without a release build (task-58)
#
# -Stage/-Release refuse a dirty git worktree unless -AllowDirty is passed, run the Rust tests and the
# shell quality gate first, and build with --locked. The stage carries build-info.json (commit,
# toolchains, payload hashes) and dist\SHA256SUMS.txt covers both zips. Code signing is NOT performed:
# it needs an external code-signing certificate, so the script prints the exact TODO instead.
#
# The release name (vX.Y.Z) is DERIVED from v3\shell\pubspec.yaml (SHELL-F10); passing -Version is
# only an assertion that must equal the derived name, never an override.
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
#   * -Stage/-Release refuse a dirty worktree (unless -AllowDirty), run the Rust and shell gates,
#     build with --locked, write build-info.json into the stage and dist\SHA256SUMS.txt beside the
#     zips. Signing is a documented TODO: it needs an external code-signing certificate.
#   * -Selftest exercises these guards with temp directories, a temp zip and no build.

[CmdletBinding()]
param(
    [switch]$Verify,
    [switch]$Stage,
    [switch]$Release,
    [switch]$Selftest,
    [switch]$AcceptDirectMLRedistribution,
    # SHELL-F9: -Stage/-Release refuse a dirty worktree, run the Rust and shell gates, and build with
    # --locked. -AllowDirty is the explicit escape for a local packaging experiment, never for a
    # release; it does not skip the gates.
    [switch]$AllowDirty,
    [string]$ModelsDir = "",
    [string]$FlutterRoot = $(if ($env:FLUTTER_ROOT) { $env:FLUTTER_ROOT } else { 'C:\src\flutter' }),
    # Derived from v3/shell/pubspec.yaml (SHELL-F10). Passing an explicit value is allowed only when
    # it names the same version; a mismatch is refused before any path is built.
    [string]$Version = ""
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
# One product version, three files that must agree with it (SHELL-F10): pubspec is canonical, the
# generated Dart constant feeds the app bar, and the Cargo workspace version is what `hello` reports.
$PubspecPath = Join-Path $ShellDir 'pubspec.yaml'
$VersionDartPath = Join-Path $ShellDir 'lib\app\version.dart'
$CoreManifestPath = Join-Path $CoreDir 'Cargo.toml'

$VersionPattern = '^v\d+\.\d+\.\d+$'
$ModelNames = @(
    'ch_PP-OCRv4_det_infer.onnx',
    'ch_PP-OCRv4_rec_infer.onnx',
    'ch_ppocr_mobile_v2.0_cls_infer.onnx',
    # PP-OCRv6 small: the optional trial set selectable in Setup. v4 remains the default.
    'PP-OCRv6_small_det_infer.onnx',
    'PP-OCRv6_small_rec_infer.onnx',
    'PP-OCRv6_small_rec_dict.txt'
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

# SHELL-F10 — one product version. pubspec.yaml is canonical; these readers exist so the guards are
# testable without touching the real files (the selftest feeds them synthetic text).
function Read-PubspecProductVersion([string]$text) {
    $match = [regex]::Match($text, '(?m)^version:\s*(\d+\.\d+\.\d+)\+\d+\s*$')
    if (-not $match.Success) { return $null }
    return $match.Groups[1].Value
}

function Read-CargoWorkspaceVersion([string]$text) {
    $match = [regex]::Match($text, '(?ms)^\[workspace\.package\].*?^version\s*=\s*"(\d+\.\d+\.\d+)"')
    if (-not $match.Success) { return $null }
    return $match.Groups[1].Value
}

function Read-DartProductVersion([string]$text) {
    $match = [regex]::Match($text, "productVersion\s*=\s*'(\d+\.\d+\.\d+)'")
    if (-not $match.Success) { return $null }
    return $match.Groups[1].Value
}

function Resolve-PublishVersion([string]$requested, [string]$product, [string]$cargo, [string]$dart) {
    if (-not $product) {
        return @{ Error = "v3\shell\pubspec.yaml has no 'version: X.Y.Z+N' line; no path was built." }
    }
    if ($cargo -ne $product) {
        return @{ Error = "Version mismatch: v3\core\Cargo.toml [workspace.package] says '$cargo' but pubspec.yaml says '$product'. Set both to one product version." }
    }
    if ($dart -ne $product) {
        return @{ Error = "Version mismatch: lib\app\version.dart says '$dart' but pubspec.yaml says '$product'. Run: dart run tool/gen_version.dart from v3\shell." }
    }
    $derived = "v$product"
    if ($requested -ne '' -and $requested -ne $derived) {
        return @{ Error = "Version mismatch: -Version '$requested' disagrees with pubspec.yaml's '$product' (expected '$derived')." }
    }
    return @{ Error = $null; Version = $derived; Product = $product }
}

function Get-PublishVersion([string]$requested) {
    foreach ($path in @($PubspecPath, $CoreManifestPath, $VersionDartPath)) {
        if (-not (Test-Path -LiteralPath $path)) {
            return @{ Error = "product version source missing: $path" }
        }
    }
    return Resolve-PublishVersion $requested `
        (Read-PubspecProductVersion (Get-Content -Raw -LiteralPath $PubspecPath)) `
        (Read-CargoWorkspaceVersion (Get-Content -Raw -LiteralPath $CoreManifestPath)) `
        (Read-DartProductVersion (Get-Content -Raw -LiteralPath $VersionDartPath))
}

# SHELL-F9 — release gates and provenance. These helpers are pure where possible so -Selftest can
# exercise the refusal rules without a git worktree, a build or a certificate.
function Test-DirtyTreeAllowed([string]$statusText, [bool]$allowDirty) {
    if ($allowDirty) { return $null }
    $lines = New-Object System.Collections.Generic.List[string]
    foreach ($line in ($statusText -split "`n")) {
        $trimmed = $line.TrimEnd()
        if ($trimmed -ne '') { $lines.Add($trimmed) }
    }
    if ($lines.Count -eq 0) { return $null }
    $sample = ($lines | Select-Object -First 10) -join "`n    "
    return "the worktree has $($lines.Count) uncommitted path(s); commit the release inputs or pass -AllowDirty.`n    $sample"
}

function Invoke-ReleaseGates {
    Write-Host '[gates] cargo test --workspace --locked --offline...'
    Push-Location $CoreDir
    try {
        & cargo test --workspace --locked --offline
        if ($LASTEXITCODE -ne 0) { Fail 'cargo test --workspace failed; refusing to package an untested core.' }
    } finally {
        Pop-Location
    }
    Write-Host '[gates] shell quality gate (tool\check.ps1)...'
    $check = Join-Path $ShellDir 'tool\check.ps1'
    if (-not (Test-Path -LiteralPath $check)) { Fail "shell quality gate missing: $check" }
    try {
        & $check
    } catch {
        Fail "the shell quality gate failed: $($_.Exception.Message)"
    }
    if ($LASTEXITCODE -ne 0) { Fail 'the shell quality gate failed (see the log above).' }
    Write-Host '[gates] PASS'
}

function Get-ToolchainInfo {
    $info = @{ rustc = 'unknown'; cargo = 'unknown'; flutter = 'unknown'; dart = 'unknown' }
    try {
        $value = ((& rustc --version 2>&1 | Out-String).Trim())
        if ($value -ne '') { $info.rustc = ($value -split "`n")[0].Trim() }
    } catch { }
    try {
        $value = ((& cargo --version 2>&1 | Out-String).Trim())
        if ($value -ne '') { $info.cargo = ($value -split "`n")[0].Trim() }
    } catch { }
    try {
        $flutter = Join-Path $FlutterRoot 'bin\flutter.bat'
        if (-not (Test-Path -LiteralPath $flutter)) { $flutter = 'flutter' }
        $value = ((& $flutter --version 2>&1 | Out-String).Trim())
        if ($value -ne '') { $info.flutter = ($value -split "`n")[0].Trim() }
    } catch { }
    try {
        $dart = Join-Path $FlutterRoot 'bin\cache\dart-sdk\bin\dart.exe'
        if (-not (Test-Path -LiteralPath $dart)) { $dart = 'dart' }
        $value = ((& $dart --version 2>&1 | Out-String).Trim())
        if ($value -ne '') { $info.dart = ($value -split "`n")[0].Trim() }
    } catch { }
    return $info
}

function Get-PayloadManifest([string]$stageDir, [string]$excludeRelative) {
    $paths = New-Object System.Collections.Generic.List[string]
    foreach ($relative in @(Get-StageRelativeFiles $stageDir)) {
        if ($excludeRelative -ne '' -and $relative -eq $excludeRelative) { continue }
        $paths.Add($relative)
    }
    $ordered = $paths.ToArray()
    [Array]::Sort($ordered, [StringComparer]::Ordinal)
    $entries = New-Object System.Collections.Generic.List[object]
    foreach ($relative in $ordered) {
        $file = Join-Path $stageDir $relative.Replace('/', '\')
        $entries.Add([pscustomobject]@{
            path = $relative
            bytes = (Get-Item -LiteralPath $file).Length
            sha256 = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()
        })
    }
    return $entries.ToArray()
}

function New-BuildInfo([string]$ProductVersion, [string]$Release, [string]$Commit, [string]$Branch, [bool]$Dirty, [string]$BuiltAtUtc, [hashtable]$Toolchain, [object[]]$Payload) {
    $info = [ordered]@{
        product_version = $ProductVersion
        release = $Release
        git_commit = $Commit
        git_branch = $Branch
        git_dirty = $Dirty
        built_at_utc = $BuiltAtUtc
        toolchain = [ordered]@{
            rustc = [string]$Toolchain['rustc']
            cargo = [string]$Toolchain['cargo']
            flutter = [string]$Toolchain['flutter']
            dart = [string]$Toolchain['dart']
        }
        payload = @($Payload)
    }
    return ($info | ConvertTo-Json -Depth 8)
}

function Write-BuildInfoFile([string]$stageDir, [string]$productVersion, [string]$release, [string]$commit, [string]$branch, [bool]$dirty) {
    $payload = @(Get-PayloadManifest $stageDir 'build-info.json')
    $json = New-BuildInfo -ProductVersion $productVersion -Release $release -Commit $commit -Branch $branch -Dirty $dirty -BuiltAtUtc ([DateTime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ssZ')) -Toolchain (Get-ToolchainInfo) -Payload $payload
    [IO.File]::WriteAllText((Join-Path $stageDir 'build-info.json'), $json + "`n", (New-Object Text.UTF8Encoding $false))
}

function Write-ChecksumFile([string]$path, [string[]]$files) {
    $lines = New-Object System.Collections.Generic.List[string]
    foreach ($file in $files) {
        if (-not (Test-Path -LiteralPath $file)) { Fail "cannot checksum a missing file: $file" }
        $hash = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()
        $lines.Add("$hash  $(Split-Path -Leaf $file)")
    }
    [IO.File]::WriteAllLines($path, $lines.ToArray(), (New-Object Text.UTF8Encoding $false))
}

function Write-SigningTodo {
    Write-Host ''
    Write-Host 'Signing: NOT PERFORMED (SHELL-F9).' -ForegroundColor Yellow
    Write-Host '  The payload is unsigned. SmartScreen and antivirus heuristics will flag a tool that'
    Write-Host '  synthesises input and drives a COM device, and players cannot verify provenance beyond'
    Write-Host '  the SHA-256 file. Authenticode signing needs an external code-signing certificate that'
    Write-Host '  this repository does not carry. Before zipping, sign the staged binaries with:'
    Write-Host '    signtool sign /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 /f <cert.pfx> /p <password> <file>'
    Write-Host '  for SealTools.exe and seal-core.exe, then re-run the release so the checksums describe'
    Write-Host '  the signed files.'
}

function Test-DistChild([string]$path, [string]$label) {    $full = [IO.Path]::GetFullPath($path)
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
    $installerExe = Join-Path $DistDir "SealTools-Setup-$version-x64.exe"
    foreach ($pair in @(
        @{ Label = 'stage directory'; Path = $stage },
        @{ Label = 'app zip'; Path = $appZip },
        @{ Label = 'firmware zip'; Path = $firmwareZip },
        @{ Label = 'installer'; Path = $installerExe }
    )) {
        $errorLine = Test-DistChild $pair.Path $pair.Label
        if ($errorLine) { return @{ Error = $errorLine } }
    }
    return @{ Error = $null; Stage = $stage; AppZip = $appZip; FirmwareZip = $firmwareZip; InstallerExe = $installerExe }
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
    # The portable layout (PLAN-PORTABLE-SETUP): setup/, machine/ and artifacts/ are PLAYER STATE and
    # never ship; user.yaml/meta.yaml are the player's notes and the machine's bookkeeping; the
    # migration backup and the transaction journal are never part of a release.
    if ($relative -like 'config/setup/*' -or
        $relative -like 'config/machine/*' -or
        $relative -like 'config/artifacts/*' -or
        $relative -like 'config/.migration-backup-*' -or
        $relative -like 'config/.seal-txn*' -or
        $relative -eq 'config/user.yaml' -or
        $relative -eq 'config/meta.yaml') {
        return 'portable-layout player state'
    }
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
        'native_assets.json',
        'build-info.json'
    )
    $allowedConfig = @(
        'config/README.md',
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

    $required = @('SealTools.exe', 'seal-core.exe', 'DirectML.dll', 'build-info.json')
    foreach ($runtime in $FlutterRuntimeFiles) { $required += $runtime.Replace('\', '/') }
    $required += @(
        'models/ch_PP-OCRv4_det_infer.onnx',
        'models/ch_PP-OCRv4_rec_infer.onnx',
        'models/ch_ppocr_mobile_v2.0_cls_infer.onnx',
        'models/PP-OCRv6_small_det_infer.onnx',
        'models/PP-OCRv6_small_rec_infer.onnx',
        'models/PP-OCRv6_small_rec_dict.txt',
        'config/README.md',
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
    Write-Host "product: $ProductVersion (release $Version)"
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
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Fail 'cargo is not on PATH; -Stage needs it to build the payload.'
    }
    $flutter = Join-Path $FlutterRoot 'bin\flutter.bat'
    if (-not (Test-Path -LiteralPath $flutter)) {
        Fail "Flutter not found at '$flutter'. Set FLUTTER_ROOT or pass -FlutterRoot."
    }

    # SHELL-F9: a package is built from a committed tree, then gated, then built with the lockfile.
    # The status is captured once so build-info.json can record whether this was a dirty package.
    $statusText = ((& git -C $Repo status --porcelain 2>$null) -join "`n")
    if ($LASTEXITCODE -ne 0) {
        Fail "cannot read git status in $Repo; -Stage refuses to package an unidentifiable tree."
    }
    $worktreeDirty = -not [string]::IsNullOrWhiteSpace($statusText)
    $dirtyProblem = Test-DirtyTreeAllowed $statusText $AllowDirty.IsPresent
    if ($dirtyProblem) { Fail "$dirtyProblem`nA release must be built from a committed tree; commit the inputs or pass -AllowDirty." }

    Invoke-ReleaseGates

    Write-Host '[1/4] Building seal-core (release, --locked)...'
    Push-Location $CoreDir
    try {
        & cargo build --release --locked -p seal-core
        if ($LASTEXITCODE -ne 0) { Fail 'cargo build --release --locked -p seal-core failed (see the log above).' }
    } finally {
        Pop-Location
    }

    # SHELL-F11: the notices that ship in the zip must name every crate this build links. The cargo
    # build above has populated the source cache, so the audit can read each crate's own licence text.
    Write-Host '[notices] Checking the generated Rust-crate attribution...'
    & (Join-Path $V3 'tool\gen-third-party-notices.ps1') -Check
    if ($LASTEXITCODE -ne 0) {
        Fail 'The Rust-crate notices are stale; run v3\tool\gen-third-party-notices.ps1 and commit the result.'
    }

    Write-Host '[2/4] Building the Flutter Windows release bundle (lockfile enforced)...'
    Push-Location $ShellDir
    try {
        & $flutter pub get --offline --enforce-lockfile
        if ($LASTEXITCODE -ne 0) { Fail 'flutter pub get --enforce-lockfile failed; pubspec.lock is stale (see the log above).' }
        & $flutter build windows --release --no-pub
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
        @{ Source = 'README.md'; Destination = 'config\README.md' },
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

    $commit = ((& git -C $Repo rev-parse HEAD 2>$null) | Out-String).Trim()
    $branch = ((& git -C $Repo rev-parse --abbrev-ref HEAD 2>$null) | Out-String).Trim()
    if ($commit -eq '') { $commit = 'unknown'; $branch = 'unknown' }
    Write-BuildInfoFile $StageDir $ProductVersion $Version $commit $branch $worktreeDirty
    Write-Host "[stage] build-info.json records $commit on $branch (dirty: $worktreeDirty)."

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
        $badVersions = @('', 'v3', '3.0.0', 'v3.0', 'v3.0.0.0', '..', '*', '\..\..\v3\config', 'v3.0\..\..\v3\config', 'v3.0;')
        foreach ($candidate in $badVersions) {
            $errorLine = Test-VersionValue $candidate
            Assert-Selftest ([bool]$errorLine) "refuses Version '$candidate'"
        }
        Assert-Selftest ($null -eq (Test-VersionValue 'v3.0.8')) 'accepts Version v3.0.8'
        $validPaths = Get-ReleasePaths 'v3.0.8'
        Assert-Selftest (-not $validPaths.Error) 'v3.0.8 release paths resolve'
        $traversal = Join-Path $DistDir 'SealTools-\..\..\v3\config'
        Assert-Selftest ([bool](Test-DistChild $traversal 'stage directory')) 'refuses a stage path outside dist (traversal proof)'
        Assert-Selftest ([bool](Test-DistChild (Join-Path $DistDir 'SealTools-v3.0\..\..\v3\config') 'stage directory')) 'refuses a normalized stage path outside dist'

        Write-Host ''
        Write-Host 'Product version (one source of truth):'
        Assert-Selftest ((Read-PubspecProductVersion "version: 3.0.8+1") -eq '3.0.8') 'reads the product version from pubspec'
        Assert-Selftest ($null -eq (Read-PubspecProductVersion "version: 3.0")) 'refuses a pubspec version without a build number'
        Assert-Selftest ((Read-CargoWorkspaceVersion "[workspace.package]`nversion = `"3.0.8`"") -eq '3.0.8') 'reads the Cargo workspace version'
        Assert-Selftest ((Read-DartProductVersion "const String productVersion = '3.0.8';") -eq '3.0.8') 'reads the generated Dart product version'
        $agreed = Resolve-PublishVersion '' '3.0.8' '3.0.8' '3.0.8'
        Assert-Selftest ((-not $agreed.Error) -and ($agreed.Version -eq 'v3.0.8')) 'derives v3.0.8 from an agreeing set'
        $wrongRequest = Resolve-PublishVersion 'v3.0' '3.0.8' '3.0.8' '3.0.8'
        Assert-Selftest ([bool]$wrongRequest.Error) 'refuses -Version v3.0 against pubspec 3.0.8'
        $wrongCargo = Resolve-PublishVersion '' '3.0.8' '0.1.0' '3.0.8'
        Assert-Selftest ([bool]$wrongCargo.Error) 'refuses a Cargo version that disagrees with pubspec'
        $wrongDart = Resolve-PublishVersion '' '3.0.8' '3.0.8' '0.1.0'
        Assert-Selftest ([bool]$wrongDart.Error) 'refuses a stale generated version.dart'
        $checkoutVersion = Get-PublishVersion ''
        Assert-Selftest (-not $checkoutVersion.Error) 'this checkout has one product version (pubspec, Cargo.toml, version.dart)'
        if ($checkoutVersion.Error) { Write-Host "  version: $($checkoutVersion.Error)" }

        Write-Host ''
        Write-Host 'Third-party notices generator (SHELL-F11):'
        & (Join-Path $V3 'tool\gen-third-party-notices.ps1') -SelfTest
        Assert-Selftest ($LASTEXITCODE -eq 0) 'notices generator selftest passes'

        Write-Host ''
        Write-Host 'Release gates (SHELL-F9):'
        Assert-Selftest ($null -eq (Test-DirtyTreeAllowed '' $false)) 'accepts a clean worktree'
        Assert-Selftest ([bool](Test-DirtyTreeAllowed " M v3/publish.ps1" $false)) 'refuses a dirty worktree'
        Assert-Selftest ([bool](Test-DirtyTreeAllowed "?? untracked.txt" $false)) 'counts an untracked file as dirty'
        Assert-Selftest ($null -eq (Test-DirtyTreeAllowed " M v3/publish.ps1" $true)) 'allows -AllowDirty'
        $buildStage = Join-Path $temp 'build-info-stage'
        New-Item -ItemType Directory -Path $buildStage -Force | Out-Null
        Set-Content -LiteralPath (Join-Path $buildStage 'b.txt') -Value 'bee' -Encoding ASCII
        Set-Content -LiteralPath (Join-Path $buildStage 'a.txt') -Value 'aye' -Encoding ASCII
        $manifest = @(Get-PayloadManifest $buildStage 'build-info.json')
        Assert-Selftest ($manifest.Count -eq 2) 'payload manifest lists the staged files'
        Assert-Selftest (($manifest[0].path -eq 'a.txt') -and ($manifest[1].path -eq 'b.txt')) 'payload manifest sorts paths deterministically'
        Assert-Selftest ([bool]($manifest[0].sha256 -match '^[0-9a-f]{64}$')) 'payload manifest hashes each file'
        $toolchain = @{ rustc = 'rustc 1.99.1'; cargo = 'cargo 1.99.1'; flutter = 'Flutter 3.47.8'; dart = 'Dart 3.13.7' }
        $buildInfo = New-BuildInfo -ProductVersion '3.0.8' -Release 'v3.0.8' -Commit 'deadbeef' -Branch 'main' -Dirty $false -BuiltAtUtc '2026-01-01T00:00:00Z' -Toolchain $toolchain -Payload $manifest
        $parsed = $buildInfo | ConvertFrom-Json
        Assert-Selftest (($parsed.product_version -eq '3.0.8') -and ($parsed.release -eq 'v3.0.8')) 'build-info records the product and release names'
        Assert-Selftest (($parsed.git_commit -eq 'deadbeef') -and ($parsed.toolchain.rustc -eq 'rustc 1.99.1')) 'build-info records the commit and toolchains'
        Assert-Selftest (@($parsed.payload).Count -eq 2) 'build-info carries the payload list'
        $sameInfo = New-BuildInfo -ProductVersion '3.0.8' -Release 'v3.0.8' -Commit 'deadbeef' -Branch 'main' -Dirty $false -BuiltAtUtc '2026-01-01T00:00:00Z' -Toolchain $toolchain -Payload $manifest
        Assert-Selftest ($buildInfo -eq $sameInfo) 'build-info is deterministic for fixed inputs'
        $zipA = Join-Path $temp 'SealTools-v3.0.8.zip'
        $zipB = Join-Path $temp 'SealTools-v3.0.8-firmware.zip'
        Set-Content -LiteralPath $zipA -Value 'app payload' -Encoding ASCII
        Set-Content -LiteralPath $zipB -Value 'sketch' -Encoding ASCII
        $sums = Join-Path $temp 'SHA256SUMS.txt'
        Write-ChecksumFile $sums @($zipA, $zipB)
        Assert-Selftest (Test-Path -LiteralPath $sums) 'writes dist\SHA256SUMS.txt'
        $sumLines = @(Get-Content -LiteralPath $sums)
        Assert-Selftest ($sumLines.Count -eq 2) 'checksum file has one line per asset'
        Assert-Selftest ($sumLines[0] -match '^[0-9a-f]{64}  SealTools-v3\.0\.8\.zip$') 'checksum file uses sha256sum naming and lowercase hex'
        Assert-Selftest ($sumLines[0].Split(' ')[0] -eq (Get-FileHash -LiteralPath $zipA -Algorithm SHA256).Hash.ToLowerInvariant()) 'checksum matches the asset bytes'

        Write-Host ''
        Write-Host 'Forbidden files (global deny):'
        $stage = Join-Path $temp 'stage'
        $forbidden = @(
            'data/flutter_assets/assets/config/local.yaml',
            'data/local.yaml',
            'data/logs/tool.log',
            'data/calib_gem_result.png',
            'data/.env',
            'config/setup/common.yaml',
            'config/machine/pet.yaml',
            'config/artifacts/pet_close_button.png',
            'config/user.yaml',
            'config/meta.yaml',
            'config/.migration-backup-deadbeef/local.yaml',
            'config/.seal-txn/txn-1/journal.json'
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
        $requiredFiles = @('SealTools.exe', 'seal-core.exe', 'DirectML.dll', 'build-info.json')
        foreach ($runtime in $FlutterRuntimeFiles) { $requiredFiles += $runtime.Replace('\', '/') }
        $requiredFiles += @(
            'models/ch_PP-OCRv4_det_infer.onnx',
            'models/ch_PP-OCRv4_rec_infer.onnx',
            'models/ch_ppocr_mobile_v2.0_cls_infer.onnx',
            'models/PP-OCRv6_small_det_infer.onnx',
            'models/PP-OCRv6_small_rec_infer.onnx',
            'models/PP-OCRv6_small_rec_dict.txt',
            'config/README.md',
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
    Write-Host 'Selftest passed: version agreement, dirty-tree/gates, payload manifest, checksums, denial rules, hidden/reparse, post-zip and firmware guards.'
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

if ($Selftest) {
    Invoke-Selftest
}

# SHELL-F10: the release name comes from pubspec, not from a second hand-edited default.
$resolvedVersion = Get-PublishVersion $Version
if ($resolvedVersion.Error) { Fail $resolvedVersion.Error }
$Version = $resolvedVersion.Version
$ProductVersion = $resolvedVersion.Product
Write-Host "product version: $ProductVersion (pubspec.yaml; release name $Version)"

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
    Write-Host '  build-info.json (commit, toolchain versions, payload SHA-256 list)'
    Write-Host '  seal-core.exe, DirectML.dll'
    Write-Host '  models/<the three PP-OCRv4 .onnx files and the three PP-OCRv6 small files>'
    Write-Host '  config/defaults.yaml.example, local.yaml.example, attributes.yaml, attributes.us.yaml.example'
    Write-Host '  docs/pet-data.csv, USER_GUIDE.md, USER_GUIDE.zh-TW.md, INSTALL.md, THIRD-PARTY-NOTICES.md'
    Write-Host "Installer -> $($releasePaths.InstallerExe) (per-user NSIS)"
    Write-Host "Firmware -> $FirmwareZip (seal_mouse/ + seal_mouse.ino)"
    Write-Host "App zip  -> $AppZip (portable self-install)"
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
    Write-Hash $installerExe
    Write-Hash $FirmwareZip
    $checksumPath = Join-Path $DistDir 'SHA256SUMS.txt'
    Write-ChecksumFile $checksumPath @($AppZip, $installerExe, $FirmwareZip)
    Write-Host ''
    Write-Host "Checksums written to $checksumPath (sha256sum format, lowercase, one line per asset)."
    Get-Content -LiteralPath $checksumPath | ForEach-Object { Write-Host "  $_" }
    Write-SigningTodo
    exit 0
}

Invoke-Stage
Write-Host 'Staged only (-Stage): nothing was zipped. Use -Release (with -AcceptDirectMLRedistribution) to package.'
exit 0
