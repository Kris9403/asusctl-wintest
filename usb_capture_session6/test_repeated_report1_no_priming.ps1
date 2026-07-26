<#
.SYNOPSIS
    Same as test_repeated_report1.ps1 but SKIPS the 0x5d b3/b4/b5 priming
    triplet entirely (that triplet always hardcodes RainbowCycle mode
    regardless of target -- would override a real static baseline the
    user set manually via GUI). Keeps both ReportID=1 Output writes
    (before + right before streaming) and the 0x0305 handshake.

    Run this AFTER manually setting a static colour via the GUI, so the
    baseline is real and unconfounded by RainbowCycle's own colour
    cycling.
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

$pkt0201 = [byte[]](0x01, 0x01)
$ok = [HidSend]::TrySetOutputReport($mi00, $pkt0201)
Log "0x0201 (01 01) #1 -> $(if ($ok) {'OK'} else {"FAIL err=$([HidSend]::LastError())"})"

# NO b3/b4/b5 priming this time -- real static baseline stays untouched.

$pkt0305 = [byte[]](0x05,0x00,0x08,0x00,0x0f,0x00,0x00,0x00,0x00,0x01)
$ok = [HidSend]::TrySetFeature($mi01, $pkt0305)
Log "0x0305 (handshake) -> $(if ($ok) {'OK'} else {"FAIL err=$([HidSend]::LastError())"})"

$ok = [HidSend]::TrySetOutputReport($mi00, $pkt0201)
Log "0x0201 (01 01) #2, right before 0x04 stream -> $(if ($ok) {'OK'} else {"FAIL err=$([HidSend]::LastError())"})"

Log "NO-PRIMING SEQUENCE COMPLETE -- starting 0x04 stream now"

$handle = [HidSend]::OpenPersistent($mi01)
if ($handle.IsInvalid) { Write-Output "FATAL: could not open persistent handle to $mi01"; exit 1 }

function Build-SingleZonePacket {
    param([int]$zone, [byte]$r, [byte]$g, [byte]$b)
    $pkt = New-Object byte[] 51
    $pkt[0] = 0x04; $pkt[1] = 1; $pkt[2] = 0x01
    $pkt[3] = [byte]($zone -band 0xFF)
    $pkt[4] = [byte](($zone -shr 8) -band 0xFF)
    $pkt[19] = $r; $pkt[20] = $g; $pkt[21] = $b
    $pkt[22] = 0xFF
    return $pkt
}

$packet = Build-SingleZonePacket -zone 0x02 -r 0 -g 255 -b 0
$endTime = $sw.Elapsed.TotalSeconds + $DurationSec
$sendCount = 0

while ($sw.Elapsed.TotalSeconds -lt $endTime) {
    [HidSend]::SetFeatureOnHandle($handle, $packet) | Out-Null
    $sendCount++
    Start-Sleep -Milliseconds 30
}

$handle.Close()
Log "DONE -- $DurationSec seconds elapsed, $sendCount packets sent to kbd3 (zone 0x02, green). Report what you saw."
