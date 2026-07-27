# Linux session 7 captures (2026-07-27)

See `HANDOFF.md` "Linux session 6, the clean-boot decisive test + MS OS
descriptor investigation" and the section following it for full context.
These two captures are the tail end of an already-exhaustive investigation.

## `authorized_toggle_full_reenum.pcapng`

Captured across a genuine USB-core-level reset (`/sys/bus/usb/devices/
5-4/authorized` toggled 1->0->1) -- deeper than any driver-unbind test
this whole investigation, forces a real logical disconnect/reconnect of
the whole composite device, both interfaces together. Confirms the exact
same `0x5a`/`0x5d`/`0x5e` three-way handshake already fully characterized
from the earlier kernel-reprobe capture (`kernel_reprobe_real_init_
sequence.pcapng`, session 6) -- byte-for-byte the same structure, same
brightness-restore command, same `0101` write afterward. Nothing new.
Confirms the deepest possible Linux-side reset doesn't reveal a hidden
extra init step.

## `dark_blue_frontbar_wire_verified.pcapng`

Captured via `sudo dumpcap -i usbmon5 -a duration:45 -w ...` (run
directly, non-interactively, across all six `usbmon` interfaces
simultaneously -- a working pattern for automated capture that sidesteps
the AppArmor/dumpcap issues hit earlier with scripted `sudo tshark`).

Captured `g615lr-dark-blue-frontbar.rs`: dark reset -> static BLUE (whole
chassis, classic protocol) -> `0x0305` handshake -> `count=4` packet
targeting ONLY the front lightbar zones (`0x0C/0x0D/0x0E/0x0F` --
front-corner-right/left, front-bar-right/left), bright cyan, full alpha.
Wire-verified byte-correct: `04 04 01 0c 00 0d 00 0e 00 0f 00...` then
all four RGBA blocks `[00,ff,ff,ff]` (cyan, full alpha), exactly as
intended. **Visually: whole chassis stayed static blue, front lightbar
showed nothing distinct -- same negative result as every other `0x04`
test this entire investigation.** Packet construction and wire
transmission confirmed correct yet again; the underlying gap is
unchanged.
