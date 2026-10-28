<#
.SYNOPSIS
Builds Agency Agents' Windows installers (NSIS, x64 and arm64) and signs them with Azure Artifact Signing.

.DESCRIPTION
Run it INSIDE Windows (the "Windows 11" Parallels VM), from a local copy of the repository, with Windows
PowerShell 5.1. The VM shares only ~/vm-exchange with the Mac (\\Mac\Exchange), never the home folder:

    (on the Mac)
    rsync -a --delete --exclude .git --exclude node_modules --exclude target --exclude .svelte-kit \
      --exclude build --exclude dist --exclude .phase-c --exclude '.env*' ./ ~/vm-exchange/agency-agents-app/src/

    (in the VM)
    robocopy \\Mac\Exchange\agency-agents-app\src C:\Users\michael\dev\agency-agents-app /MIR /XD node_modules target .svelte-kit build dist
    cd C:\Users\michael\dev\agency-agents-app
    az login
    powershell -ExecutionPolicy Bypass -File scripts\release-windows.ps1 -CopyTo \\Mac\Exchange\agency-agents-app\out

The installers land in dist\windows\ (and in -CopyTo), named like the release assets:
Agency_Agents_<version>_x64-setup.exe and Agency_Agents_<version>_arm64-setup.exe, with SHA256SUMS.txt.

Signing. Tauri signs every Windows binary it builds when bundle.windows.signCommand is set, which this script does
with a --config file (tauri.conf.json is never edited): the app's executable, the NSIS plugin DLLs the installer
extracts while it runs, the uninstaller, and the installer itself. Smart App Control checks each binary that loads,
so all of them need a signature that chains to a trusted root, which Artifact Signing's does.

    AGENCY_SIGN_ENDPOINT   The Artifact Signing account's regional endpoint. Default: https://eus.codesigning.azure.net/
    AGENCY_SIGN_ACCOUNT    The Artifact Signing account. Default: msitarzewski-signing
    AGENCY_SIGN_PROFILE    The certificate profile (Public Trust). Default: autopaper (the account's one profile;
                           the certificate names the person, not the app)
    AGENCY_SIGN_SUBJECT    The signer every signature must carry. Default:
                           CN=Michael Sitarzewski, O=Michael Sitarzewski, L=Dallas, S=tx, C=US

Authentication is the Artifact Signing dlib's DefaultAzureCredential, limited to two sources: the Azure CLI's login
(az login) or a service principal's environment variables (AZURE_TENANT_ID, AZURE_CLIENT_ID, AZURE_CLIENT_SECRET).
The identity needs the "Artifact Signing Certificate Profile Signer" role. Secrets are never printed or written.

SignTool and the dlib come from NuGet, pinned and checked by SHA-256, into dist\windows-tools. The dlib is x64 only,
so x64 SignTool runs it (under emulation on ARM64 Windows) and needs the x64 .NET runtime 8 or later. Signatures are
timestamped: Artifact Signing certificates are valid for 72 hours, and the timestamp keeps a signature valid after.

-TestCert signs with a self-signed certificate in the current user's store instead (made on first use), to test the
build and the signing steps without Azure. Windows doesn't trust it: those installers are for testing only.

Updater artifacts stay off on Windows (bundle.createUpdaterArtifacts false), as in the CI build.

.PARAMETER TestCert
Sign with a self-signed test certificate, not Artifact Signing.

.PARAMETER Arch
Which installers to build: x64, arm64 or all (default).

.PARAMETER CopyTo
A folder to copy the installers and SHA256SUMS.txt into after a successful run, e.g. \\Mac\Exchange\agency-agents-app\out.

.PARAMETER SkipNpmCi
Use the node_modules already there instead of running npm ci.
#>
[CmdletBinding()]
param(
    [switch]$TestCert,
    [ValidateSet('x64', 'arm64', 'all')]
    [string]$Arch = 'all',
    [string]$CopyTo = '',
    [switch]$SkipNpmCi
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
Add-Type -AssemblyName System.IO.Compression.FileSystem

$Repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).ProviderPath
$Out = Join-Path $Repo 'dist\windows'
$Stage = Join-Path $Repo 'dist\windows-stage'
$Tools = Join-Path $Repo 'dist\windows-tools'
$Version = (Get-Content (Join-Path $Repo 'src-tauri\tauri.conf.json') -Raw | ConvertFrom-Json).version
$TimestampUrl = 'http://timestamp.acs.microsoft.com'
$TestSubject = 'CN=Agency Agents Test Signing'

$Targets = [ordered]@{ x64 = 'x86_64-pc-windows-msvc'; arm64 = 'aarch64-pc-windows-msvc' }
if ($Arch -ne 'all') { $Targets = [ordered]@{ $Arch = $Targets[$Arch] } }

# Pinned tools (NuGet), checked by the SHA-256 of their .nupkg (the same pins as AutoPaper's release script).
$SdkBuildTools = @{ Id = 'Microsoft.Windows.SDK.BuildTools'; Version = '10.0.28000.2705'; Sha256 = '8BFDFB6CA2633F531CF80B5FA22512BA61A394D7988F0970DB83BAADC67929ED' }
$SigningClient = @{ Id = 'Microsoft.ArtifactSigning.Client'; Version = '1.0.128'; Sha256 = '74BD7D27E6CE1051409C38D9B46BC8DF0400ECD643D51FFBF2AC00869061E40B' }

function Write-Step([string]$Message) {
    Write-Host ''
    Write-Host "==> $Message"
}

# Native commands run without 2>&1 (Windows PowerShell 5.1 turns redirected stderr into terminating errors under
# ErrorActionPreference Stop); a non-zero exit code is an error.
function Invoke-Native([string]$Exe, [string[]]$Arguments) {
    & $Exe @Arguments | Out-Host
    if ($LASTEXITCODE -ne 0) {
        throw "$Exe $($Arguments -join ' ') failed (exit code $LASTEXITCODE)"
    }
}

function Write-Utf8([string]$Path, [string]$Text) {
    New-Item -ItemType Directory -Force (Split-Path $Path) | Out-Null
    [System.IO.File]::WriteAllText($Path, $Text, (New-Object System.Text.UTF8Encoding $false))
}

# A NuGet package's contents, downloaded once into dist\windows-tools and checked against its pinned SHA-256.
function Get-NuGetTool([hashtable]$Package) {
    $folder = Join-Path $Tools "$($Package.Id).$($Package.Version)"
    $marker = Join-Path $folder '.verified'
    if (Test-Path $marker) { return $folder }
    New-Item -ItemType Directory -Force $Tools | Out-Null
    $id = $Package.Id.ToLowerInvariant()
    $nupkg = Join-Path $Tools "$id.$($Package.Version).zip"
    $url = "https://api.nuget.org/v3-flatcontainer/$id/$($Package.Version)/$id.$($Package.Version).nupkg"
    Write-Host "Downloading $($Package.Id) $($Package.Version)"
    Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $nupkg
    $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $nupkg).Hash.ToUpperInvariant()
    if ($hash -ne $Package.Sha256) {
        Remove-Item $nupkg -Force
        throw "$($Package.Id) $($Package.Version) has SHA-256 $hash, expected $($Package.Sha256): not using it."
    }
    if (Test-Path $folder) { Remove-Item $folder -Recurse -Force }
    [System.IO.Compression.ZipFile]::ExtractToDirectory($nupkg, $folder)
    Remove-Item $nupkg -Force
    Set-Content -LiteralPath $marker -Value $hash
    return $folder
}

# The x64 .NET runtime (8 or later) the Artifact Signing dlib runs on (x64 SignTool hosts it).
function Test-X64DotNet {
    $roots = @()
    if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') {
        $roots += Join-Path $env:ProgramFiles 'dotnet\x64'
        $registered = Get-ItemProperty 'HKLM:\SOFTWARE\dotnet\Setup\InstalledVersions\x64' -Name InstallLocation -ErrorAction SilentlyContinue
        if ($registered) { $roots += $registered.InstallLocation }
    } else {
        $roots += Join-Path $env:ProgramFiles 'dotnet'
    }
    foreach ($root in $roots) {
        $shared = Join-Path $root 'shared\Microsoft.NETCore.App'
        if (Test-Path $shared) {
            foreach ($version in Get-ChildItem $shared -Directory) {
                if ([int]($version.Name.Split('.')[0]) -ge 8) { return $true }
            }
        }
    }
    return $false
}

# The signature on one file must be present, made by the expected signer and (with Artifact Signing) trusted by
# Windows and timestamped.
function Assert-Signed([string]$Path) {
    $signature = Get-AuthenticodeSignature -LiteralPath $Path
    $name = Split-Path $Path -Leaf
    if (-not $signature.SignerCertificate) { throw "$Path isn't signed." }
    if ($TestCert) {
        if ($signature.SignerCertificate.Thumbprint -ne $Signing.Thumbprint) {
            throw "$Path is signed by $($signature.SignerCertificate.Subject), not the test certificate."
        }
        Write-Host "  signed (test certificate): $name"
        return
    }
    if ($signature.Status -ne 'Valid') { throw "$Path's signature isn't valid: $($signature.Status) ($($signature.StatusMessage))" }
    if ($signature.SignerCertificate.Subject -ne $Signing.Subject) {
        throw "$Path is signed by '$($signature.SignerCertificate.Subject)', expected '$($Signing.Subject)'."
    }
    if (-not $signature.TimeStamperCertificate) { throw "$Path's signature isn't timestamped." }
    Write-Host "  signed: $name"
}

# ── Checks and signing identity ───────────────────────────────────────────────────────────────

if ($Repo.StartsWith('\\')) {
    throw "The repository is on a network share ($Repo). Copy it to a local disk first (Get-Help .\scripts\release-windows.ps1)."
}
foreach ($command in 'node', 'npm.cmd', 'cargo', 'rustup') {
    if (-not (Get-Command $command -ErrorAction SilentlyContinue)) { throw "$command isn't on PATH." }
}
$installedTargets = (& rustup target list --installed) -split "`r?`n"
foreach ($target in $Targets.Values) {
    if ($installedTargets -notcontains $target) { throw "The Rust target $target isn't installed: rustup target add $target" }
}

# ring needs clang for arm64, and the Visual C++ workload doesn't put it on PATH (CI's runner image ships LLVM).
if (-not (Get-Command clang -ErrorAction SilentlyContinue)) {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    $vs = if (Test-Path $vswhere) {
        & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Llvm.Clang -property installationPath
    }
    $llvmArch = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { 'ARM64' } else { 'x64' }
    $clangBin = if ($vs) { Join-Path $vs "VC\Tools\Llvm\$llvmArch\bin" }
    if (-not $clangBin -or -not (Test-Path (Join-Path $clangBin 'clang.exe'))) {
        throw 'clang is needed (ring builds with it): add "C++ Clang tools for Windows" to the Visual Studio Build Tools, or install LLVM.'
    }
    $env:Path = "$clangBin;$env:Path"
}

$Signing = @{}
if ($TestCert) {
    $certificate = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert |
        Where-Object { $_.Subject -eq $TestSubject -and $_.NotAfter -gt (Get-Date).AddDays(1) } |
        Sort-Object NotAfter -Descending | Select-Object -First 1
    if (-not $certificate) {
        Write-Step 'Creating the self-signed test certificate'
        $certificate = New-SelfSignedCertificate -Type CodeSigningCert -Subject $TestSubject -CertStoreLocation Cert:\CurrentUser\My `
            -KeyAlgorithm RSA -KeyLength 3072 -HashAlgorithm SHA256 -NotAfter (Get-Date).AddYears(1)
    }
    $Signing.Subject = $certificate.Subject
    $Signing.Thumbprint = $certificate.Thumbprint
    $Signing.Kind = 'self-signed test certificate (not trusted: testing only)'
} else {
    $endpoint = if ($env:AGENCY_SIGN_ENDPOINT) { $env:AGENCY_SIGN_ENDPOINT } else { 'https://eus.codesigning.azure.net/' }
    $account = if ($env:AGENCY_SIGN_ACCOUNT) { $env:AGENCY_SIGN_ACCOUNT } else { 'msitarzewski-signing' }
    $profileName = if ($env:AGENCY_SIGN_PROFILE) { $env:AGENCY_SIGN_PROFILE } else { 'autopaper' }
    $Signing.Subject = if ($env:AGENCY_SIGN_SUBJECT) { $env:AGENCY_SIGN_SUBJECT } else {
        'CN=Michael Sitarzewski, O=Michael Sitarzewski, L=Dallas, S=tx, C=US'
    }
    if (-not $endpoint.StartsWith('https://')) { throw "AGENCY_SIGN_ENDPOINT must be an https:// address, like https://eus.codesigning.azure.net/" }
    if (-not (Test-X64DotNet)) {
        throw ('The Artifact Signing dlib needs the x64 .NET runtime 8 or later (x64 SignTool loads it). Install it, then ' +
            'run this again: winget install --id Microsoft.DotNet.Runtime.8 --architecture x64')
    }
    # Fail before the build, not at its first signature, when there's no credential.
    $servicePrincipal = $env:AZURE_TENANT_ID -and $env:AZURE_CLIENT_ID -and $env:AZURE_CLIENT_SECRET
    if (-not $servicePrincipal) {
        if (-not (Get-Command az -ErrorAction SilentlyContinue)) {
            throw 'Signing needs the Azure CLI (winget install --id Microsoft.AzureCLI --exact) and az login, or a service principal (AZURE_TENANT_ID, AZURE_CLIENT_ID, AZURE_CLIENT_SECRET).'
        }
        $previous = $ErrorActionPreference
        $ErrorActionPreference = 'Continue'
        & az account show --query id -o tsv 2>&1 | Out-Null
        $loggedIn = $LASTEXITCODE -eq 0
        $ErrorActionPreference = $previous
        if (-not $loggedIn) { throw 'The Azure CLI isn''t logged in: az login, then run this again.' }
    }
    $Signing.Endpoint = $endpoint
    $Signing.Account = $account
    $Signing.Profile = $profileName
    $Signing.Kind = "Azure Artifact Signing ($account / $profileName)"
}
Write-Host "Agency Agents $Version for Windows ($($Targets.Keys -join ', ')), signed with the $($Signing.Kind)"
Write-Host "Signer: $($Signing.Subject)"

# ── Tools and the signing command ─────────────────────────────────────────────────────────────

Write-Step 'Tools'
$buildTools = Get-NuGetTool $SdkBuildTools
$sdkBin = Get-ChildItem (Join-Path $buildTools 'bin') -Directory | Sort-Object Name -Descending | Select-Object -First 1
# The dlib is x64 only, so x64 SignTool signs (under emulation on ARM64); the test certificate uses it too.
$SignTool = Join-Path $sdkBin.FullName 'x64\signtool.exe'
if (-not (Test-Path $SignTool)) { throw "$SignTool is missing" }
Write-Host "SignTool: $SignTool"

if (Test-Path $Stage) { Remove-Item $Stage -Recurse -Force }
New-Item -ItemType Directory -Force $Stage | Out-Null

if ($TestCert) {
    $signArgs = @('sign', '/fd', 'SHA256', '/s', 'My', '/sha1', $Signing.Thumbprint, '%1')
} else {
    $Dlib = Join-Path (Get-NuGetTool $SigningClient) 'bin\x64\Azure.CodeSigning.Dlib.dll'
    if (-not (Test-Path $Dlib)) { throw "$Dlib is missing from $($SigningClient.Id) $($SigningClient.Version)" }
    # DefaultAzureCredential, limited to a service principal's environment variables and the Azure CLI's login.
    $metadata = Join-Path $Stage 'artifact-signing.json'
    Write-Utf8 $metadata (([ordered]@{
        Endpoint = $Signing.Endpoint
        CodeSigningAccountName = $Signing.Account
        CertificateProfileName = $Signing.Profile
        ExcludeCredentials = @('ManagedIdentityCredential', 'WorkloadIdentityCredential', 'SharedTokenCacheCredential',
            'VisualStudioCredential', 'VisualStudioCodeCredential', 'AzurePowerShellCredential',
            'AzureDeveloperCliCredential', 'InteractiveBrowserCredential')
    }) | ConvertTo-Json)
    $signArgs = @('sign', '/v', '/fd', 'SHA256', '/tr', $TimestampUrl, '/td', 'SHA256', '/dlib', $Dlib, '/dmdf', $metadata, '%1')
}

# Merged over tauri.conf.json for this build only.
$buildConfig = Join-Path $Stage 'tauri.windows-release.json'
Write-Utf8 $buildConfig (@{
    bundle = @{
        createUpdaterArtifacts = $false
        windows = @{ signCommand = @{ cmd = $SignTool; args = $signArgs } }
    }
} | ConvertTo-Json -Depth 6)

# ── Build ─────────────────────────────────────────────────────────────────────────────────────

Push-Location $Repo
try {
    if (-not $SkipNpmCi) {
        Write-Step 'npm ci'
        Invoke-Native 'npm.cmd' @('ci', '--no-audit', '--no-fund')
    }

    if (Test-Path $Out) { Remove-Item $Out -Recurse -Force }
    New-Item -ItemType Directory -Force $Out | Out-Null
    $installers = @()
    foreach ($archName in $Targets.Keys) {
        $target = $Targets[$archName]
        $release = Join-Path $Repo "src-tauri\target\$target\release"
        $nsis = Join-Path $release 'bundle\nsis'
        if (Test-Path $nsis) { Remove-Item $nsis -Recurse -Force }

        Write-Step "Building and signing the $archName installer ($target)"
        Invoke-Native 'npm.cmd' @('run', 'tauri', '--', 'build', '--target', $target, '--bundles', 'nsis', '--config', $buildConfig)

        $built = @(Get-ChildItem $nsis -Filter "*_${Version}_$archName-setup.exe")
        if ($built.Count -ne 1) { throw "Expected one $archName installer in $nsis, found $($built.Count)." }

        # Tauri signs the app's executable before NSIS packs it, then puts the unsigned build back in target\, so the
        # signed executable and the uninstaller exist only inside the installer: an install shows them.
        Write-Step "Checking the $archName signatures"
        $plugins = Join-Path $release "nsis\$archName\Plugins\x86-unicode"
        foreach ($dll in 'NSISdl.dll', 'StartMenu.dll', 'System.dll', 'nsDialogs.dll', 'additional\nsis_tauri_utils.dll') {
            Assert-Signed (Join-Path $plugins $dll)
        }
        Assert-Signed $built[0].FullName

        $asset = Join-Path $Out ($built[0].Name -replace ' ', '_')
        Copy-Item -LiteralPath $built[0].FullName -Destination $asset
        $installers += $asset
    }
} finally {
    Pop-Location
}

$sums = ($installers | ForEach-Object {
    '{0}  {1}' -f (Get-FileHash -Algorithm SHA256 -LiteralPath $_).Hash.ToLowerInvariant(), (Split-Path $_ -Leaf)
}) -join "`n"
Write-Utf8 (Join-Path $Out 'SHA256SUMS.txt') "$sums`n"

if ($CopyTo) {
    New-Item -ItemType Directory -Force $CopyTo | Out-Null
    Copy-Item (Join-Path $Out '*') -Destination $CopyTo -Force
    Write-Host "Copied to $CopyTo"
}

Write-Step 'Done'
Get-ChildItem $Out | ForEach-Object { Write-Host "  $($_.Name)  $($_.Length) bytes" }
if ($TestCert) { Write-Warning 'Signed with the self-signed test certificate: these installers are for testing only.' }
