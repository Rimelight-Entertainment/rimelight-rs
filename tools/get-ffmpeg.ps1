# Downloads the trimmed FFmpeg that the Rimelight apps link against (built by tools/ffmpeg/build.sh and published as a
# release asset), checks its SHA-256 and unpacks it to <Dest>/current. The release, asset, SHA-256 and URL come from
# tools/ffmpeg/pin.txt. Run once per machine, and again when the pin changes (the folder's PINNED_RELEASE.txt says
# which build it holds).
#
# Usage (Rimelight Media, from its repo root):
#   powershell -ExecutionPolicy Bypass -File tools/get-ffmpeg.ps1 -Dest ffmpeg
# Usage (any app that wants the folder elsewhere, e.g. a target folder):
#   powershell -ExecutionPolicy Bypass -File tools/get-ffmpeg.ps1 -Dest <folder>
# Then point the build at it, or leave it where the vendored ffmpeg-sys-next looks (<folder>/current above OUT_DIR).
param(
    [Parameter(Mandatory = $true)]
    [string]$Dest
)

$ErrorActionPreference = 'Stop'

# key=value lines of tools/ffmpeg/pin.txt (# starts a comment).
$pinFile = Join-Path $PSScriptRoot 'ffmpeg\pin.txt'
$pin = @{}
foreach ($line in Get-Content -LiteralPath $pinFile) {
    $t = $line.Trim()
    if ($t -eq '' -or $t.StartsWith('#')) { continue }
    $i = $t.IndexOf('=')
    if ($i -gt 0) { $pin[$t.Substring(0, $i).Trim()] = $t.Substring($i + 1).Trim() }
}
foreach ($key in 'release', 'asset', 'sha256', 'url') {
    if (-not $pin.ContainsKey($key)) { throw "$pinFile has no '$key=' line" }
}
$Release = $pin['release']
$Asset = $pin['asset']
$Sha256 = $pin['sha256']
$Url = $pin['url']

$dest = $Dest
$current = Join-Path $dest 'current'
$stamp = Join-Path $current 'PINNED_RELEASE.txt'
if (Test-Path (Join-Path $current 'lib')) {
    $have = (Get-Content -LiteralPath $stamp -Raw -ErrorAction SilentlyContinue)
    if ($null -ne $have -and $have.Trim() -eq "$Release/$Asset") {
        Write-Host "FFmpeg is already set up in $current"
        exit 0
    }
}
New-Item -ItemType Directory -Force $dest | Out-Null
$zip = Join-Path $dest $Asset
if (-not (Test-Path $zip)) {
    Write-Host "Downloading $Url"
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    Invoke-WebRequest -Uri $Url -OutFile $zip -UseBasicParsing
}
$hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $zip).Hash
if ($hash -ne $Sha256) {
    Remove-Item $zip
    throw "$Asset has SHA-256 $hash, expected $Sha256; the file was removed"
}
$tmp = Join-Path $dest 'unpack'
if (Test-Path $tmp) { Remove-Item -Recurse -Force $tmp }
Expand-Archive -LiteralPath $zip -DestinationPath $tmp
# The zip holds one folder named like the asset.
$inner = Get-ChildItem $tmp -Directory | Select-Object -First 1
if (Test-Path $current) { Remove-Item -Recurse -Force $current }
Move-Item $inner.FullName $current
Remove-Item -Recurse -Force $tmp
Remove-Item $zip
Set-Content -LiteralPath $stamp -Value "$Release/$Asset" -Encoding ascii # read by the up-to-date check above (PIN in build.rs)
Write-Host "FFmpeg is set up in $current"
