[CmdletBinding()]
param(
    [string]$Version,
    [string]$InstallDir,
    [string]$ArchivePath,
    [string]$ChecksumsPath
)

$ErrorActionPreference = 'Stop'
$Repository = 'zerodha/kite-cli'
$Target = 'x86_64-pc-windows-msvc'
$TemporaryDirectory = Join-Path ([IO.Path]::GetTempPath()) ("kite-install-" + [guid]::NewGuid())
$OwnedTemporaryDirectory = $false

function Get-ReleaseTag([string]$RequestedVersion, [object[]]$Releases) {
    if ($RequestedVersion) {
        $requestedTag = if ($RequestedVersion.StartsWith('v')) { $RequestedVersion } else { "v$RequestedVersion" }
        $release = $Releases | Where-Object { $_.tagName -eq $requestedTag -or $_.tag_name -eq $requestedTag } | Select-Object -First 1
        if (-not $release) { throw "Release $requestedTag was not found." }
        return $requestedTag
    }

    $visibleReleases = @($Releases | Where-Object { -not $_.isDraft -and -not $_.draft })
    $release = $visibleReleases | Where-Object { -not $_.isPrerelease -and -not $_.prerelease } | Select-Object -First 1
    if (-not $release) {
        $release = $visibleReleases | Where-Object { $_.isPrerelease -or $_.prerelease } | Select-Object -First 1
    }
    if (-not $release) { throw 'No published stable release or prerelease was found.' }
    if ($release.tagName) { return [string]$release.tagName }
    return [string]$release.tag_name
}

function Download-ReleaseFiles([string]$Tag, [string]$ArchiveName, [string]$Destination) {
    $gh = Get-Command gh -ErrorAction SilentlyContinue
    if ($gh) {
        & $gh.Source auth status *> $null
        if ($LASTEXITCODE -eq 0) {
            & $gh.Source release download $Tag --repo $Repository --pattern $ArchiveName --pattern checksums.txt --dir $Destination *> $null
            if ($LASTEXITCODE -ne 0) {
                throw "Could not download the Windows archive for $Tag. This release may not include Windows yet."
            }
            return
        }
    }

    $headers = @{ 'User-Agent' = 'kite-cli-installer' }
    $token = if ($env:GH_TOKEN) { $env:GH_TOKEN } else { $env:GITHUB_TOKEN }
    if ($token) { $headers['Authorization'] = "Bearer $token" }
    try {
        $releases = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repository/releases?per_page=100" -Headers $headers
    }
    catch {
        throw 'Could not read GitHub releases. For a private repository, install GitHub CLI and run gh auth login first.'
    }
    $release = $releases | Where-Object { $_.tag_name -eq $Tag -and -not $_.draft } | Select-Object -First 1
    if (-not $release) { throw "Published release $Tag was not found." }
    foreach ($name in @($ArchiveName, 'checksums.txt')) {
        $asset = $release.assets | Where-Object { $_.name -eq $name } | Select-Object -First 1
        if (-not $asset) {
            throw "Release $Tag does not include $name."
        }
        Invoke-WebRequest -Uri $asset.browser_download_url -Headers $headers -OutFile (Join-Path $Destination $name)
    }
}

try {
    if (-not $InstallDir) {
        if (-not $env:LOCALAPPDATA) { throw 'LOCALAPPDATA is unavailable. Set -InstallDir to a user-writable directory.' }
        $InstallDir = Join-Path $env:LOCALAPPDATA 'Programs\kite'
    }
    $InstallDir = [IO.Path]::GetFullPath($InstallDir)

    if ($ArchivePath) {
        $ArchivePath = (Resolve-Path -LiteralPath $ArchivePath).Path
        $archiveName = [IO.Path]::GetFileName($ArchivePath)
        $match = [regex]::Match($archiveName, '^kite-(?<version>[0-9A-Za-z.+-]+)-x86_64-pc-windows-msvc\.zip$')
        if (-not $match.Success) { throw 'Archive name does not match the Windows x86_64 release format.' }
        $version = $match.Groups['version'].Value
        if (-not $ChecksumsPath) { $ChecksumsPath = Join-Path (Split-Path -Parent $ArchivePath) 'checksums.txt' }
        $ChecksumsPath = (Resolve-Path -LiteralPath $ChecksumsPath).Path
    }
    else {
        New-Item -ItemType Directory -Path $TemporaryDirectory | Out-Null
        $OwnedTemporaryDirectory = $true
        $gh = Get-Command gh -ErrorAction SilentlyContinue
        if ($gh) {
            & $gh.Source auth status *> $null
            if ($LASTEXITCODE -eq 0) {
                $releaseJson = & $gh.Source release list --repo $Repository --limit 100 --json tagName,isPrerelease,isDraft
                if ($LASTEXITCODE -ne 0) { throw 'Could not list GitHub releases.' }
                $releases = @($releaseJson | ConvertFrom-Json)
            }
        }
        if (-not $releases) {
            $headers = @{ 'User-Agent' = 'kite-cli-installer' }
            $token = if ($env:GH_TOKEN) { $env:GH_TOKEN } else { $env:GITHUB_TOKEN }
            if ($token) { $headers['Authorization'] = "Bearer $token" }
            try {
                $releases = @(Invoke-RestMethod -Uri "https://api.github.com/repos/$Repository/releases?per_page=100" -Headers $headers)
            }
            catch {
                throw 'Could not read GitHub releases. For a private repository, install GitHub CLI and run gh auth login first.'
            }
        }
        $tag = Get-ReleaseTag $Version $releases
        if ($tag -notmatch '^v[0-9]+\.[0-9]+\.[0-9]+(?:[-+][0-9A-Za-z.-]+)?$') { throw "Invalid release tag: $tag" }
        $version = $tag.Substring(1)
        $archiveName = "kite-$version-$Target.zip"
        Download-ReleaseFiles $tag $archiveName $TemporaryDirectory
        $ArchivePath = Join-Path $TemporaryDirectory $archiveName
        $ChecksumsPath = Join-Path $TemporaryDirectory 'checksums.txt'
    }

    $checksumLine = Get-Content -LiteralPath $ChecksumsPath | Where-Object { $_ -match '^([0-9a-fA-F]{64})\s+\*?(.+?)\s*$' -and $Matches[2] -eq $archiveName } | Select-Object -First 1
    if (-not $checksumLine) { throw "checksums.txt has no SHA-256 entry for $archiveName." }
    $null = $checksumLine -match '^([0-9a-fA-F]{64})\s+\*?(.+?)\s*$'
    $expected = $Matches[1].ToLowerInvariant()
    $actual = (Get-FileHash -LiteralPath $ArchivePath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $expected) { throw "SHA-256 check failed for $archiveName." }

    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $archive = [IO.Compression.ZipFile]::OpenRead($ArchivePath)
    try {
        $entry = $archive.Entries | Where-Object { $_.FullName -ceq 'kite.exe' } | Select-Object -First 1
        if (-not $entry) { throw 'Windows archive does not contain kite.exe at its root.' }
        $extractedBinary = Join-Path $TemporaryDirectory 'kite.exe'
        if (-not $OwnedTemporaryDirectory) {
            New-Item -ItemType Directory -Path $TemporaryDirectory | Out-Null
            $OwnedTemporaryDirectory = $true
        }
        $source = $entry.Open()
        try {
            $destination = [IO.File]::Create($extractedBinary)
            try { $source.CopyTo($destination) } finally { $destination.Dispose() }
        }
        finally { $source.Dispose() }
    }
    finally { $archive.Dispose() }

    $versionOutput = & $extractedBinary --version 2>&1
    $reportedVersion = [regex]::Match(($versionOutput -join "`n"), '\b[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?\b').Value
    if ($LASTEXITCODE -ne 0 -or -not $reportedVersion -or ($version -ne $reportedVersion -and $version -ne "$reportedVersion-preview")) {
        throw 'Windows binary version check failed.'
    }
    & $extractedBinary --help *> $null
    if ($LASTEXITCODE -ne 0) { throw 'Windows binary help check failed.' }

    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    $targetPath = Join-Path $InstallDir 'kite.exe'
    if (Test-Path -LiteralPath $targetPath) {
        $existing = Get-Item -LiteralPath $targetPath -Force
        if ($existing.Attributes -band [IO.FileAttributes]::ReparsePoint) {
            throw "Refusing to replace a reparse point: $targetPath"
        }
    }
    $stagedPath = Join-Path $InstallDir ('.kite-install-' + [guid]::NewGuid() + '.exe')
    Copy-Item -LiteralPath $extractedBinary -Destination $stagedPath
    Move-Item -LiteralPath $stagedPath -Destination $targetPath -Force

    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $pathEntries = @($userPath -split ';' | Where-Object { $_ })
    $alreadyOnPath = $pathEntries | Where-Object { [IO.Path]::GetFullPath($_).TrimEnd('\') -ieq $InstallDir.TrimEnd('\') }
    if (-not $alreadyOnPath) {
        $updatedPath = (@($pathEntries) + $InstallDir) -join ';'
        [Environment]::SetEnvironmentVariable('Path', $updatedPath, 'User')
        $env:Path = "$env:Path;$InstallDir"
    }

    Write-Output "Installed kite $reportedVersion to $targetPath"
    Write-Output "Verified SHA-256 $actual"
    if (-not $alreadyOnPath) { Write-Output 'Added the install directory to the user PATH.' }
}
catch {
    [Console]::Error.WriteLine("kite install: $($_.Exception.Message)")
    exit 1
}
finally {
    if ($OwnedTemporaryDirectory -and (Test-Path -LiteralPath $TemporaryDirectory)) {
        Remove-Item -LiteralPath $TemporaryDirectory -Recurse -Force
    }
}
