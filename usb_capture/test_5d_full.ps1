Add-Type -Path "C:\Users\Krushna\AppData\Local\Temp\claude\C--Users-Krushna-claude\adbed461-e0a3-4f3c-84e7-623be742f445\scratchpad\usb_capture\HidSend.cs"

$paths = [HidSend]::EnumeratePaths(0x0B05, 0x19B6)
$targetPath = $paths | Where-Object { $_ -match "mi_00&col04" } | Select-Object -First 1
Write-Output "Using: $targetPath"

function Build-Effect-Packet {
    param([int]$zone, [int]$mode, [byte]$r1, [byte]$g1, [byte]$b1, [byte]$speed, [byte]$dir, [byte]$r2, [byte]$g2, [byte]$b2)
    $pkt = New-Object byte[] 64
    $pkt[0] = 0x5d
    $pkt[1] = 0xb3
    $pkt[2] = [byte]$zone
    $pkt[3] = [byte]$mode
    $pkt[4] = $r1; $pkt[5] = $g1; $pkt[6] = $b1
    $pkt[7] = $speed
    $pkt[8] = $dir
    $pkt[9] = 0
    $pkt[10] = $r2; $pkt[11] = $g2; $pkt[12] = $b2
    return $pkt
}
function Build-Simple-Packet {
    param([byte]$cmd)
    $pkt = New-Object byte[] 64
    $pkt[0] = 0x5d
    $pkt[1] = $cmd
    return $pkt
}

# RainbowCycle (mode=2), zone=0 (all), medium speed
$effectPkt = Build-Effect-Packet -zone 0 -mode 2 -r1 0 -g1 0 -b1 0 -speed 0xeb -dir 0 -r2 0 -g2 0 -b2 0
$setPkt = Build-Simple-Packet -cmd 0xb5
$applyPkt = Build-Simple-Packet -cmd 0xb4

$ok1 = [HidSend]::TrySetFeature($targetPath, $effectPkt)
Write-Output "effect (b3): $(if ($ok1) {'SUCCESS'} else {"failed err=$([HidSend]::LastError())"})"
$ok2 = [HidSend]::TrySetFeature($targetPath, $setPkt)
Write-Output "set (b5): $(if ($ok2) {'SUCCESS'} else {"failed err=$([HidSend]::LastError())"})"
$ok3 = [HidSend]::TrySetFeature($targetPath, $applyPkt)
Write-Output "apply (b4): $(if ($ok3) {'SUCCESS'} else {"failed err=$([HidSend]::LastError())"})"
