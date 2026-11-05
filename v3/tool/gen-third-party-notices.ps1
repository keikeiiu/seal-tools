# Generates the Rust-crate attribution section of v3\docs\THIRD-PARTY-NOTICES.md (SHELL-F11).
#
# MIT and Apache-2.0 require the copyright notice and licence text to travel with a binary
# distribution. The release ships seal-core.exe, which statically links the crates in
# v3\core\Cargo.lock, so the notices must name every bundled crate and carry its own licence text.
#
# The crate list comes from `cargo tree -p seal-core --edges normal` (target-scoped: only crates
# actually linked for this Windows build), and each text is read from the crate's own package in the
# cargo registry cache. No network access and no new dependency: this is the installed toolchain plus
# the sources a build already needs.
#
#   pwsh -File v3\tool\gen-third-party-notices.ps1            regenerate the section
#   pwsh -File v3\tool\gen-third-party-notices.ps1 -Check     fail when the committed file is stale
#   pwsh -File v3\tool\gen-third-party-notices.ps1 -SelfTest  exercise rendering without cargo
#
# publish.ps1 runs -Check before staging a release, so a lockfile change cannot ship with stale
# notices. The generated block sits between the two marker comments below; everything outside them is
# hand-written and left alone.
[CmdletBinding()]
param(
    [switch]$Check,
    [switch]$SelfTest,
    [string]$NoticesPath = '',
    [string]$CoreDir = ''
)

$ErrorActionPreference = 'Stop'

$V3 = Split-Path -Parent $PSScriptRoot
if ($CoreDir -eq '') { $CoreDir = Join-Path $V3 'core' }
if ($NoticesPath -eq '') { $NoticesPath = Join-Path $V3 'docs\THIRD-PARTY-NOTICES.md' }
$CargoWrapper = Join-Path $CoreDir 'cargo.cmd'

$BeginMarker = '<!-- BEGIN GENERATED RUST CRATES: v3\tool\gen-third-party-notices.ps1 -->'
$EndMarker = '<!-- END GENERATED RUST CRATES -->'

function Stop-Generator([string]$message) {
    [Console]::Error.WriteLine("[!] $message")
    exit 1
}

function Read-Utf8([string]$path) {
    $text = [IO.File]::ReadAllText($path, [Text.Encoding]::UTF8)
    if ($text.Length -gt 0 -and $text[0] -eq [char]0xFEFF) { $text = $text.Substring(1) }
    return $text.Replace("`r`n", "`n").Replace("`r", "`n")
}

function Get-Sha256Hex([string]$text) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        $hash = $sha.ComputeHash([Text.Encoding]::UTF8.GetBytes($text))
    } finally {
        $sha.Dispose()
    }
    return [BitConverter]::ToString($hash).Replace('-', '')
}

function Invoke-Cargo([string[]]$arguments) {
    if (Test-Path -LiteralPath $CargoWrapper) {
        return & $CargoWrapper @arguments
    }
    return & cargo @arguments
}

function Get-BundledPackages([string]$coreDir) {
    Push-Location $coreDir
    try {
        $metadataText = (Invoke-Cargo @('metadata', '--locked', '--offline', '--format-version', '1')) -join "`n"
        if ($LASTEXITCODE -ne 0 -or $metadataText.Trim() -eq '') {
            Stop-Generator 'cargo metadata failed; the crate set cannot be audited (build once, then rerun).'
        }
        $treeLines = @(Invoke-Cargo @('tree', '-p', 'seal-core', '--edges', 'normal', '--prefix', 'none', '--locked', '--offline'))
        if ($LASTEXITCODE -ne 0) {
            Stop-Generator 'cargo tree failed; the bundled crate set cannot be audited.'
        }
    } finally {
        Pop-Location
    }

    $metadata = $metadataText | ConvertFrom-Json
    $byKey = @{}
    foreach ($package in $metadata.packages) {
        if ($null -eq $package.source) { continue } # first-party seal-* crates
        $byKey["$($package.name) $($package.version)"] = $package
    }

    $keys = New-Object System.Collections.Generic.List[string]
    foreach ($line in $treeLines) {
        $text = [string]$line
        # cargo tree prints a path in parentheses for the workspace's own seal-* crates. They are
        # first-party code and are not attributed here; a registry crate never has a path.
        if ($text -match '\([^)]*[\\/][^)]*\)') { continue }
        $match = [regex]::Match($text, '^(\S+) v(\S+)')
        if (-not $match.Success) { continue }
        $key = "$($match.Groups[1].Value) $($match.Groups[2].Value)"
        if (-not $keys.Contains($key)) { $keys.Add($key) }
    }
    $ordered = $keys.ToArray()
    [Array]::Sort($ordered, [StringComparer]::Ordinal)

    $packages = New-Object System.Collections.Generic.List[object]
    foreach ($key in $ordered) {
        if (-not $byKey.ContainsKey($key)) {
            Stop-Generator "cargo tree names '$key' but cargo metadata does not; refusing to guess its licence."
        }
        $package = $byKey[$key]
        $directory = Split-Path -Parent $package.manifest_path
        if (-not (Test-Path -LiteralPath $directory)) {
            Stop-Generator "crate source missing for '$key': $directory"
        }
        $names = New-Object System.Collections.Generic.List[string]
        foreach ($file in @(Get-ChildItem -LiteralPath $directory -File -Force)) {
            if ($file.Name -match '^(LICEN[SC]E|COPYING|COPYRIGHT|NOTICE|UNLICENSE)') { $names.Add($file.Name) }
        }
        $licenseFile = [string]$package.license_file
        if ($licenseFile -ne '' -and -not $names.Contains($licenseFile) -and (Test-Path -LiteralPath (Join-Path $directory $licenseFile))) {
            $names.Add($licenseFile)
        }
        $fileNames = $names.ToArray()
        [Array]::Sort($fileNames, [StringComparer]::Ordinal)

        $files = New-Object System.Collections.Generic.List[object]
        foreach ($name in $fileNames) {
            $text = (Read-Utf8 (Join-Path $directory $name)).Trim()
            if ($text -eq '') { continue }
            if ($text.Contains('~~~~')) {
                Stop-Generator "licence text in '$key/$name' contains the Markdown fence; extend the renderer."
            }
            $files.Add([pscustomobject]@{ Name = $name; Text = $text })
        }
        if ($files.Count -eq 0) {
            Stop-Generator "no licence text in '$key' (declared '$($package.license)'); a release cannot attribute it."
        }
        $packages.Add([pscustomobject]@{
            Name = $package.name
            Version = $package.version
            License = [string]$package.license
            Files = $files.ToArray()
            TextIds = @()
        })
    }
    return $packages.ToArray()
}

function Group-LicenseTexts([object[]]$packages) {
    $byHash = @{}
    $texts = New-Object System.Collections.Generic.List[object]
    foreach ($package in $packages) {
        $ids = New-Object System.Collections.Generic.List[string]
        foreach ($file in $package.Files) {
            $hash = Get-Sha256Hex $file.Text
            if (-not $byHash.ContainsKey($hash)) {
                $id = 'T{0:D3}' -f ($texts.Count + 1)
                $byHash[$hash] = $id
                $texts.Add([pscustomobject]@{ Id = $id; Text = $file.Text; Crates = (New-Object System.Collections.Generic.List[string]) })
            }
            $id = $byHash[$hash]
            if (-not $ids.Contains($id)) { $ids.Add($id) }
            $entry = $texts | Where-Object { $_.Id -eq $id }
            $label = "$($package.Name) $($package.Version)"
            if (-not $entry.Crates.Contains($label)) { $entry.Crates.Add($label) }
        }
        $package.TextIds = $ids.ToArray()
    }
    return $texts.ToArray()
}

function New-NoticesBlock([object[]]$packages) {
    $texts = Group-LicenseTexts $packages
    $lines = New-Object System.Collections.Generic.List[string]
    $lines.Add('## Rust crates')
    $lines.Add('')
    $lines.Add('The core is Rust. `seal-core.exe` statically links the crates below; each keeps its own licence,')
    $lines.Add('and this section carries the copyright notice and licence text that MIT and Apache-2.0 require')
    $lines.Add('with a binary distribution. The crate list comes from `cargo tree -p seal-core --edges normal`')
    $lines.Add('(the crates linked for the Windows build), not from every dev or other-platform lock entry.')
    $lines.Add('First-party `seal-*` crates are Seal Tools'' own code and are not listed.')
    $lines.Add('')
    $lines.Add('This section is generated by `v3\tool\gen-third-party-notices.ps1`; do not edit it by hand.')
    $lines.Add('Regenerate it after a lockfile change; `publish.ps1` refuses to stage a stale copy.')
    $lines.Add('')
    $lines.Add('| Crate | Version | Licence |')
    $lines.Add('| --- | --- | --- |')
    foreach ($package in $packages) {
        $license = $package.License
        if ($license -eq '') { $license = 'unknown' }
        $license = $license.Replace('|', '\|')
        $lines.Add("| $($package.Name) | $($package.Version) | $license |")
    }
    $lines.Add('')
    $lines.Add('### Licence texts')
    $lines.Add('')
    foreach ($text in $texts) {
        $lines.Add("<a id=""rust-licence-$($text.Id)""></a>")
        $lines.Add('')
        $lines.Add("#### $($text.Id)")
        $lines.Add('')
        $applies = New-Object System.Collections.Generic.List[string]
        foreach ($crate in $text.Crates) { $applies.Add("``$crate``") }
        $lines.Add("Applies to: $($applies -join ', ').")
        $lines.Add('')
        $lines.Add('~~~~text')
        foreach ($textLine in $text.Text.Split("`n")) { $lines.Add($textLine.TrimEnd()) }
        $lines.Add('~~~~')
        $lines.Add('')
    }
    return ($lines -join "`n").TrimEnd() + "`n"
}

function Test-NoticesContent([string]$content, [string]$block) {
    $match = [regex]::Match($content, '(?s)' + [regex]::Escape($BeginMarker) + '(.*?)' + [regex]::Escape($EndMarker))
    if (-not $match.Success) {
        return "the generated markers are missing from $NoticesPath"
    }
    $actual = $match.Groups[1].Value.Trim("`n")
    if ($actual -ne $block.Trim("`n")) {
        return "the Rust-crate section in $NoticesPath is stale; run v3\tool\gen-third-party-notices.ps1"
    }
    return $null
}

function Write-Notices([string]$path, [string]$block) {
    $content = Read-Utf8 $path
    $matchExists = [regex]::IsMatch($content, '(?s)' + [regex]::Escape($BeginMarker) + '(.*?)' + [regex]::Escape($EndMarker))
    if (-not $matchExists) { Stop-Generator "the generated markers are missing from $path" }
    $replacement = $BeginMarker + "`n" + $block.TrimEnd() + "`n" + $EndMarker
    $pattern = [regex]::new('(?s)' + [regex]::Escape($BeginMarker) + '.*?' + [regex]::Escape($EndMarker))
    # MatchEvaluator, not a replacement string: licence text contains '$', which a replacement string
    # would treat as a group substitution.
    $updated = $pattern.Replace($content, [System.Text.RegularExpressions.MatchEvaluator] { param($match) $replacement })
    [IO.File]::WriteAllText($path, $updated, (New-Object Text.UTF8Encoding $false))
}

function Invoke-SelfTest {
    $script:SelfTestFailures = 0
    function Check([bool]$condition, [string]$label) {
        if ($condition) {
            Write-Host "[ok] $label"
        } else {
            Write-Host "[FAIL] $label"
            $script:SelfTestFailures++
        }
    }
    $first = [pscustomobject]@{
        Name = 'alpha'
        Version = '1.2.3'
        License = 'MIT OR Apache-2.0'
        Files = @(
            [pscustomobject]@{ Name = 'LICENSE-MIT'; Text = "MIT License`nCopyright (c) Alpha" },
            [pscustomobject]@{ Name = 'LICENSE-APACHE'; Text = "Apache License 2.0 text" }
        )
        TextIds = @()
    }
    $second = [pscustomobject]@{
        Name = 'beta'
        Version = '0.1.0'
        License = 'MIT'
        Files = @([pscustomobject]@{ Name = 'LICENSE'; Text = "Apache License 2.0 text" })
        TextIds = @()
    }
    $packages = @($first, $second)
    $block = New-NoticesBlock $packages
    Check ($block.Contains('| alpha | 1.2.3 | MIT OR Apache-2.0 |')) 'renders the crate index'
    Check ($block.Contains('| beta | 0.1.0 | MIT |')) 'renders a second crate'
    Check ($block.Contains('#### T001') -and -not $block.Contains('#### T003')) 'deduplicates identical licence texts'
    Check ($block.Contains('Applies to: `alpha 1.2.3`, `beta 0.1.0`.') -and $block.Contains('Applies to: `alpha 1.2.3`.')) 'maps each text to every crate that uses it'
    Check ((New-NoticesBlock $packages) -eq $block) 'renders deterministically'
    $temp = Join-Path ([IO.Path]::GetTempPath()) ('seal-notices-' + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $temp -Force | Out-Null
    try {
        $path = Join-Path $temp 'NOTICES.md'
        $template = "# Notices`n`n$BeginMarker`nold`n$EndMarker`n`n## Fonts`n"
        [IO.File]::WriteAllText($path, $template, (New-Object Text.UTF8Encoding $false))
        $stale = Test-NoticesContent (Read-Utf8 $path) $block
        Check ([bool]$stale) 'detects a stale section'
        Write-Notices $path $block
        $current = Test-NoticesContent (Read-Utf8 $path) $block
        Check ($null -eq $current) 'accepts the regenerated section'
        Check ((Read-Utf8 $path).Contains('## Fonts')) 'leaves the hand-written sections alone'
    } finally {
        Remove-Item -LiteralPath $temp -Recurse -Force -ErrorAction SilentlyContinue
    }
    if ($script:SelfTestFailures -gt 0) {
        Write-Host "[!] Generator selftest failed: $($script:SelfTestFailures) check(s)."
        exit 1
    }
    Write-Host 'Generator selftest passed.'
    exit 0
}

if ($SelfTest) { Invoke-SelfTest }

if (-not (Test-Path -LiteralPath $NoticesPath)) {
    Stop-Generator "notices file missing: $NoticesPath"
}
$packages = Get-BundledPackages $CoreDir
$block = New-NoticesBlock $packages
if ($Check) {
    $problem = Test-NoticesContent (Read-Utf8 $NoticesPath) $block
    if ($problem) { Stop-Generator $problem }
    Write-Host "Rust-crate notices are current ($($packages.Count) bundled crates)."
    exit 0
}
Write-Notices $NoticesPath $block
Write-Host "Wrote the Rust-crate section for $($packages.Count) bundled crates to $NoticesPath."
exit 0
