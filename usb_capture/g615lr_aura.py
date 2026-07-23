#!/usr/bin/env python3
"""
ASUS ROG Strix G16 (2025) / G615LR Aura RGB control.

Reverse-engineered protocol (verified against real captures + Aura Creator's
own saved XML profile): color-setting does NOT use the classic 0x5d N-Key
static/breathe protocol. It uses a separate multi-zone batch report on
Report ID 0x04 (Feature report), sent as a USB control transfer:

    bmRequestType = 0x21 (host->device, class, interface)
    bRequest      = 0x09 (SET_REPORT)
    wValue        = 0x0304 (ReportType=Feature(3), ReportID=4)
    wIndex        = 0x0001 (interface 1)
    wLength       = 0x0033 (51 bytes)

Payload (51 bytes):
    byte 0      = 0x04 (report id echo)
    byte 1      = zone count N (1-8)
    byte 2      = 0x01 (cmd flag, meaning unconfirmed -- 0x01 works)
    bytes 3-18  = 8 zone-ID slots, 2 bytes LE each (first N used, rest 0)
    bytes 19-50 = 8 color slots, 4 bytes each: [G, R, B, A] (first N used, rest 0)
                  NOTE: channel order is G,R,B -- NOT R,G,B. A is always 0xFF.

Usage:
    python3 g615lr_aura.py

Requires: pyusb (pip install pyusb)
"""
import usb.core
import usb.util

VID = 0x0B05
PID = 0x19B6

# Zone name -> USB zone ID, from keyboardmap.txt cross-verified against the
# Aura Creator XML "index" field for this exact device (G615LR).
ZONES = {
    "kbd1": 0x00, "kbd2": 0x01, "kbd3": 0x02, "kbd4": 0x03,
    "back_bar_left": 0x04, "back_bar_right": 0x05,
    "corner_tl": 0x06, "corner_tr": 0x07,
    "left_side_back": 0x08, "left_side_front": 0x09,
    "right_side_front": 0x0A, "right_side_back": 0x0B,
    "corner_br": 0x0C, "corner_bl": 0x0D,
    "front_bar_left": 0x0E, "front_bar_right": 0x0F,
}


def build_packet(zone_colors):
    """
    zone_colors: list of (zone_id:int, r:int, g:int, b:int), max 8 entries.
    Returns the 51-byte report payload (not including the setup packet).
    """
    if not 1 <= len(zone_colors) <= 8:
        raise ValueError("must set between 1 and 8 zones per packet")

    pkt = bytearray(51)
    pkt[0] = 0x04
    pkt[1] = len(zone_colors)
    pkt[2] = 0x01

    for i, (zone_id, r, g, b) in enumerate(zone_colors):
        zoff = 3 + i * 2
        pkt[zoff] = zone_id & 0xFF
        pkt[zoff + 1] = (zone_id >> 8) & 0xFF

        coff = 19 + i * 4
        pkt[coff] = g & 0xFF      # channel order is G,R,B,A -- confirmed
        pkt[coff + 1] = r & 0xFF
        pkt[coff + 2] = b & 0xFF
        pkt[coff + 3] = 0xFF      # alpha/enable, always full

    return bytes(pkt)


def send_zones(dev, zone_colors):
    """Send one batch (<=8 zones) of (zone_id, r, g, b) tuples to the device."""
    payload = build_packet(zone_colors)
    # bmRequestType=0x21, bRequest=0x09(SET_REPORT), wValue=0x0304, wIndex=1
    dev.ctrl_transfer(0x21, 0x09, 0x0304, 0x0001, payload)


def set_all_zones(dev, r, g, b):
    """Convenience: set every one of the 16 zones to the same color."""
    all_zones = list(ZONES.values())
    for i in range(0, len(all_zones), 8):
        batch = [(z, r, g, b) for z in all_zones[i:i + 8]]
        send_zones(dev, batch)


def main():
    dev = usb.core.find(idVendor=VID, idProduct=PID)
    if dev is None:
        raise SystemExit(f"Device {VID:04x}:{PID:04x} not found")

    if dev.is_kernel_driver_active(1):
        dev.detach_kernel_driver(1)

    # Example: keyboard all red, lightbar corners in R/G/Y/B, everything
    # else off. Edit this to whatever you want to test.
    send_zones(dev, [
        (ZONES["kbd1"], 255, 0, 0),
        (ZONES["kbd2"], 255, 0, 0),
        (ZONES["kbd3"], 255, 0, 0),
        (ZONES["kbd4"], 255, 0, 0),
        (ZONES["corner_tl"], 255, 0, 0),
        (ZONES["corner_tr"], 0, 255, 0),
        (ZONES["corner_br"], 255, 255, 0),
        (ZONES["corner_bl"], 0, 0, 255),
    ])
    print("Sent. Check the keyboard (red) and 4 corners (R/G/Y/B).")


if __name__ == "__main__":
    main()
