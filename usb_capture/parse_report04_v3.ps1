$dir = "C:\Users\Krushna\AppData\Local\Temp\claude\C--Users-Krushna-claude\adbed461-e0a3-4f3c-84e7-623be742f445\scratchpad\usb_capture"
$p = "$dir\test_documented.pcapng"

$frames = & "C:\Program Files\Wireshark\tshark.exe" -r $p -Y "frame contains 21:09:04:03" -T fields -e frame.number 2>&1

foreach ($fr in $frames) {
  $hex = & "C:\Program Files\Wireshark\tshark.exe" -r $p -Y "frame.number==$fr" -T fields -e usb.data_fragment 2>&1
  $bytes = @()
  for ($i=0; $i -lt $hex.Length; $i+=2) { $bytes += [Convert]::ToInt32($hex.Substring($i,2),16) }
  $count = $bytes[1]
  $indexed = for ($i=0; $i -lt $bytes.Length; $i++) { "$i=$('{0:X2}' -f $bytes[$i])" }
  Write-Output "frame $fr count=$count totalbytes=$($bytes.Length) : $($indexed -join ' ')"
}
