# FLOW — Windows Setup Script
# Run in PowerShell as Administrator

Write-Host ""
Write-Host "  FLOW — Windows Setup" -ForegroundColor Cyan
Write-Host ""

# Check if Rust is installed
if (!(Get-Command rustc -ErrorAction SilentlyContinue)) {
    Write-Host "  Installing Rust..." -ForegroundColor Yellow
    Invoke-WebRequest -Uri https://win.rustup.rs/x86_64 -OutFile rustup-init.exe
    .\rustup-init.exe -y
    Remove-Item rustup-init.exe
    $env:PATH += ";$env:USERPROFILE\.cargo\bin"
    Write-Host "  Rust installed!" -ForegroundColor Green
} else {
    Write-Host "  Rust already installed: $(rustc --version)" -ForegroundColor Green
}

# Build FLOW
Write-Host "  Building FLOW..." -ForegroundColor Yellow
cargo build --release
Write-Host ""
Write-Host "  Done! Run FLOW with:" -ForegroundColor Green
Write-Host '  .\target\release\flow-agent.exe --name "Razer" --color 1' -ForegroundColor White
Write-Host ""

# Open firewall for FLOW
Write-Host "  Opening firewall port 19847..." -ForegroundColor Yellow
netsh advfirewall firewall add rule name="FLOW" dir=in action=allow protocol=TCP localport=19847
netsh advfirewall firewall add rule name="FLOW-UDP" dir=in action=allow protocol=UDP localport=5353
Write-Host "  Firewall configured!" -ForegroundColor Green
Write-Host ""
