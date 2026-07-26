<#
.SYNOPSIS
    Windows session 9: the first-ever count>1 (multi-zone) 0x04
    reproduction test. Every prior 0x04 test all investigation has been
    count=1 (one zone per packet). This replicates the EXACT byte-for-
    byte packet from static_armory_to_aura_lightbar_only.pcapng's one
    real, human-confirmed-working 0x04 write: 5 zones in one packet
    (kbd1-4 + back_right/lightbar), keyboard zones at alpha~0 (invisible),
    lightbar zone at full alpha with a real colour.

    Streamed continuously (unlike the real capture's one-shot write)
    since we don't have Aura's own 0x0305 stream to hold the state --
    every other reproduction test needed continuous streaming to survive
    RainbowCycle's overwrite, so this does too.

    WATCH THE BACK-RIGHT LIGHTBAR ZONE specifically (not the keyboard).
#>
param(
    [int]$DurationSec = 15
)

Add-Type -Path "C:\Users\Krushna\asusctl-wintest\usb_capture\HidSend.cs"

$VID = 0x0B05
$TargetPID = 0x19B6

$paths = [HidSend]::EnumeratePaths($VID, $TargetPID)
$mi00 = $paths | Where-Object { $_ -match "mi_00&col04" } | Select-Object -First 1
$mi01 = $paths | Where-Object { $_ -match "mi_01" } | Select-Object -First 1

if (-not $mi00) { Write-Output "FATAL: mi_00&col04 path not found"; exit 1 }
if (-not $mi01) { Write-Output "FATAL: mi_01 path not found"; exit 1 }
Write-Output "iface0 path: $mi00"
Write-Output "iface1 path: $mi01"

$sw = [System.Diagnostics.Stopwatch]::StartNew()
function Log($msg) {
    $t = $sw.Elapsed.TotalSeconds
    Write-Output ("[{0,6:N3}s] {1}" -f $t, $msg)
}

# --- Real priming triplet, matching what preceded the real working write ---
$b3 = New-Object byte[] 64
$b3[0]=0x5d; $b3[1]=0xb3; $b3[2]=0x00; $b3[3]=0x02; $b3[4]=0x00; $b3[5]=0x00; $b3[6]=0x00; $b3[7]=0xeb
$ok = [HidSend]::TrySetOutputReport($mi00, $b3)
Log "0x5d b3 (priming) -> $(if ($ok) {'OK'} else {"FAIL err=$([HidSend]::LastError())"})"

$b4 = New-Object byte[] 64
$b4[0]=0x5d; $b4[1]=0xb4
$ok = [HidSend]::TrySetOutputReport($mi00, $b4)
Log "0x5d b4 (priming) -> $(if ($ok) {'OK'} else {"FAIL err=$([HidSend]::LastError())"})"

$b5 = New-Object byte[] 64
$b5[0]=0x5d; $b5[1]=0xb5
$ok = [HidSend]::TrySetOutputReport($mi00, $b5)
Log "0x5d b5 (priming) -> $(if ($ok) {'OK'} else {"FAIL err=$([HidSend]::LastError())"})"

$pkt0305 = [byte[]](0x05,0x00,0x08,0x00,0x0f,0x00,0x00,0x00,0x00,0x01)
$ok = [HidSend]::TrySetFeature($mi01, $pkt0305)
Log "0x0305 (handshake) -> $(if ($ok) {'OK'} else {"FAIL err=$([HidSend]::LastError())"})"

Log "PRIMING COMPLETE -- starting count=5 multi-zone stream now"

# --- Build the EXACT confirmed real packet: count=5, zones kbd1-4 + back_right ---
# Structure (empirically decoded, HANDOFF.md Windows session 9):
#   data[0]=0x04, data[1]=count, data[2]=0x01
#   data[3:19] = zone ID list (u16 LE each), zero-padded to 16 bytes
#   data[19:19+4N] = RGBA blocks (R,G,B,A), same order as zone list
function Build-MultiZonePacket {
    $pkt = New-Object byte[] 51
    $pkt[0] = 0x04
    $pkt[1] = 5      # count
    $pkt[2] = 0x01
    # zone IDs: kbd1=0x00, kbd2=0x01, kbd3=0x02, kbd4=0x03, back_right=0x04
    $zones = @(0x00, 0x01, 0x02, 0x03, 0x04)
    for ($i = 0; $i -lt $zones.Count; $i++) {
        $pkt[3 + 2*$i] = $zones[$i]
        $pkt[4 + 2*$i] = 0x00
    }
    # RGBA blocks starting at offset 19, same order as zones
    # kbd1-4: R,G,B=0, alpha=1 (invisible) -- matches the real capture exactly
    for ($i = 0; $i -lt 4; $i++) {
        $base = 19 + 4*$i
        $pkt[$base]   = 0x00
        $pkt[$base+1] = 0x00
        $pkt[$base+2] = 0x00
        $pkt[$base+3] = 0x01
    }
    # back_right (lightbar, 5th zone): R=0x61,G=0xff,B=0x00, alpha=0xff (full)
    $base = 19 + 4*4
    $pkt[$base]   = 0x61
    $pkt[$base+1] = 0xff
    $pkt[$base+2] = 0x00
    $pkt[$base+3] = 0xff
    return $pkt
}

$packet = Build-MultiZonePacket
Log "Packet bytes: $(($packet | ForEach-Object { $_.ToString('x2') }) -join ' ')"

$handle = [HidSend]::OpenPersistent($mi01)
if ($handle.IsInvalid) { Write-Output "FATAL: could not open persistent handle to $mi01"; exit 1 }

$endTime = $sw.Elapsed.TotalSeconds + $DurationSec
$sendCount = 0
while ($sw.Elapsed.TotalSeconds -lt $endTime) {
    [HidSend]::SetFeatureOnHandle($handle, $packet) | Out-Null
    $sendCount++
    Start-Sleep -Milliseconds 30
}

$handle.Close()
Log "DONE -- $DurationSec seconds elapsed, $sendCount packets sent. Report: did back_right (lightbar) show the yellow-green colour, with keyboard staying off/unchanged?"
