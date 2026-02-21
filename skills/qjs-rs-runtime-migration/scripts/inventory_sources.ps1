param(
    [string]$QuickJsPath = "D:\dev\QuickJS",
    [string]$BoaPath = "D:\dev\boa"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Get-Inventory {
    param([string]$RootPath)

    if (-not (Test-Path $RootPath)) {
        throw "Path not found: $RootPath"
    }

    $files = Get-ChildItem -Path $RootPath -Recurse -File
    $topDirs = Get-ChildItem -Path $RootPath -Directory |
        Select-Object -ExpandProperty Name |
        Sort-Object

    return [PSCustomObject]@{
        path = $RootPath
        file_count = $files.Count
        top_level_dirs = $topDirs
    }
}

$quickjs = Get-Inventory -RootPath $QuickJsPath
$boa = Get-Inventory -RootPath $BoaPath

$result = [PSCustomObject]@{
    quickjs = $quickjs
    boa = $boa
}

$result | ConvertTo-Json -Depth 6
