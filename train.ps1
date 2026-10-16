# Arctos OS Host AI Trainer Runner
$HostTarget = (rustc -vV | Select-String "host:" | ForEach-Object { $_.ToString().Split(":")[1].Trim() })
Write-Host "[Arctos] Running trainer on host target: $HostTarget"
cargo run --package trainer --target $HostTarget
