Add-Type -Path "C:\Users\Krushna\AppData\Local\Temp\claude\C--Users-Krushna-claude\adbed461-e0a3-4f3c-84e7-623be742f445\scratchpad\usb_capture\HidSend.cs"

function Build-Packet {
    param([array]$ZoneColors)  # array of @{zone=int; r=int; g=int; b=int}
    $pkt = New-Object byte[] 51
    $pkt[0] = 0x04
    $pkt[1] = [byte]$ZoneColors.Count
    $pkt[2] = 0x01
    for ($i = 0; $i -lt $ZoneColors.Count; $i++) {
        $zc = $ZoneColors[$i]
        $zoff = 3 + $i * 2
        $pkt[$zoff] = [byte]($zc.zone -band 0xFF)
        $pkt[$zoff + 1] = [byte](($zc.zone -shr 8) -band 0xFF)
        $coff = 19 + $i * 4
        $pkt[$coff] = [byte]$zc.g       # order: G, R, B, A
        $pkt[$coff + 1] = [byte]$zc.r
        $pkt[$coff + 2] = [byte]$zc.b
        $pkt[$coff + 3] = 0xFF
    }
    return $pkt
}

$VID = 0x0B05
$TargetPID = 0x19B6

Write-Output "Enumerating HID interfaces for $($VID.ToString('X4')):$($TargetPID.ToString('X4'))..."
$paths = [HidSend]::EnumeratePaths($VID, $TargetPID)
Write-Output "Found $($paths.Count) matching interfaces:"
$paths | ForEach-Object { Write-Output "  $_" }

# Only the 4 sidebar segments lit (R/G/Y/B), everything else forced OFF (black) --
# unambiguous test: only left/right sidebars (back+front) should be lit.
$batch1 = @(
    @{zone=0x08; r=255; g=0;   b=0},    # L sidebar back = red
    @{zone=0x09; r=0;   g=255; b=0},    # L sidebar front = green
    @{zone=0x0A; r=255; g=255; b=0},    # R sidebar front = yellow
    @{zone=0x0B; r=0;   g=0;   b=255},  # R sidebar back = blue
    @{zone=0x00; r=0; g=0; b=0},        # kbd1 off
    @{zone=0x01; r=0; g=0; b=0},        # kbd2 off
    @{zone=0x02; r=0; g=0; b=0},        # kbd3 off
    @{zone=0x03; r=0; g=0; b=0}         # kbd4 off
)
$batch2 = @(
    @{zone=0x06; r=0; g=0; b=0},        # corner TL off
    @{zone=0x07; r=0; g=0; b=0},        # corner TR off
    @{zone=0x0C; r=0; g=0; b=0},        # corner BR off
    @{zone=0x0D; r=0; g=0; b=0},        # corner BL off
    @{zone=0x04; r=0; g=0; b=0},        # back bar left off
    @{zone=0x05; r=0; g=0; b=0},        # back bar right off
    @{zone=0x0E; r=0; g=0; b=0},        # front bar (0x0E) off
    @{zone=0x0F; r=0; g=0; b=0}         # front bar (0x0F) off
)

$targetPath = $paths | Where-Object { $_ -match "mi_01" } | Select-Object -First 1
if (-not $targetPath) {
    Write-Output "Could not find MI_01 interface, aborting"
    exit 1
}
Write-Output "Using: $targetPath"

foreach ($batch in @($batch1, $batch2)) {
    $packet = Build-Packet -ZoneColors $batch
    Write-Output "Packet hex: $(($packet | ForEach-Object { $_.ToString('X2') }) -join ' ')"
    $ok = [HidSend]::TrySetFeature($targetPath, $packet)
    Write-Output "  -> $(if ($ok) { 'SUCCESS' } else { "failed (err=$([HidSend]::LastError()))" })"
}
