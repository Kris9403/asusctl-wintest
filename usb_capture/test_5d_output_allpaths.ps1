Add-Type -Path "C:\Users\Krushna\AppData\Local\Temp\claude\C--Users-Krushna-claude\adbed461-e0a3-4f3c-84e7-623be742f445\scratchpad\usb_capture\HidSend.cs"

$paths = [HidSend]::EnumeratePaths(0x0B05, 0x19B6)

$pkt = New-Object byte[] 64
$pkt[0]=0x5d; $pkt[1]=0xb3; $pkt[2]=0; $pkt[3]=2; $pkt[7]=0xeb

foreach ($p in $paths) {
    $ok = [HidSend]::TrySetOutputReport($p, $pkt)
    Write-Output "$(if ($ok) {'SUCCESS'} else {"failed err=$([HidSend]::LastError())"})  on $p"
}
