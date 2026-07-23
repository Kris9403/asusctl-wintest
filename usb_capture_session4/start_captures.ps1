Start-Process "C:\Program Files\Wireshark\tshark.exe" -ArgumentList '-i "\\.\USBPcap1" -w "C:\Users\Krushna\asusctl-wintest\usb_capture_session4\cap1.pcapng" -a duration:180'
Start-Process "C:\Program Files\Wireshark\tshark.exe" -ArgumentList '-i "\\.\USBPcap2" -w "C:\Users\Krushna\asusctl-wintest\usb_capture_session4\cap2.pcapng" -a duration:180'
Start-Process "C:\Program Files\Wireshark\tshark.exe" -ArgumentList '-i "\\.\USBPcap3" -w "C:\Users\Krushna\asusctl-wintest\usb_capture_session4\cap3.pcapng" -a duration:180'
