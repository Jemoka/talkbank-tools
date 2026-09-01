$ErrorActionPreference = "Stop"

if (-not (Get-Command uv -ErrorAction SilentlyContinue)) {
    Write-Host "Installing uv..."
    Invoke-RestMethod https://astral.sh/uv/install.ps1 | Invoke-Expression
    $env:Path = "$env:USERPROFILE\.local\bin;$env:USERPROFILE\.cargo\bin;$env:Path"
}

if (-not (Get-Command uv -ErrorAction SilentlyContinue)) {
    throw (
        "uv was installed, but it is not available on PATH. " +
        "Start a new shell and rerun this script."
    )
}

$batchalignPattern = '^batchalign v.*\[extras: ([^]]*, )?all(, [^]]*)?\]'
$batchalignTool = uv tool list --show-extras | Select-String $batchalignPattern
if ($LASTEXITCODE -ne 0) {
    throw "Unable to inspect installed uv tools."
}

if ($batchalignTool) {
    Write-Host "Upgrading batchalign[all]..."
    uv tool install --upgrade --python=3.11 --prerelease=allow 'batchalign[all]'
} else {
    Write-Host "Installing batchalign[all]..."
    uv tool install --python=3.11 --prerelease=allow 'batchalign[all]'
}

if ($LASTEXITCODE -ne 0) {
    throw "Unable to install batchalign[all]."
}

Write-Host "Batchalign is ready."
