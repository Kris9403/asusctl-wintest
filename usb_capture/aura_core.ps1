<#
.SYNOPSIS
    Shared core for ROG Strix G16 2025 / G615LR Aura lighting control.
    Dot-source this from control/animation scripts instead of duplicating
    zone maps and packet-building logic:

        $scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
        . (Join-Path $scriptDir "aura_core.ps1")

    Loads HidSend.cs relative to its own location, so this whole folder can
    be moved anywhere (or copied out of the temp scratchpad) without editing
    any paths.

    ZONE MAP CORRECTED 2026-07-23 (Windows session 3) against
    `usb_capture_session3/ground_truth/WDL_G615LR.csv` -- the OFFICIAL ASUS
    Aura Creator device-layout file for this exact laptop (grid + physical
    x/y/z coordinates + lamp_id, pulled straight from Aura Creator's own
    per-device profile, not re-derived empirically). Confirmed by a live
    controlled test the same session: sent wire zone 0x06, the physically
    WRONG corner lit up under the previous map, exactly matching what this
    ground-truth file said 0x06 actually is. The back-edge zones (0x04-0x07)
    and the left sidebar's front/back split (0x09/0x0B) were wrong in every
    previous version of this map -- likely the real explanation for a chunk
    of this whole project's long-standing "flip-flop" zone/colour
    instability, not just noise. Keyboard (0x00-0x03), 0x08, 0x0A, and the
    front edge (0x0C-0x0F) all checked out already correct. See HANDOFF.md
    "Windows session 3" for the full derivation.

    Previously this file used a confusing two-hop indirection (physical
    name -> "internal" name -> wire ID) to keep empirically-confirmed wire
    IDs separate from physical-position assumptions. Now that we have an
    authoritative ground-truth source instead of empirical guessing, that
    indirection is gone -- $PHYSICAL_ZONES below maps physical name straight
    to wire ID.
#>

$script:AuraCoreDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Add-Type -Path (Join-Path $script:AuraCoreDir "HidSend.cs")

$AURA_VID = 0x0B05
$AURA_PID = 0x19B6

# Physical location (what you actually address, standing in front of the
# laptop) -> raw USB wire zone ID. Source of truth:
# usb_capture_session3/ground_truth/WDL_G615LR.csv (ASUS's own Aura Creator
# device profile for this exact laptop model).
$PHYSICAL_ZONES = [ordered]@{
    kbd1 = 0x00; kbd2 = 0x01; kbd3 = 0x02; kbd4 = 0x03

    back_left  = 0x05   # was wrongly 0x04 in every prior version
    back_right = 0x04   # was wrongly 0x05 in every prior version
    back_corner_left  = 0x07   # was wrongly 0x06 in every prior version
    back_corner_right = 0x06   # was wrongly 0x07 in every prior version

    right_bar_back  = 0x08
    left_bar_back   = 0x09   # was wrongly "left_bar_front" in every prior version
    right_bar_front = 0x0A
    left_bar_front  = 0x0B   # was wrongly "left_bar_back" in every prior version

    front_corner_right = 0x0C
    front_corner_left  = 0x0D
    front_bar_right = 0x0E
    front_bar_left  = 0x0F
}

# Back-compat alias -- old scripts/callers may still reference $PHYSICAL_MAP.
$PHYSICAL_MAP = $PHYSICAL_ZONES

# Zones that do NOT use the G/R channel swap -- take plain RGB directly.
# Single source of truth now; previously duplicated (and had drifted out of
# sync) across aura_control.ps1, aura_animate.ps1, and aura_india.ps1.
# LATEST finding: all 16 zones test as no-swap. See README.md's "Known
# instability" section before trusting this for anything that matters --
# this flip-flopped once already for the back bar/back corners.
$NO_SWAP_ZONES = @(0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x0A, 0x0C, 0x0D, 0x0E, 0x0F)

# Finds the MI_01 HID interface path for the Aura device. Throws if not found.
function Get-AuraDevicePath {
    $paths = [HidSend]::EnumeratePaths($AURA_VID, $AURA_PID)
    $targetPath = $paths | Where-Object { $_ -match "mi_01" } | Select-Object -First 1
    if (-not $targetPath) { throw "MI_01 interface not found" }
    return $targetPath
}

function ConvertFrom-HexColor {
    param([Parameter(Mandatory)][string]$Hex)
    return @{
        r = [Convert]::ToInt32($Hex.Substring(0,2),16)
        g = [Convert]::ToInt32($Hex.Substring(2,2),16)
        b = [Convert]::ToInt32($Hex.Substring(4,2),16)
    }
}

# Physical name -> raw wire zone ID. Throws on unknown name.
function Resolve-PhysicalZone {
    param([Parameter(Mandatory)][string]$PhysicalName)
    if (-not $PHYSICAL_ZONES.Contains($PhysicalName)) {
        throw "Unknown physical zone: $PhysicalName"
    }
    return $PHYSICAL_ZONES[$PhysicalName]
}

# Builds the 51-byte HID Feature report payload for a single zone/color pair.
# (Batching multiple zones in one packet is untested since the swap-table
# correction -- see README.md -- so every script here sends one zone per
# packet, which is confirmed reliable.)
function Build-AuraPacket {
    param([int]$Zone, [byte]$R, [byte]$G, [byte]$B)
    $pkt = New-Object byte[] 51
    $pkt[0] = 0x04; $pkt[1] = 1; $pkt[2] = 0x01
    $pkt[3] = [byte]($Zone -band 0xFF)
    $pkt[4] = [byte](($Zone -shr 8) -band 0xFF)
    if ($NO_SWAP_ZONES -contains $Zone) {
        $pkt[19] = $R; $pkt[20] = $G; $pkt[21] = $B
    } else {
        $pkt[19] = $G; $pkt[20] = $R; $pkt[21] = $B
    }
    $pkt[22] = 0xFF
    return $pkt
}

# Standard HSV -> RGB conversion. h in [0,360), s/v in [0,1].
function ConvertFrom-Hsv {
    param([double]$H, [double]$S, [double]$V)
    $c = $V * $S
    $x = $c * (1 - [Math]::Abs((($H / 60.0) % 2) - 1))
    $m = $V - $c
    $r=0; $g=0; $b=0
    if     ($H -lt 60)  { $r=$c; $g=$x; $b=0 }
    elseif ($H -lt 120) { $r=$x; $g=$c; $b=0 }
    elseif ($H -lt 180) { $r=0;  $g=$c; $b=$x }
    elseif ($H -lt 240) { $r=0;  $g=$x; $b=$c }
    elseif ($H -lt 300) { $r=$x; $g=0;  $b=$c }
    else                { $r=$c; $g=0;  $b=$x }
    return @{
        r = [byte][Math]::Round(($r + $m) * 255)
        g = [byte][Math]::Round(($g + $m) * 255)
        b = [byte][Math]::Round(($b + $m) * 255)
    }
}
