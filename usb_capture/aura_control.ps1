<#
.SYNOPSIS
    Direct control of ROG Strix G16 2025 / G615LR Aura lighting via Windows HID API.
    Lights ONLY the zones you specify; every other zone is forced off, so
    there's never any ambiguity about what's lit.

    You address zones by their PHYSICAL location, sitting in front of the
    laptop (e.g. "back_corner_left", "left_bar_front") -- the script
    translates that through the hardware's cross-wiring to the correct
    internal zone code, and applies the correct GRB/RGB color-channel fix
    per zone, automatically. You always give colors as normal RGB hex
    (e.g. FF0000 = red).

    Zone maps and packet-building logic live in aura_core.ps1 (shared with
    aura_animate.ps1 and aura_india.ps1) -- edit there, not here.

.EXAMPLE
    .\aura_control.ps1 -Zone back_corner_left -Color FF0000

.EXAMPLE
    .\aura_control.ps1 -Zone back_corner_left,back_corner_right,front_corner_right,front_corner_left -Color FF0000,00FF00,FFFF00,0000FF

.EXAMPLE
    .\aura_control.ps1 -List
#>
param(
    [string[]]$Zone,
    [string[]]$Color,
    [switch]$List
)

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
. (Join-Path $scriptDir "aura_core.ps1")

if ($List) {
    $PHYSICAL_ZONES.GetEnumerator() | ForEach-Object {
        Write-Output ("{0,-20} -> 0x{1:X2}" -f $_.Key, $_.Value)
    }
    exit 0
}

if (-not $Zone -or -not $Color -or $Zone.Count -ne $Color.Count) {
    Write-Output "Usage: .\aura_control.ps1 -Zone <physical_name>[,...] -Color <RRGGBB>[,...]"
    Write-Output "       .\aura_control.ps1 -List    (show all physical zone names)"
    exit 1
}

try {
    $targetPath = Get-AuraDevicePath
} catch {
    Write-Output $_.Exception.Message
    exit 1
}

$requested = @{}
for ($i = 0; $i -lt $Zone.Count; $i++) {
    try {
        $hexId = Resolve-PhysicalZone -PhysicalName $Zone[$i]
    } catch {
        Write-Output "Unknown physical zone: $($Zone[$i])  (use -List to see valid names)"
        exit 1
    }
    $requested[$hexId] = ConvertFrom-HexColor -Hex $Color[$i]
}

# Send ONE zone per packet (safe, avoids any cross-zone packet issues)
foreach ($zid in $PHYSICAL_ZONES.Values) {
    $c = if ($requested.Contains($zid)) { $requested[$zid] } else { @{r=0;g=0;b=0} }
    $packet = Build-AuraPacket -Zone $zid -R $c.r -G $c.g -B $c.b
    [HidSend]::TrySetFeature($targetPath, $packet) | Out-Null
}

Write-Output ""
Write-Output "Lit zones:"
for ($i = 0; $i -lt $Zone.Count; $i++) {
    $hexId = $PHYSICAL_ZONES[$Zone[$i]]
    Write-Output "  $($Zone[$i]) (0x$('{0:X2}' -f $hexId)) = $($Color[$i])"
}
