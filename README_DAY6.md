# Day 6: Testing & Dashboard

## Testing Workflow

### 1. Start EchoTrap
```powershell
cargo run -- --port 9000 --threshold 3 --window 10
```

### 2. Check Metrics Dashboard
Open browser: `http://127.0.0.1:8080/metrics`

Or use PowerShell:
```powershell
.\test_metrics.ps1
```

### 3. Run Attack Simulation
```powershell
.\test_attack.ps1
```

## Expected Results

### Normal Traffic
- Connections increment: `total_connections=1, 2, ...`
- No alerts or migrations
- Metrics show increasing connection count

### Attack Simulation
- After 3 rapid connections: `[ALERT] Port scan/brute-force suspected`
- Migration triggered: `Migration requested — attempting to move to port XXXXX`
- Old listener shuts down, new one starts
- Metrics show: `attacks=1, migrations=1`

### Metrics Endpoint
JSON response format:
```json
{
  "connections": 15,
  "attacks": 2,
  "migrations": 2
}
```

## Validation Checklist

- [ ] Normal connections work without triggering alerts
- [ ] Rapid connections trigger attack detection
- [ ] Port migration occurs automatically
- [ ] Metrics endpoint returns valid JSON
- [ ] Connection/attack/migration counters are accurate
- [ ] Old listener shuts down cleanly
- [ ] New listener accepts connections on migrated port

## Testing Commands

### Manual Connection Test
```powershell
$client = New-Object System.Net.Sockets.TcpClient
$client.Connect('localhost', 9000)
$client.Close()
```

### Check Current Port
Look for "EchoTrap listening on 0.0.0.0:XXXXX" in logs

### Verify Migration
After attack, connect to new port shown in migration logs