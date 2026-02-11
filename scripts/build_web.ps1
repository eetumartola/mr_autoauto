$ErrorActionPreference = "Stop"

Set-Location -Path (Join-Path $PSScriptRoot "..")

Write-Host "Ensuring wasm target is installed..."
rustup target add wasm32-unknown-unknown

if (-not (Get-Command trunk -ErrorAction SilentlyContinue)) {
    Write-Host "Installing trunk..."
    cargo install trunk --locked
}

Write-Host "Building web bundle (release, web-safe feature set)..."
trunk build `
    --config Trunk.toml `
    --release `
    --no-default-features `
    --minify false

if ($LASTEXITCODE -ne 0) {
    throw "trunk build failed with exit code $LASTEXITCODE"
}

Write-Host "Web build complete: web/dist"
