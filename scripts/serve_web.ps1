$ErrorActionPreference = "Stop"

Set-Location -Path (Join-Path $PSScriptRoot "..")

Write-Host "Ensuring wasm target is installed..."
rustup target add wasm32-unknown-unknown

if (-not (Get-Command trunk -ErrorAction SilentlyContinue)) {
    Write-Host "Installing trunk..."
    cargo install trunk --locked
}

Write-Host "Starting trunk dev server (web-safe feature set)..."
trunk serve `
    --config Trunk.toml `
    --release `
    --no-default-features `
    --minify false `
    --address 127.0.0.1 `
    --port 8080

if ($LASTEXITCODE -ne 0) {
    throw "trunk serve failed with exit code $LASTEXITCODE"
}
