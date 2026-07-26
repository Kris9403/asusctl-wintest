<#
.SYNOPSIS
    Windows session 8: tests whether re-sending the ReportID=1 Output
    write (0x0201, data 01 01) a SECOND time -- immediately before the
    0x04 stream starts, on top of the one already sent before priming --
    changes anything. The single-invocation form (before priming only)
    is already known to fail; see HANDOFF.md Windows session 8.

    WATCH kbd3 (third keyboard key-zone key, wire ID 0x02) for any
    visible effect once streaming starts.
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

# --- First ReportID=1 Output write, before priming (matches every existing script) ---
$pkt0201 = [byte[]](0x01, 0x01)
$ok = [HidSend]::TrySetOutputReport($mi00, $pkt0201)
Log "0x0201 (01 01) #1, before priming -> $(if ($ok) {'OK'} else {"FAIL err=$([HidSend]::LastError())"})"

# --- Priming triplet ---
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

# --- THE NEW BIT: send ReportID=1 Output write AGAIN, right before streaming starts ---
$ok = [HidSend]::TrySetOutputReport($mi00, $pkt0201)
Log "0x0201 (01 01) #2, RIGHT BEFORE 0x04 STREAM -> $(if ($ok) {'OK'} else {"FAIL err=$([HidSend]::LastError())"})"

Log "PRIMING + REPEATED REPORT1 COMPLETE -- starting 0x04 stream now"

# --- Stream kbd3 (0x02), bright green, distinct from any priming/RainbowCycle red/blue ---
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
Log "DONE -- $DurationSec seconds elapsed, $sendCount packets sent to kbd3 (zone 0x02, green). Report whether kbd3 showed ANY visible green effect."
