<#
.SYNOPSIS
    Software-driven Aura animations (Rainbow, Starry Night, Breathing) for
    ROG Strix G16 2025 / G615LR, using the confirmed-working per-zone 0x04
    protocol. This laptop has no firmware animation engine (see README.md /
    docs/g615lr-aura-protocol.md) -- Armoury Crate itself drives every
    "hardware" mode by streaming 0x04 packets from the PC in a loop, which
    is exactly what this script does.

    Structured to mirror asusctl's own effect architecture
    (rog-aura::effects::EffectState -- next_colour_state() / get_colour())
    so the animation logic here can be ported into a real rog-aura effect
    with minimal changes: each effect is a function that takes the current
    tick and a zone index and returns a colour, same shape as the Rust
    trait's next_colour_state()/get_colour() pair.

    Zone maps and packet-building logic live in aura_core.ps1 (shared with
    aura_control.ps1 and aura_india.ps1) -- edit there, not here.

    Runs until Ctrl+C.

.EXAMPLE
    .\aura_animate.ps1 -Effect Rainbow
.EXAMPLE
    .\aura_animate.ps1 -Effect StarryNight -Fps 20
.EXAMPLE
    .\aura_animate.ps1 -Effect Breathe -Color FF0000 -Fps 25
#>
param(
    [ValidateSet("Rainbow", "StarryNight", "Breathe")]
    [string]$Effect = "Rainbow",
    [string]$Color = "FF0000",   # used by Breathe
    [int]$Fps = 20
)

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
. (Join-Path $scriptDir "aura_core.ps1")

$ZONES = @($INTERNAL_ZONES.Values)

# --- Effects -----------------------------------------------------------
# Each effect: next_colour_state(tick, zoneIndex) -> @{r;g;b}
# (mirrors EffectState::next_colour_state / get_colour in rog-aura)

function Effect-Rainbow {
    param([int]$tick, [int]$zoneIndex, [int]$zoneCount)
    $hueStep = 4.0          # degrees per tick -- matches a moderate sweep speed
    $phaseSpread = 360.0 / $zoneCount
    $hue = ($tick * $hueStep + $zoneIndex * $phaseSpread) % 360
    return ConvertFrom-Hsv -H $hue -S 1.0 -V 1.0
}

$script:starState = @{}
function Effect-StarryNight {
    param([int]$tick, [int]$zoneIndex, [int]$zoneCount)
    if (-not $script:starState.ContainsKey($zoneIndex)) {
        $script:starState[$zoneIndex] = 0.0
    }
    $brightness = $script:starState[$zoneIndex]
    # Random chance to spark a new star on a currently-dark zone
    if ($brightness -le 0 -and (Get-Random -Minimum 0.0 -Maximum 1.0) -lt 0.06) {
        $brightness = 255.0
    } else {
        $brightness = [Math]::Max(0, $brightness - 18.0)  # fade out
    }
    $script:starState[$zoneIndex] = $brightness
    $v = [byte]$brightness
    return @{ r = $v; g = $v; b = $v }   # white twinkle
}

function Effect-Breathe {
    param([int]$tick, [int]$zoneIndex, [int]$zoneCount, [hashtable]$target)
    # Simple sine breathing, in phase across all zones
    $phase = ($tick % 100) / 100.0
    $level = (1 + [Math]::Sin($phase * 2 * [Math]::PI - [Math]::PI/2)) / 2.0  # 0..1
    return @{
        r = [byte]([Math]::Round($target.r * $level))
        g = [byte]([Math]::Round($target.g * $level))
        b = [byte]([Math]::Round($target.b * $level))
    }
}

# --- Setup ---------------------------------------------------------------
try {
    $targetPath = Get-AuraDevicePath
} catch {
    Write-Output $_.Exception.Message
    exit 1
}

$handle = [HidSend]::OpenPersistent($targetPath)
if ($handle.IsInvalid) { Write-Output "Failed to open device"; exit 1 }

$breatheTarget = ConvertFrom-HexColor -Hex $Color

$frameDelayMs = [Math]::Max(10, [int](1000 / $Fps))
$tick = 0

Write-Output "Running '$Effect' at ~$Fps fps -- Ctrl+C to stop"

try {
    while ($true) {
        for ($i = 0; $i -lt $ZONES.Count; $i++) {
            $zone = $ZONES[$i]
            $c = switch ($Effect) {
                "Rainbow"     { Effect-Rainbow -tick $tick -zoneIndex $i -zoneCount $ZONES.Count }
                "StarryNight" { Effect-StarryNight -tick $tick -zoneIndex $i -zoneCount $ZONES.Count }
                "Breathe"     { Effect-Breathe -tick $tick -zoneIndex $i -zoneCount $ZONES.Count -target $breatheTarget }
            }
            $packet = Build-AuraPacket -Zone $zone -R $c.r -G $c.g -B $c.b
            [HidSend]::SetFeatureOnHandle($handle, $packet) | Out-Null
        }
        $tick++
        Start-Sleep -Milliseconds $frameDelayMs
    }
} finally {
    $handle.Close()
}
