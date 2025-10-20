$client = New-Object System.Net.Sockets.TcpClient
$client.Connect('localhost', 9000)
$stream = $client.GetStream()
$reader = New-Object System.IO.StreamReader($stream)
$writer = New-Object System.IO.StreamWriter($stream)
$banner = $reader.ReadLine()
Write-Host 'Banner:' $banner
$writer.WriteLine('Hello EchoTrap')
$writer.Flush()
Start-Sleep -Milliseconds 100
$response = $reader.ReadLine()
Write-Host 'Echo:' $response
$client.Close()