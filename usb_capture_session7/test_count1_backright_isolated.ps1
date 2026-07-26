<#
.SYNOPSIS
    The critical isolation test after tonight's breakthrough. The
    working test used count=5 (multi-zone) AND zone 0x04 (back_right)
    for the first time simultaneously -- this isolates which variable
    actually mattered by sending count=1, targeting ONLY back_right,
    same priming, same colour, same full alpha.

    If this ALSO lights the lightbar: count>1 never mattered, zone 0x04
    just behaves differently from every other zone tried before.
    If this produces zero effect (the usual result): count>1 (or
    "keyboard zones present in the same packet") is the real missing
    piece.

    WATCH THE BACK-RIGHT LIGHTBAR ZONE.
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

Log "PRIMING COMPLETE -- starting count=1 (back_right ONLY) stream now"

function Build-Count1BackRight {
    $pkt = New-Object byte[] 51
    $pkt[0] = 0x04
    $pkt[1] = 1        # count = 1 -- THE ONLY DIFFERENCE from the working test
    $pkt[2] = 0x01
    $pkt[3] = 0x04     # zone = back_right
    $pkt[4] = 0x00
    # RGBA at offset 19, same colour as the working test: 61 ff 00 ff
    $pkt[19] = 0x61
    $pkt[20] = 0xff
    $pkt[21] = 0x00
    $pkt[22] = 0xff
    return $pkt
}

$packet = Build-Count1BackRight
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
Log "DONE -- $DurationSec seconds elapsed, $sendCount packets sent. Report: did back_right light up yellow-green, same as the multi-zone test?"
