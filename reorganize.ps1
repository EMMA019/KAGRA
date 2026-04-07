# reorganize.ps1 (Simplified version)
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$root = $PSScriptRoot

Write-Host "Starting reorganization..."

function Move-Safe($src, $dst) {
    if (-not (Test-Path $src)) { return }
    $dstDir = Split-Path $dst -Parent
    if (-not (Test-Path $dstDir)) { New-Item -ItemType Directory -Path $dstDir | Out-Null }
    if (-not (Test-Path $dst)) {
        Move-Item -Path $src -Destination $dst -Force
        Write-Host "Moved: $src to $dst"
    }
}

# [1] python\kagra -> kagra
$pyKagra = Join-Path $root "python\kagra"
if (Test-Path $pyKagra) {
    Get-ChildItem $pyKagra -File | ForEach-Object {
        Move-Safe $_.FullName (Join-Path $root "kagra\$($_.Name)")
    }
}

# [2] rpg/ -> kagra/
foreach ($f in @("anim_state.py","effects.py","mapgen.py")) {
    Move-Safe (Join-Path $root "rpg\$f") (Join-Path $root "kagra\$f")
}
Move-Safe (Join-Path $root "rpg\mapgen_preview.py") (Join-Path $root "tools\mapgen_preview.py")
Move-Safe (Join-Path $root "rpg\main.py") (Join-Path $root "examples\rpg_demo.py")

# [3] assets
$imgRoot = Join-Path $root "assets\img"
if (Test-Path $imgRoot) {
    Get-ChildItem $imgRoot -File -Filter "char_*.png" | ForEach-Object {
        Move-Safe $_.FullName (Join-Path $root "assets\img\player\$($_.Name)")
    }
}
$exMaps = Join-Path $root "examples\assets\maps"
if (Test-Path $exMaps) {
    Get-ChildItem $exMaps -File | ForEach-Object {
        Move-Safe $_.FullName (Join-Path $root "assets\maps\$($_.Name)")
    }
}

# [5] pyproject.toml
$ppt = Join-Path $root "pyproject.toml"
if (Test-Path $ppt) {
    $content = Get-Content $ppt -Raw
    if ($content -match 'python-source\s*=\s*"python"') {
        $content = $content -replace 'python-source\s*=\s*"python"', 'python-source = "."'
        Set-Content $ppt $content -Encoding UTF8
    }
}

Write-Host "=== Done ==="