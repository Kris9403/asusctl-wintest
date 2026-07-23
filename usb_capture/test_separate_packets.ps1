Add-Type -Path "C:\Users\Krushna\AppData\Local\Temp\claude\C--Users-Krushna-claude\adbed461-e0a3-4f3c-84e7-623be742f445\scratchpad\usb_capture\HidSend.cs"

function Build-Single-Zone-Packet {
    param([int]$zone, [byte]$r, [byte]$g, [byte]$b, [bool]$swap)
    $pkt = New-Object byte[] 51
    $pkt[0] = 0x04
    $pkt[1] = 1
    $pkt[2] = 0x01
    $pkt[3] = [byte]($zone -band 0xFF)
    $pkt[4] = [byte](($zone -shr 8) -band 0xFF)
    if ($swap) {
        $pkt[19] = $g; $pkt[20] = $r; $pkt[21] = $b
    } else {
        $pkt[19] = $r; $pkt[20] = $g; $pkt[21] = $b
    }
    $pkt[22] = 0xFF
    return $pkt
}

$VID = 0x0B05
$TargetPID = 0x19B6
$paths = [HidSend]::EnumeratePaths($VID, $TargetPID)
$targetPath = $paths | Where-Object { $_ -match "mi_01" } | Select-Object -First 1

# First, force everything else off in one packet (all 16 zones, black,
# except we still need 0x09 and 0x0A -- set those off too for now)
$allZones = @(0x00,0x01,0x02,0x03,0x04,0x05,0x06,0x07,0x08,0x09,0x0A,0x0B,0x0C,0x0D,0x0E,0x0F)
$pkt = New-Object byte[] 51
$pkt[0]=0x04; $pkt[1]=8; $pkt[2]=0x01
for ($i=0;$i -lt 8;$i++){ $pkt[3+$i*2]=$allZones[$i]; $pkt[4+$i*2]=0; $pkt[19+$i*4+3]=0xFF }
[HidSend]::TrySetFeature($targetPath, $pkt) | Out-Null
$pkt2 = New-Object byte[] 51
$pkt2[0]=0x04; $pkt2[1]=8; $pkt2[2]=0x01
for ($i=0;$i -lt 8;$i++){ $pkt2[3+$i*2]=$allZones[8+$i]; $pkt2[4+$i*2]=0; $pkt2[19+$i*4+3]=0xFF }
[HidSend]::TrySetFeature($targetPath, $pkt2) | Out-Null

Write-Output "Cleared. Now sending left_bar_front (0x09, swap) in its OWN packet..."
$p1 = Build-Single-Zone-Packet -zone 0x09 -r 255 -g 0 -b 0 -swap $true
$ok1 = [HidSend]::TrySetFeature($targetPath, $p1)
Write-Output "  -> $(if ($ok1) {'SUCCESS'} else {'FAILED'})"

Write-Output "Now sending right_bar_front (0x0A, no-swap) in its OWN separate packet..."
$p2 = Build-Single-Zone-Packet -zone 0x0A -r 255 -g 0 -b 0 -swap $false
$ok2 = [HidSend]::TrySetFeature($targetPath, $p2)
Write-Output "  -> $(if ($ok2) {'SUCCESS'} else {'FAILED'})"

Write-Output ""
Write-Output "Both sent as TWO SEPARATE HidD_SetFeature calls (not combined in one packet)."
Write-Output "left_bar_front should be RED, right_bar_front should be RED."
