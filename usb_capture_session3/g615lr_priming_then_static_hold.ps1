<#
.SYNOPSIS
    Windows session 1 (this repo) controlled test, answering QUESTIONS.md
    Q1 + Q2 in one run:
      Q1: real latency from end of priming to a VISIBLE colour change.
      Q2: does one UNCHANGING zone/colour streamed continuously (matching
          every Linux test so far) also fail on Windows, or only
          varying/cycling zones work?

    Bypasses Armoury Crate's GUI entirely -- sends the exact byte-verified
    priming sequence (re-extracted directly from aura.pcap via tshark, see
    HANDOFF.md "Windows session 1") via HidSend.cs, then streams ONE static
    zone (0x06 -- back_corner_right, per the corrected ground-truth zone
    map in aura_core.ps1; this script originally mislabeled it back-left
    before the correction, see HANDOFF.md "Windows session 3" -- bright
    red, same zone/colour choice as Linux's own g615lr-prime-then-stream.rs
    for direct comparability) for a long duration so "8 seconds wasn't long
    enough" can be ruled out outright.

    WATCH THE BACK-RIGHT CORNER OF THE CHASSIS while this runs. It prints
    an elapsed-seconds marker on every send -- note the marker nearest to
    when (if ever) the corner visibly turns red, and report that back.

.EXAMPLE
    .\g615lr_priming_then_static_hold.ps1 -DurationSec 90
#>
param(
    [int]$DurationSec = 90,
    [int]$IntervalMs = 250
)

Add-Type -Path "C:\Users\Krushna\asusctl-wintest\usb_capture\HidSend.cs"

$VID = 0x0B05
$TargetPID = 0x19B6

$paths = [HidSend]::EnumeratePaths($VID, $TargetPID)
$mi00 = $paths | Where-Object { $_ -match "mi_00&col04" } | Select-Object -First 1
$mi01 = $paths | Where-Object { $_ -match "mi_01" } | Select-Object -First 1

if (-not $mi00) { Write-Output "FATAL: mi_00&col04 path not found"; exit 1 }
if (-not $mi01) { Write-Output "FATAL: mi_01 path not found"; exit 1 }
Write-Output "iface0 (0x5d/0x0201) path: $mi00"
Write-Output "iface1 (0x0305/0x0304) path: $mi01"

$sw = [System.Diagnostics.Stopwatch]::StartNew()
function Log($msg) {
    $t = $sw.Elapsed.TotalSeconds
    Write-Output ("[{0,6:N3}s] {1}" -f $t, $msg)
}

# --- Priming sequence, byte-perfect from aura.pcap (see HANDOFF.md) -------

# 0x0201: Output, ReportID 1, 2 bytes: 01 01
$pkt0201 = [byte[]](0x01, 0x01)
$ok = [HidSend]::TrySetOutputReport($mi00, $pkt0201)
Log "0x0201 (wake) -> $(if ($ok) {'OK'} else {"FAIL err=$([HidSend]::LastError())"})"

# 0x5d b3: Output, ReportID 0x5d, 64 bytes: 5d b3 00 02 00 00 00 eb 00...
$b3 = New-Object byte[] 64
$b3[0]=0x5d; $b3[1]=0xb3; $b3[2]=0x00; $b3[3]=0x02; $b3[4]=0x00; $b3[5]=0x00; $b3[6]=0x00; $b3[7]=0xeb
$ok = [HidSend]::TrySetOutputReport($mi00, $b3)
Log "0x5d b3 (priming) -> $(if ($ok) {'OK'} else {"FAIL err=$([HidSend]::LastError())"})"

# 0x5d b4: Output, ReportID 0x5d, 64 bytes: 5d b4 00...
$b4 = New-Object byte[] 64
$b4[0]=0x5d; $b4[1]=0xb4
$ok = [HidSend]::TrySetOutputReport($mi00, $b4)
Log "0x5d b4 (priming) -> $(if ($ok) {'OK'} else {"FAIL err=$([HidSend]::LastError())"})"

# 0x5d b5: Output, ReportID 0x5d, 64 bytes: 5d b5 00...
$b5 = New-Object byte[] 64
$b5[0]=0x5d; $b5[1]=0xb5
$ok = [HidSend]::TrySetOutputReport($mi00, $b5)
Log "0x5d b5 (priming) -> $(if ($ok) {'OK'} else {"FAIL err=$([HidSend]::LastError())"})"

# 0x0305: Feature, ReportID 5, 10 bytes: 05 00 08 00 0f 00 00 00 00 01
$pkt0305 = [byte[]](0x05,0x00,0x08,0x00,0x0f,0x00,0x00,0x00,0x00,0x01)
$ok = [HidSend]::TrySetFeature($mi01, $pkt0305)
Log "0x0305 (handshake) -> $(if ($ok) {'OK'} else {"FAIL err=$([HidSend]::LastError())"})"

Log "PRIMING COMPLETE -- starting static single-zone hold now"

# --- Static single-zone hold: 0x06 (back_corner_right), bright red, no swap -

$handle = [HidSend]::OpenPersistent($mi01)
if ($handle.IsInvalid) { Write-Output "FATAL: could not open persistent handle to $mi01"; exit 1 }

function Build-SingleZonePacket {
    param([int]$zone, [byte]$r, [byte]$g, [byte]$b)
    $pkt = New-Object byte[] 51
    $pkt[0] = 0x04; $pkt[1] = 1; $pkt[2] = 0x01
    $pkt[3] = [byte]($zone -band 0xFF)
    $pkt[4] = [byte](($zone -shr 8) -band 0xFF)
    # zone 0x06 (back_corner_right) is a confirmed NO-SWAP zone -- plain RGB
    $pkt[19] = $r; $pkt[20] = $g; $pkt[21] = $b
    $pkt[22] = 0xFF
    return $pkt
}

$packet = Build-SingleZonePacket -zone 0x06 -r 255 -g 0 -b 0
$endTime = $sw.Elapsed.TotalSeconds + $DurationSec
$sendCount = 0
$lastLogSec = -1

while ($sw.Elapsed.TotalSeconds -lt $endTime) {
    [HidSend]::SetFeatureOnHandle($handle, $packet) | Out-Null
    $sendCount++
    $curSec = [int]$sw.Elapsed.TotalSeconds
    if ($curSec -ne $lastLogSec) {
        Log "streaming zone 0x06 = FF0000, send #$sendCount"
        $lastLogSec = $curSec
    }
    Start-Sleep -Milliseconds $IntervalMs
}

$handle.Close()
Log "DONE -- $DurationSec seconds elapsed, $sendCount packets sent. Report the elapsed-seconds marker closest to when the corner visibly changed (or 'never' if it stayed red the whole EC-default baseline / never turned red at all)."
