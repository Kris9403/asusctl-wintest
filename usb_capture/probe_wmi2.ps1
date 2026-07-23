$inst = Get-CimInstance -Namespace root\wmi -ClassName AsusAtkWmi_WMNB
Write-Output "Instance found: $($null -ne $inst)"

$ids = [ordered]@{
  "WIRELESS_LED (0x00010002)"    = [uint32]0x00010002
  "WLAN_LED (0x00010012)"        = [uint32]0x00010012
  "LED1 (0x00020011)"            = [uint32]0x00020011
  "LED2 (0x00020012)"            = [uint32]0x00020012
  "LED3 (0x00020013)"            = [uint32]0x00020013
  "LED4 (0x00020014)"            = [uint32]0x00020014
  "LED5 (0x00020015)"            = [uint32]0x00020015
  "LED6 (0x00020016)"            = [uint32]0x00020016
  "MICMUTE_LED (0x00040017)"     = [uint32]0x00040017
  "CAMERA_LED_NEG (0x00060078)"  = [uint32]0x00060078
  "CAMERA_LED (0x00060079)"      = [uint32]0x00060079
  "TOUCHPAD_LED (0x00100012)"    = [uint32]0x00100012
  "KBD_BACKLIGHT (0x00050021)"   = [uint32]0x00050021
  "LIGHTBAR (0x00050025)"        = [uint32]0x00050025
  "SCREENPAD_LIGHT (0x00050032)" = [uint32]0x00050032
  "TUF_RGB_MODE (0x00100056)"    = [uint32]0x00100056
  "TUF_RGB_MODE2 (0x0010005A)"   = [uint32]0x0010005A
  "TUF_RGB_STATE (0x00100057)"   = [uint32]0x00100057
}
foreach ($k in $ids.Keys) {
  try {
    $r = Invoke-CimMethod -InputObject $inst -MethodName DSTS -Arguments @{Device_ID=$ids[$k]} -ErrorAction Stop
    Write-Output "$k -> device_status = 0x$('{0:X8}' -f $r.device_status)"
  } catch {
    Write-Output "$k -> ERROR: $($_.Exception.Message)"
  }
}
