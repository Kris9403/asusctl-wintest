<#
.SYNOPSIS
    India tricolor Aura layout for ROG Strix G16 2025 / G615LR:
      - Back bar + back corners:  Saffron (#FF9933)
      - Sidebars + kbd1/kbd4:     White (#FFFFFF)
      - kbd2/kbd3 (the "wheel"):  Smooth breathing blue (#0000FF), representing the Ashoka Chakra
      - Front bar + front corners: Green (#138808)

    Zone maps and packet-building logic live in aura_core.ps1 (shared with
    aura_control.ps1 and aura_animate.ps1) -- edit there, not here.

    Static zones are sent once at startup (the hardware holds each zone's
    color until explicitly changed -- see README.md), then only kbd2/kbd3
    are re-sent per frame for the breathing animation. Re-sending all zones
    every frame was the cause of a flicker observed on kbd1 in earlier
    testing; this fixes it and cuts USB traffic substantially.

    Runs until Ctrl+C.
#>
param([int]$Fps = 25)

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
. (Join-Path $scriptDir "aura_core.ps1")

# --- Static India layout (everything except kbd2/kbd3) ---------------------
$saffron = ConvertFrom-HexColor "FF9933"
$white   = ConvertFrom-HexColor "FFFFFF"
$green   = ConvertFrom-HexColor "138808"
$chakraTarget = ConvertFrom-HexColor "0000FF"

$staticZones = @{}
foreach ($z in @("back_corner_left","back_corner_right","back_left","back_right")) { $staticZones[(Resolve-PhysicalZone $z)] = $saffron }
foreach ($z in @("left_bar_front","left_bar_back","right_bar_front","right_bar_back","kbd1","kbd4")) { $staticZones[(Resolve-PhysicalZone $z)] = $white }
foreach ($z in @("front_corner_left","front_corner_right","front_left","front_right")) { $staticZones[(Resolve-PhysicalZone $z)] = $green }

$chakraZones = @((Resolve-PhysicalZone "kbd2"), (Resolve-PhysicalZone "kbd3"))

# --- Device setup ------------------------------------------------------
try {
    $targetPath = Get-AuraDevicePath
} catch {
    Write-Output $_.Exception.Message
    exit 1
}
$handle = [HidSend]::OpenPersistent($targetPath)
if ($handle.IsInvalid) { Write-Output "Failed to open device"; exit 1 }

$frameDelayMs = [Math]::Max(10, [int](1000 / $Fps))
$tick = 0

# Send static zones ONCE -- hardware holds zone state until explicitly
# changed, so re-sending all 12 every frame alongside the animated pair was
# pure wasted USB traffic and the likely cause of the kbd1 flicker observed
# in live testing. Only the chakra zones need to be in the per-frame loop.
foreach ($zid in $staticZones.Keys) {
    $c = $staticZones[$zid]
    [HidSend]::SetFeatureOnHandle($handle, (Build-AuraPacket -Zone $zid -R $c.r -G $c.g -B $c.b)) | Out-Null
}

Write-Output "India layout running (Saffron/White/Green + breathing blue wheel on kbd2/kbd3) -- Ctrl+C to stop"

try {
    while ($true) {
        # Breathing blue on the wheel zones (sine, ~4s period at 25fps/100 ticks)
        $phase = ($tick % 100) / 100.0
        $level = (1 + [Math]::Sin($phase * 2 * [Math]::PI - [Math]::PI/2)) / 2.0
        $r = [byte]([Math]::Round($chakraTarget.r * $level))
        $g = [byte]([Math]::Round($chakraTarget.g * $level))
        $b = [byte]([Math]::Round($chakraTarget.b * $level))
        foreach ($zid in $chakraZones) {
            [HidSend]::SetFeatureOnHandle($handle, (Build-AuraPacket -Zone $zid -R $r -G $g -B $b)) | Out-Null
        }

        $tick++
        Start-Sleep -Milliseconds $frameDelayMs
    }
} finally {
    $handle.Close()
}
