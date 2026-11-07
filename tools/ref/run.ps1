$ErrorActionPreference = 'Stop'
Set-Location (Resolve-Path "$PSScriptRoot\..\..")

cargo build
if ($LASTEXITCODE) { exit $LASTEXITCODE }
node tools/ref/make.mjs
if ($LASTEXITCODE) { exit $LASTEXITCODE }

$snapFail = 0
foreach ($s in Get-ChildItem tools\ref\scenarios\*.js) {
  & target\debug\blip.exe --snapshot "tools\ref\scenarios\$($s.Name)" "target\ref\$($s.BaseName).native.png"
  if ($LASTEXITCODE) { Write-Host "$($s.BaseName): native snapshot failed ($LASTEXITCODE)"; $snapFail = 1 }
}

cargo run --example uidiff -- target/ref
if ($LASTEXITCODE) { exit $LASTEXITCODE }
exit $snapFail
