# Day 6 Metrics Testing Script
Write-Host "Testing EchoTrap metrics endpoint..."

try {
    $response = Invoke-RestMethod -Uri "http://127.0.0.1:19080/metrics" -Method Get
    Write-Host "`n=== Current Metrics ==="
    Write-Host "Connections: $($response.connections)"
    Write-Host "Attacks: $($response.attacks)"
    Write-Host "Migrations: $($response.migrations)"
    Write-Host "`nMetrics endpoint working correctly!"
} catch {
    Write-Host "Error: Could not connect to metrics endpoint"
    Write-Host "Make sure EchoTrap is running with dashboard enabled"
}