# Day 6 Attack Simulation Script
Write-Host "Starting EchoTrap attack simulation..."

# Test 1: Normal connections (should not trigger migration)
Write-Host "`n=== Test 1: Normal Traffic ==="
for ($i = 1; $i -le 2; $i++) {
    try {
        $client = New-Object System.Net.Sockets.TcpClient
        $client.Connect('localhost', 9000)
        Write-Host "Normal connection $i successful"
        $client.Close()
        Start-Sleep -Seconds 2
    } catch {
        Write-Host "Connection failed (port may have migrated)"
    }
}

# Test 2: Rapid connections (should trigger migration)
Write-Host "`n=== Test 2: Attack Simulation ==="
for ($i = 1; $i -le 5; $i++) {
    try {
        $client = New-Object System.Net.Sockets.TcpClient
        $client.Connect('localhost', 9000)
        Write-Host "Attack connection $i"
        $client.Close()
        Start-Sleep -Milliseconds 100
    } catch {
        Write-Host "Connection $i failed (expected after migration)"
    }
}

Write-Host "`n=== Check metrics at http://127.0.0.1:18080/metrics ==="