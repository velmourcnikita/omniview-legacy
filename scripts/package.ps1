param([ValidateSet('x64','x86')][string]$Architecture='x86',[switch]$SkipBuild)
$ErrorActionPreference='Stop'
$taskRoot=Split-Path -Parent $PSScriptRoot
Push-Location $taskRoot
try {
    $taskTarget=if($Architecture -eq 'x86'){'i686-win7-windows-msvc'}else{'x86_64-win7-windows-msvc'}
    $env:RUSTC_BOOTSTRAP='1'
    if(-not $SkipBuild){
        cargo build '-Zbuild-std=std,panic_abort' --locked --release --target $taskTarget
        if($LASTEXITCODE -ne 0){throw 'Legacy build failed'}
    }
    $taskDist=Join-Path $taskRoot "dist/$Architecture"
    New-Item -ItemType Directory -Path $taskDist -Force | Out-Null
    foreach($taskName in @('OmniView.exe','OmniView-Student.exe')){
        Copy-Item -LiteralPath "target/$taskTarget/release/omniview.exe" -Destination (Join-Path $taskDist $taskName)
    }
    Copy-Item -LiteralPath README.md -Destination $taskDist
    Copy-Item -LiteralPath LICENSE -Destination $taskDist
    $taskFiles=foreach($taskName in @('OmniView.exe','OmniView-Student.exe','README.md','LICENSE')){
        $taskFile=Join-Path $taskDist $taskName
        [ordered]@{name=$taskName;bytes=(Get-Item -LiteralPath $taskFile).Length;sha256=(Get-FileHash -LiteralPath $taskFile -Algorithm SHA256).Hash.ToLowerInvariant();architecture=$Architecture}
    }
    $taskFiles | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskDist 'manifest.json') -Encoding utf8
    python scripts/check-imports.py (Join-Path $taskDist 'OmniView.exe')
    if($LASTEXITCODE -ne 0){throw 'Import check failed'}
    Get-ChildItem -LiteralPath $taskDist | Select-Object Name,Length
}finally{Pop-Location}
