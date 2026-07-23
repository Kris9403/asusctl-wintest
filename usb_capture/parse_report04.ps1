$dir = "C:\Users\Krushna\AppData\Local\Temp\claude\C--Users-Krushna-claude\adbed461-e0a3-4f3c-84e7-623be742f445\scratchpad\usb_capture"
$files = "test.pcapng","test2.pcapng","test3.pcapng","test4.pcapng","allcorners.pcapng","allmodes.pcapng"

$zoneNames = @{
  0="KbdZone1(Left)"; 1="KbdZone2"; 2="KbdZone3"; 3="KbdZone4(Right)";
  4="BackBar(L)"; 5="BackBar(R)"; 6="Corner(TL)"; 7="Corner(TR)";
  8="LSidebar(Back)"; 9="LSidebar(Front)"; 10="RSidebar(Front)"; 11="RSidebar(Back)";
  12="Corner(BR)"; 13="Corner(BL)"; 14="FrontBar(L)"; 15="FrontBar(R)"
}

foreach ($f in $files) {
  $p = "$dir\$f"
  $frames = & "C:\Program Files\Wireshark\tshark.exe" -r $p -Y "frame contains 21:09:04:03" -T fields -e frame.number 2>&1
  if (-not $frames) { continue }
  Write-Output "=========== $f ==========="
  foreach ($fr in $frames) {
    $hex = & "C:\Program Files\Wireshark\tshark.exe" -r $p -Y "frame.number==$fr" -T fields -e usb.data_fragment 2>&1
    if (-not $hex) { continue }
    $bytes = @()
    for ($i = 0; $i -lt $hex.Length; $i += 2) { $bytes += [Convert]::ToInt32($hex.Substring($i,2),16) }
    $reportId = $bytes[0]
    $count = $bytes[1]
    $cmd = $bytes[2]
    $zones = @()
    $off = 3
    for ($z = 0; $z -lt $count; $z++) {
      $zid = $bytes[$off] + ($bytes[$off+1] * 256)
      $zones += $zid
      $off += 2
    }
    $zoneStr = ($zones | ForEach-Object { "$_=$($zoneNames[$_])" }) -join ", "
    $rgbBytes = $bytes[$off..($bytes.Length-1)]
    $rgbTriplets = @()
    for ($i = 0; $i -lt $count*3; $i += 3) {
      if ($i+2 -lt $rgbBytes.Length) {
        $rgbTriplets += ("{0:X2}{1:X2}{2:X2}" -f $rgbBytes[$i], $rgbBytes[$i+1], $rgbBytes[$i+2])
      }
    }
    Write-Output "frame $fr : reportId=0x$('{0:X2}' -f $reportId) count=$count cmd=0x$('{0:X2}' -f $cmd) zones=[$zoneStr] colors=[$($rgbTriplets -join ', ')]"
  }
}
