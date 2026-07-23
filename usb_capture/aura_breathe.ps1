<#
.SYNOPSIS
    Software-driven Breathing effect for ROG Strix G16 2025 / G615LR Aura
    lighting, since the hardware built-in Breathe mode does not work on this
    laptop (confirmed -- see README.md). This repeatedly resends the 0x04
    per-zone static-color protocol with a computed color, fading between
    black and your target color(s) -- same triangle-wave math as asusctl's
    own software Breathe effect (rog-aura/src/effects/breathe.rs), just
    driven from PowerShell instead of the asusd daemon.

    Runs until you press Ctrl+C.

.EXAMPLE
    # Breathe the whole keyboard between black and red
    .\aura_breathe.ps1 -Zone kbd1,kbd2,kbd3,kbd4 -Color1 FF0000

.EXAMPLE
    # Breathe alternating between red and blue, faster
    .\aura_breathe.ps1 -Zone back_corner_left,back_corner_right -Color1 FF0000 -Color2 0000FF -Speed 3
#>
param(
    [Parameter(Mandatory=$true)][string[]]$Zone,
    [Parameter(Mandatory=$true)][string]$Color1,
    [string]$Color2 = "000000",
    [int]$Speed = 2,          # 1 (slow) - 4 (fast)
    [int]$FrameDelayMs = 40
)

Add-Type -Path "C:\Users\Krushna\AppData\Local\Temp\claude\C--Users-Krushna-claude\adbed461-e0a3-4f3c-84e7-623be742f445\scratchpad\usb_capture\HidSend.cs"

$INTERNAL_ZONES = [ordered]@{
    kbd1 = 0x00; kbd2 = 0x01; kbd3 = 0x02; kbd4 = 0x03
    back_bar_left = 0x04; back_bar_right = 0x05
    corner_tl = 0x06; corner_tr = 0x07
    left_sidebar_back = 0x08; left_sidebar_front = 0x09
    right_sidebar_front = 0x0A; right_sidebar_back = 0x0B
    corner_br = 0x0C; corner_bl = 0x0D
    front_bar_right = 0x0E; front_bar_left = 0x0F
}
$PHYSICAL_MAP = [ordered]@{
    left_bar_front  = "left_sidebar_front"
    left_bar_back   = "right_sidebar_back"
    right_bar_front = "right_sidebar_front"
    right_bar_back  = "left_sidebar_back"
    front_left  = "front_bar_left"
    front_right = "front_bar_right"
    back_left   = "back_bar_left"
    back_right  = "back_bar_right"
    back_corner_left   = "corner_tl"
    back_corner_right  = "corner_tr"
    front_corner_left  = "corner_bl"
    front_corner_right = "corner_br"
    kbd1 = "kbd1"; kbd2 = "kbd2"; kbd3 = "kbd3"; kbd4 = "kbd4"
}
$NO_SWAP_ZONES = @(0x00, 0x01, 0x02, 0x03, 0x0E, 0x0F, 0x08, 0x0A, 0x0C, 0x0D)

function Build-Packet {
    param([int]$zone, [byte]$r, [byte]$g, [byte]$b)
    $pkt = New-Object byte[] 51
    $pkt[0] = 0x04; $pkt[1] = 1; $pkt[2] = 0x01
    $pkt[3] = [byte]($zone -band 0xFF)
    $pkt[4] = [byte](($zone -shr 8) -band 0xFF)
    if ($NO_SWAP_ZONES -contains $zone) {
        $pkt[19] = $r; $pkt[20] = $g; $pkt[21] = $b
    } else {
        $pkt[19] = $g; $pkt[20] = $r; $pkt[21] = $b
    }
    $pkt[22] = 0xFF
    return $pkt
}

$paths = [HidSend]::EnumeratePaths(0x0B05, 0x19B6)
$targetPath = $paths | Where-Object { $_ -match "mi_01" } | Select-Object -First 1
if (-not $targetPath) { Write-Output "MI_01 interface not found"; exit 1 }

$zoneIds = @()
foreach ($z in $Zone) {
    if (-not $PHYSICAL_MAP.Contains($z)) { Write-Output "Unknown zone: $z"; exit 1 }
    $zoneIds += $INTERNAL_ZONES[$PHYSICAL_MAP[$z]]
}

$c1 = @{ r = [Convert]::ToInt32($Color1.Substring(0,2),16); g = [Convert]::ToInt32($Color1.Substring(2,2),16); b = [Convert]::ToInt32($Color1.Substring(4,2),16) }
$c2 = @{ r = [Convert]::ToInt32($Color2.Substring(0,2),16); g = [Convert]::ToInt32($Color2.Substring(2,2),16); b = [Convert]::ToInt32($Color2.Substring(4,2),16) }

$speedDiv = [Math]::Max(1, 4 - $Speed)
$current = @{ r = [double]$c1.r; g = [double]$c1.g; b = [double]$c1.b }
$flipped = $false
$useColour1 = $true

Write-Output "Breathing $($Zone -join ', ') -- Ctrl+C to stop"

while ($true) {
    if ($current.r -eq 0 -and $current.g -eq 0 -and $current.b -eq 0) {
        $useColour1 = -not $useColour1
    }
    $target = if ($useColour1) { $c1 } else { $c2 }

    $rScale = $target.r / $speedDiv / 2
    $gScale = $target.g / $speedDiv / 2
    $bScale = $target.b / $speedDiv / 2
    if ($rScale -lt 0.5 -and $target.r -gt 0) { $rScale = 0.5 }
    if ($gScale -lt 0.5 -and $target.g -gt 0) { $gScale = 0.5 }
    if ($bScale -lt 0.5 -and $target.b -gt 0) { $bScale = 0.5 }

    if ($current.r -eq 0 -and $current.g -eq 0 -and $current.b -eq 0) {
        $flipped = $true
    } elseif ($current.r -ge $target.r -and $current.g -ge $target.g -and $current.b -ge $target.b) {
        $flipped = $false
    }

    if (-not $flipped) {
        $current.r = [Math]::Max(0, $current.r - $rScale)
        $current.g = [Math]::Max(0, $current.g - $gScale)
        $current.b = [Math]::Max(0, $current.b - $bScale)
    } else {
        $current.r = [Math]::Min(255, $current.r + $rScale)
        $current.g = [Math]::Min(255, $current.g + $gScale)
        $current.b = [Math]::Min(255, $current.b + $bScale)
    }

    $r = [byte][Math]::Round($current.r)
    $g = [byte][Math]::Round($current.g)
    $b = [byte][Math]::Round($current.b)

    foreach ($zid in $zoneIds) {
        $packet = Build-Packet -zone $zid -r $r -g $g -b $b
        [HidSend]::TrySetFeature($targetPath, $packet) | Out-Null
    }

    Start-Sleep -Milliseconds $FrameDelayMs
}
