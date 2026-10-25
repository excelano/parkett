# The Microsoft Store screenshots, as recipes. take-shots.ps1 and
# screenshot.ps1 beside this are the fleet's generated driver; this file is the
# part that is Parkett's: which file, and what the keys do before each shutter.
#
#     powershell -ExecutionPolicy Bypass -File packaging\windows\shots.ps1
#     ...\shots.ps1 -SideLoaded            # a build that is not packaged yet
#     ...\shots.ps1 -Reference             # one plain frame, to look at
#
# Every step is a key and none is a click: the grid takes the arrows, Shift
# with them and Ctrl+I as soon as the window has focus, so the recipes hold no
# coordinates and the German set is the English recipe in a German window.
# The file is packaging/demo/observations.parquet, which examples/demo.rs
# writes.
#
# Author: David M. Anderson
# Built with AI assistance (Claude, Anthropic)

[CmdletBinding()]
param(
    [switch] $Reference,
    [string] $OutDir,
    [switch] $SideLoaded,
    [ValidateSet('en', 'de')]
    [string] $Lang = 'en'
)

$ErrorActionPreference = 'Stop'

. "$PSScriptRoot\take-shots.ps1"

$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

# The Store's minimum for a desktop screenshot.
$WIDTH = 1366
$HEIGHT = 768
$FILE = Join-Path $root 'packaging\demo\observations.parquet'
$PROCESS = 'parkett'
if (-not $OutDir) { $OutDir = Join-Path $root 'dist\screenshots' }

# Two steps of egui's own zoom, so the text survives the Store's thumbnails.
$ZOOM = @('key ctrl+plus', 'key ctrl+plus')

function Opens {
    if (-not (Test-Path -LiteralPath $FILE)) { Write-Error "shots.ps1: no demo file at $FILE" }
    if ($SideLoaded) {
        $exe = Join-Path $env:LOCALAPPDATA 'Programs\Parkett\parkett.exe'
        if (-not (Test-Path -LiteralPath $exe)) {
            Write-Error "shots.ps1: no side-loaded build at $exe - run install.ps1 first"
        }
        return @('exe', $exe, $FILE)
    }
    $identity = Import-PowerShellDataFile (Join-Path $PSScriptRoot 'identity.psd1')
    if (-not (Get-AppxPackage $identity.Name -ErrorAction SilentlyContinue)) {
        Write-Error "shots.ps1: $($identity.Name) is not installed - build-msix.ps1 -SelfSign and Add-AppxPackage it, or pass -SideLoaded"
    }
    return @('package', "$($identity.PackageFamilyName)!Parkett", $FILE)
}

function Repeat([string] $step, [int] $times) { return @(1..$times | ForEach-Object { $step }) }

# One shot to a line: its name, then the steps that put the window into the
# state photographed.
function Get-Shots {
    # The grid, with a list selected and shown whole beneath it.
    Shot '01-grid' ((Repeat 'key down' 4) + (Repeat 'key right' 6)) -Settle 2

    # The file panel beside the grid.
    Shot '02-details' ((Repeat 'key down' 2) + (Repeat 'key right' 3) + @('key ctrl+i')) -Settle 2

    # A range, as it would be copied.
    Shot '03-range' ((Repeat 'key down' 3) + (Repeat 'key shift+down' 9) + (Repeat 'key shift+right' 4)) -Settle 2
}

Take-Shots -Launch (Opens) -Process $PROCESS `
    -Width $WIDTH -Height $HEIGHT -OutDir $OutDir -Lang $Lang -Reference:$Reference `
    -EveryShot $ZOOM
