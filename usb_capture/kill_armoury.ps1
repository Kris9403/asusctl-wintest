$svcNames = @("ArmouryCrate.Service","ArmouryCrateControlInterface","ArmouryHtmlDebugServer","ArmourySocketServer","ArmourySwAgent","AsusSoftwareManager","AsusSoftwareManagerAgent","GameSDK","LightingService","ROGLiveService")
foreach ($n in $svcNames) {
    Stop-Service -Name $n -Force -ErrorAction SilentlyContinue
}
Start-Sleep -Seconds 2
Get-Process | Where-Object { $_.Name -match "Armoury|LightingService|AsusSoft|ROG|GameSDK" } | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 1
Write-Output "--- remaining ---"
Get-Process | Where-Object { $_.Name -match "Armoury|LightingService|AsusSoft|ROG|GameSDK" } | Select-Object Name, Id
if (-not (Get-Process | Where-Object { $_.Name -match "Armoury|LightingService|AsusSoft|ROG|GameSDK" })) {
    Write-Output "All Armoury Crate services/processes stopped."
}
