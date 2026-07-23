$dir = "C:\Users\Krushna\AppData\Local\Temp\claude\C--Users-Krushna-claude\adbed461-e0a3-4f3c-84e7-623be742f445\scratchpad\usb_capture"
$etl = "$dir\wmitrace.etl"
$xml = "$dir\wmitrace.xml"
Remove-Item $etl, $xml -ErrorAction SilentlyContinue

logman stop AsusWmiTrace -ets 2>$null | Out-Null
logman start AsusWmiTrace -p Microsoft-Windows-WMI-Activity -o $etl -ets

Write-Host ""
Write-Host "Tracing started. Now go set a keyboard zone (or the lightbar) to a" -ForegroundColor Cyan
Write-Host "distinct static color in Armoury Crate and hit Apply." -ForegroundColor Cyan
Write-Host "Press Enter here once you're done." -ForegroundColor Cyan
Read-Host

logman stop AsusWmiTrace -ets
tracerpt $etl -o $xml -of XML -y | Out-Null

Write-Host "=== WMI-Activity events mentioning Asus / WMNB / ATK ===" -ForegroundColor Yellow
Select-String -Path $xml -Pattern "Asus|WMNB|ATK" -Context 0,0 | Select-Object -First 200
