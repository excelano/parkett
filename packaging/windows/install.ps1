# Install Parkett for one person from a source checkout: the executable, the
# icons, the .parquet association with its media type, a Start menu entry and
# an uninstall entry. Per-user, under HKCU and %LOCALAPPDATA%, with no
# administrator; the counterpart of packaging/linux/install.sh.
#
# Author: David M. Anderson
# Built with AI assistance (Claude, Anthropic)

[CmdletBinding()]
param(
    # Where to install. The default is the per-user location Windows names for
    # applications that do not go through an installer service.
    [string] $Prefix = (Join-Path $env:LOCALAPPDATA 'Programs\Parkett'),
    # The desktop executable to install. With neither this nor -NoBinary, a
    # built one is looked for.
    [string] $Binary,
    # Install the integration only.
    [switch] $NoBinary
)

$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path

# The one file type. The media type is IANA's and the one
# packaging/linux/mime/parquet.xml and AppxManifest.xml.in declare.
$Types = @(
    @{
        Extension   = '.parquet'
        ContentType = 'application/vnd.apache.parquet'
        ProgId      = 'Excelano.Parkett.Parquet'
        TypeName    = 'Apache Parquet file'
        Icon        = 'parquet.ico'
    }
)

$appName = 'Parkett'
$exeName = 'parkett.exe'
$appIcon = 'parkett.ico'

# --- writing to the registry ------------------------------------------------

# The .NET API rather than PowerShell's registry provider, because of two keys
# here: the media type is `application/vnd.apache.parquet`, and the provider
# reads the forward slash as a path separator and silently creates `application` with a child
# instead of the single key that was asked for. Measured on slipcase-desktop,
# not guessed. The .NET API takes the whole string as one name, which is what
# the MIME database wants. An empty $Name is the key's default value.
function Set-RegistryValue {
    param([string] $Path, [string] $Name, $Value, [string] $Kind = 'String')
    $key = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey($Path)
    try {
        $key.SetValue($Name, $Value, [Microsoft.Win32.RegistryValueKind] $Kind)
    } finally {
        $key.Close()
    }
}

# --- the executables --------------------------------------------------------

# Cargo is asked where its target directory is rather than guessed at, because
# `[build] target-dir` in a Cargo configuration file moves it and no
# environment variable then says so. The Linux script learned this the hard way
# and this one inherits the lesson rather than repeating it.
function Find-TargetDir {
    $targetDir = $null
    if (Get-Command cargo -ErrorAction SilentlyContinue) {
        Push-Location (Join-Path $here '..\..')
        try {
            $meta = cargo metadata --format-version 1 --no-deps 2>$null | ConvertFrom-Json
            if ($meta) { $targetDir = $meta.target_directory }
        } catch { }
        finally { Pop-Location }
    }
    if (-not $targetDir) { $targetDir = Join-Path $here '..\..\target' }
    return $targetDir
}

function Find-Built([string] $name) {
    $targetDir = Find-TargetDir
    foreach ($built in 'release', 'debug') {
        $candidate = Join-Path $targetDir "$built\$name"
        if (Test-Path -LiteralPath $candidate) { return (Resolve-Path -LiteralPath $candidate).Path }
    }
    return $null
}

$foundBinary = $null
if ($NoBinary) {
    # Nothing to find.
} elseif ($Binary) {
    if (-not (Test-Path -LiteralPath $Binary)) { throw "install.ps1: $Binary is not there" }
    $foundBinary = (Resolve-Path -LiteralPath $Binary).Path
} else {
    $foundBinary = Find-Built $exeName
}

# --- the files --------------------------------------------------------------

New-Item -ItemType Directory -Force -Path $Prefix | Out-Null

# The application's icon and the document's, both from make-ico.
$icons = @($appIcon) + ($Types | ForEach-Object { $_.Icon })
foreach ($icon in $icons) {
    $source = Join-Path $here $icon
    if (-not (Test-Path -LiteralPath $source)) {
        throw "install.ps1: $icon is not beside this script; run make-ico first"
    }
    Copy-Item -LiteralPath $source -Destination (Join-Path $Prefix $icon) -Force
}
$installedAppIcon = Join-Path $Prefix $appIcon

# The uninstaller is copied in rather than run from the repository, because the
# Add/Remove Programs entry below points at it and a checkout is not something
# that has to still be there a year later.
Copy-Item -LiteralPath (Join-Path $here 'uninstall.ps1') `
          -Destination (Join-Path $Prefix 'uninstall.ps1') -Force

$installedExe = Join-Path $Prefix $exeName
if ($foundBinary) {
    # An upgrade over a running copy is the one failure here a person meets in
    # the ordinary course of things, and Windows will not let a running
    # executable be overwritten. Left to itself the script stops with a .NET
    # IOException and a stack trace naming Copy-Item, which is true and tells
    # nobody what to do. slipcase-desktop walked this on 2026-08-26; the run
    # stopped before the registry stage, so nothing was left half-registered,
    # and that part is worth keeping exactly as it is.
    try {
        Copy-Item -LiteralPath $foundBinary -Destination $installedExe -Force
    } catch [System.IO.IOException] {
        $running = Get-Process -Name ([System.IO.Path]::GetFileNameWithoutExtension($exeName)) `
                               -ErrorAction SilentlyContinue |
                   Where-Object { $_.Path -eq $installedExe }
        if ($running) {
            throw "install.ps1: Parkett is running from $installedExe, so it cannot be replaced. " +
                  "Close it and run this again. Nothing has been changed."
        }
        throw
    }
    Write-Output "installed $installedExe from $foundBinary"
} elseif (-not (Test-Path -LiteralPath $installedExe)) {
    Write-Warning "no executable installed; the associations will point at $installedExe, which is not there yet"
}


# --- the registry -----------------------------------------------------------

$classes = 'Software\Classes'

foreach ($type in $Types) {
    $progId = $type.ProgId
    $installedIcon = Join-Path $Prefix $type.Icon

    # The type itself. `FriendlyTypeName` is what Explorer's Type column shows
    # and it is written as a plain string: the usual form is a reference into a
    # binary's resource table - `@C:\path\thing.dll,-123` - which needs
    # `SHLoadIndirectString` to read back, and nothing this project ships would
    # resolve one.
    Set-RegistryValue "$classes\$progId" '' $type.TypeName
    Set-RegistryValue "$classes\$progId" 'FriendlyTypeName' $type.TypeName
    Set-RegistryValue "$classes\$progId\DefaultIcon" '' "$installedIcon,0"
    Set-RegistryValue "$classes\$progId\shell\open\command" '' "`"$installedExe`" `"%1`""

    # The application behind the type. `ApplicationName` is the first place the
    # shell looks for a name a person recognises.
    Set-RegistryValue "$classes\$progId\Application" 'ApplicationName' $appName
    Set-RegistryValue "$classes\$progId\Application" 'ApplicationCompany' 'Excelano'
    Set-RegistryValue "$classes\$progId\Application" 'ApplicationDescription' `
        'View Apache Parquet files'
    Set-RegistryValue "$classes\$progId\Application" 'ApplicationIcon' "$installedIcon,0"

    # The extension, and its media type. `Content Type` here and the MIME
    # database entry below are the two halves of the same statement, and Windows
    # uses each in a different direction: name to type, and type to name.
    Set-RegistryValue "$classes\$($type.Extension)" '' $progId
    Set-RegistryValue "$classes\$($type.Extension)" 'Content Type' $type.ContentType
    Set-RegistryValue "$classes\$($type.Extension)\OpenWithProgids" $progId ''
    Set-RegistryValue "$classes\MIME\Database\Content Type\$($type.ContentType)" `
        'Extension' $type.Extension
}

# The Open With list, so a person can reach this application from a file it was
# not registered for, and so the shell has a name for the executable itself.
# One entry, for the one executable.
$applications = "$classes\Applications\$exeName"
Set-RegistryValue $applications 'FriendlyAppName' $appName
Set-RegistryValue "$applications\shell\open\command" '' "`"$installedExe`" `"%1`""
foreach ($type in $Types) {
    Set-RegistryValue "$applications\SupportedTypes" $type.Extension ''
}

# --- the Start menu ---------------------------------------------------------

# The counterpart of the `.desktop` entry: what puts the application in front of
# a person who has not got a document to double-click yet. The shortcut carries
# the application icon, which matters more here than on Linux - see the note
# about the window icon in README.md.
$startMenu = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs'
$shortcut = Join-Path $startMenu 'Parkett.lnk'
if (Test-Path -LiteralPath $installedExe) {
    $shell = New-Object -ComObject WScript.Shell
    $link = $shell.CreateShortcut($shortcut)
    $link.TargetPath = $installedExe
    $link.WorkingDirectory = $Prefix
    $link.IconLocation = "$installedAppIcon,0"
    $link.Description = 'View Apache Parquet files'
    $link.Save()
    # Deliberately no AppUserModelID on this shortcut. Setting one would need
    # the running process to declare the same identity through
    # SetCurrentProcessExplicitAppUserModelID, which is a raw call this
    # application cannot make under `#![deny(unsafe_code)]`. With neither side
    # declaring one, Windows derives both from the executable path, they agree,
    # and pinning and taskbar grouping work. README.md has the whole of it.
}

# --- Add/Remove Programs ----------------------------------------------------

$version = '0.1.0'
$cargoToml = Join-Path $here '..\..\Cargo.toml'
if (Test-Path -LiteralPath $cargoToml) {
    $line = Select-String -LiteralPath $cargoToml -Pattern '^version = "([^"]+)"' | Select-Object -First 1
    if ($line) { $version = $line.Matches[0].Groups[1].Value }
}

$uninstallKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\Parkett'
$uninstallCommand = "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$(Join-Path $Prefix 'uninstall.ps1')`""

# **Not plain "Parkett", and the suffix is a defect report.** The Store package
# names itself Parkett with a publisher of Excelano, and an entry of the same
# name would show two rows in Settings > Apps reading "Parkett" by
# "Excelano" with the same icon, separated by nothing but 0.1.0 against
# 0.1.0.0. Asked to remove one, a person picks the row that says Excelano,
# which is both of them, and removes the package instead of this.
#
# That is worse than an annoyance because of what the README measured. A script
# install silently shadows the package - the extension's default ProgID wins
# over a packaged association - so somebody who installs from the Store and
# wants the Store copy to be the one that opens their files has to remove this
# one, and until now the remedy was a coin flip.
#
# The package's name is fixed by the manifest and the reservation. This one is
# ours, so this one changes.
Set-RegistryValue $uninstallKey 'DisplayName' 'Parkett (user install)'
Set-RegistryValue $uninstallKey 'DisplayVersion' $version
Set-RegistryValue $uninstallKey 'Publisher' 'Excelano'
Set-RegistryValue $uninstallKey 'DisplayIcon' "$installedAppIcon,0"
Set-RegistryValue $uninstallKey 'InstallLocation' $Prefix
Set-RegistryValue $uninstallKey 'UninstallString' $uninstallCommand
Set-RegistryValue $uninstallKey 'QuietUninstallString' $uninstallCommand
Set-RegistryValue $uninstallKey 'NoModify' 1 'DWord'
Set-RegistryValue $uninstallKey 'NoRepair' 1 'DWord'

# --- tell the shell ---------------------------------------------------------

# Without this the icons and the type descriptions appear at the next logon
# rather than now, which reads as the associations not having worked.
Add-Type -Namespace Parkett -Name Shell -MemberDefinition @'
[DllImport("shell32.dll", CharSet=CharSet.Unicode)]
public static extern void SHChangeNotify(int eventId, uint flags, System.IntPtr item1, System.IntPtr item2);
'@
[Parkett.Shell]::SHChangeNotify(0x08000000, 0, [System.IntPtr]::Zero, [System.IntPtr]::Zero)

Write-Output ""
foreach ($type in $Types) {
    Write-Output "registered $($type.Extension) as $($type.ContentType), opened by $($type.ProgId)"
}
Write-Output "under $Prefix"
Write-Output ""
Write-Output "check it with:"
# Not `assoc` and `ftype`. Both report these extensions as having no association
# at all after a successful install: they read and write the machine-wide half
# of the class root only, and everything above is per-user. Measured on
# slipcase-desktop, after they were put in that script first and printed exactly
# the message a failed install would have.
foreach ($type in $Types) {
    Write-Output "  reg query `"HKCU\Software\Classes\$($type.Extension)`" /s"
}
Write-Output "and by double-clicking a .parquet file, which should open here"
Write-Output "with its own icon in the listing rather than the other one's."
