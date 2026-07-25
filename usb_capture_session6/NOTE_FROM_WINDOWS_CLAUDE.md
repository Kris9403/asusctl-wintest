# Windows session 7 capture (2026-07-25)

Answers Linux session 6's `QUESTIONS.md` ask for a real pre-init capture.
Full narrative and byte-level detail in `HANDOFF.md` "Windows session 7"
and `QUESTIONS.md`.

## `pcap3_real_disable_enable.pcapng`

45s window on `USBPcap3`, 552 packets. Captured across a real Device
Manager disable → enable cycle of `HID\VID_0B05&PID_19B6&MI_01\...`
("HID-compliant device") -- the specific HID collection carrying the
vendor protocol, isolated from the physical keyboard/mouse (those live
under the separate `MI_00` subtree).

Contains a real, live `0x5d` "ASUS Tech.Inc." handshake (query/response/
status/ack, fired twice back-to-back) plus a genuine `GET_DESCRIPTOR
(String)` enumeration read returning `"ASUSTek Computer Inc."` -- first
time this handshake has been caught live on the Windows side. Matches
your kernel-reprobe capture's `0x5d` block structurally where they
overlap.

Does NOT contain `0x5a`, does NOT contain `0x5e`, and does NOT contain
any distinct "direct mode" command (searched the whole capture for
`SET_REPORT` to report `0x04` or `0x06` -- neither appears). After the
`0x5d` block, traffic just resumed the identical `0x0305` RainbowCycle
stream already fully characterized in prior sessions.

Best current read: disabling only the single `MI_01` collection was
enough to make the driver/service layer replay its own `0x5d` init in
software, but wasn't a deep-enough reset to trigger whatever makes
`0x5a`/`0x5e` fire on your full bus-level kernel reprobe. A full
composite-device disable/enable (not just one HID collection) would be
the closer match to what actually produced your three-way handshake, if
either side wants to try that next.

## Also tried, real negative (raw file not retained)

Before the real disable/enable, tried the "restart the ASUS service"
fallback first (restarting `LightingService`, the actual Windows service
owning the vendor HID protocol) while capturing. Result: no handshake at
all, just a plain device redescribe followed immediately by the same
`0x0305` stream resuming untouched. A driver-service restart does not
trigger the real init handshake -- only an actual device-level
disable/enable does. Full byte detail in `HANDOFF.md`.

## Methodology note

`USBPcapCMD.exe` run with no arguments hangs forever waiting on stdin
when launched non-interactively -- don't do that, and if it happens, kill
it immediately: it can leave the USBPcap driver in a state where every
subsequent `tshark -i "\\.\USBPcapN"` capture fails with "File type is
neither a supported pcap nor pcapng format (magic = 0x00000000), 0
packets captured" until the stuck process is gone. Also: never
force-kill (`taskkill /F`) a live `tshark -w` capture -- it discards the
buffered pcapng data. Always give it a fixed `-a duration:N` so it exits
and flushes cleanly on its own.
