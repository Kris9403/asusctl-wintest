# Linux session 6 captures (2026-07-25)

First session where live Wireshark GUI capture actually worked on Linux
(`usbmon5`, run as the logged-in user rather than via `sudo tshark` CLI --
the CLI route kept hitting AppArmor/dumpcap confinement all session, the
GUI app just works). See `HANDOFF.md` "Linux session 6" for full context.

## `classic_mode_switch_rainbowwave_pulse.pcapng`

Real classic-`0x5d`-protocol mode switching via `rog-control-center` GUI:
RainbowWave (`5d b3 00 03 cc cc cc eb...` at t≈3.96s) then Pulse
(`5d b3 00 0a cc cc cc eb...` at t≈12.67s), both in the real `b3,b5,b4`
order, both ACK'd (`5d ec b3/b5/b4`). Confirms `write_effect_and_apply`
genuinely never sends the `b3/b4/b5` priming triplet for classic mode
changes -- that's Armoury-Crate-specific behaviour from the original
Windows capture, not a universal wire requirement. No `0x04` traffic in
this one at all -- didn't touch the lightbar canvas this run.

## `alpha_ramp_0x04_test_wire_verified.pcapng`

Captured while running `rog-platform/examples/g615lr-alpha-ramp.rs` --
primes, then streams zone `0x02` (kbd3) with a continuously-ramping alpha
byte (triangle wave, matching the exact waveform found in Windows'
`25/usb_data.txt` capture pushed this session) for 15s, 484 writes.

**This is the important one.** Every single one of the 484 writes is
confirmed on the wire, byte-for-byte correct: `04 01 01 02 00...ff 00 00
[ramping alpha]`, steady ~31ms spacing, alpha genuinely different every
frame. Structurally indistinguishable from a real animated Aura Creator
frame. Visually: still nothing distinguishable beyond the priming-induced
RainbowCycle. First time an `0x04` test has been independently wire-
verified on Linux rather than judged by eye alone -- rules out "packets
silently dropped/coalesced before the wire" as an explanation. Whatever's
still missing is confirmed firmware/device-side, not a Linux transport or
packet-construction bug.
