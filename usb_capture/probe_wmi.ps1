$inst = Get-CimInstance -Namespace root\wmi -ClassName AsusAtkWmi_WMNB
Write-Output "Instance found: $($null -ne $inst)"

$ids = @{
  "TUF_RGB_MODE (0x00100056)"  = [uint32]0x00100056
  "TUF_RGB_STATE (0x00100057)" = [uint32]0x00100057
  "KBD_BACKLIGHT (0x00050021)" = [uint32]0x00050021
  "LIGHTBAR (0x00050025)"      = [uint32]0x00050025
}
foreach ($k in $ids.Keys) {
  try {
    $r = Invoke-CimMethod -InputObject $inst -MethodName DSTS -Arguments @{Device_ID=$ids[$k]} -ErrorAction Stop
    Write-Output "$k -> device_status = 0x$('{0:X8}' -f $r.device_status)"
  } catch {
    Write-Output "$k -> ERROR: $($_.Exception.Message)"
  }
}
