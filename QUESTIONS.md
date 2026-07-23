# Questions for Windows Claude Code — G615LR per-zone Aura (`0x04`)

We are extremely close. Read `HANDOFF.md`'s "Linux session 3" section
first for full context — this file is just the distilled, actionable
question list pulled out of it so it's scannable on its own.

Where we are: individual per-zone chassis lightbar colour control
(protocol `0x04`) is **proven to work on this exact hardware** — you did
it, repeatedly, with a live Wireshark capture, including a custom India
flag layout with an animated breathing "chakra" on two specific zones
while the rest stayed static. That is not in question. What's in question
is why replicating it on Linux, using the real captured bytes and the real
captured pre-write sequence, still doesn't produce a visible per-zone
colour change — it does something (see below), just not the right thing
yet.

## What we know for certain (don't re-derive, just use)

- Packet format for `0x04` (51 bytes) is confirmed correct 3 independent
  ways: matches the live HID report descriptor pulled directly off this
  hardware, matches hand-built packets from `rog-aura::lightbar_2025`, and
  matches literal bytes replayed straight out of `usb_capture/aura.pcap`.
- The classic `0x5d` protocol (`Static`/`Breathe`/`RainbowCycle`/
  `RainbowWave`/`Pulse` only — the other 7 built-in modes are a genuine
  firmware limitation, confirmed via identical ACK behaviour for working
  vs non-working modes) now works cleanly on Linux for whole-chassis
  single-colour/effect control. That's a real, shipped win, separate from
  this investigation.
- Found and replicated the exact wire sequence that precedes the first
  `0x04` write in `aura.pcap`: `SET_IDLE`(iface1), `SET_IDLE`(iface0),
  `0x0201` "01 01", then `0x5d` `b3,b4,b5` (in that order — not `b3,b5,b4`)
  with the "priming" payload `5d b3 00 02 00 00 00 eb...`, then `0x0305`
  handshake, then the real `0x0304` write. Fully reproduced on Linux
  (`rog-platform/examples/g615lr-real-priming-sequence.rs`).
- That priming payload is **not** inert/vestigial (the original
  investigation's conclusion) — its mode byte (`02`) is a real
  `AuraModeNum::RainbowCycle`, and sending it alone visibly puts the whole
  chassis into genuine autonomous RainbowCycle animation on Linux, live
  confirmed.
- Priming + a single one-shot `0x04` write: chassis goes rainbow (proving
  the priming is real), the zone write has no visible incremental effect.
- Priming + 8 seconds of continuous `0x04` streaming (same single zone,
  ~4 writes/sec, `rog-platform/examples/g615lr-prime-then-stream.rs`):
  **still stuck on rainbow for the full 8 seconds.** This is the current
  dead end.

## The actual questions

1. **What's the real first-colour-change latency after priming, on
   Windows?** In a fresh capture, get a precise timestamp for the priming
   sequence's last packet and the timestamp your own eyes/a screen
   recording confirms the chassis actually shows a real colour (not just
   when the first `0x0304` packet is sent — when it's *visibly* correct).
   If that gap is more than 8 seconds, our test simply didn't run long
   enough and that alone might be the whole answer.

2. **Does the specific pattern of zones being written matter?** Every
   Linux test streamed the exact same single zone (`0x06`) over and over.
   The real capture's steady-state traffic cycles through many different
   zone IDs per packet, batched, changing constantly. Does replaying the
   ACTUAL cycling pattern from `aura.pcap` (not one static zone) change
   the outcome? If you can, try a Windows-side test that (like our Linux
   one) sends priming once then streams **only one unchanging zone/colour**
   continuously for 10+ seconds — if that ALSO fails to resolve to the
   real colour on Windows, that's a huge finding: it would mean our whole
   approach has been structurally wrong (needs actual zone variety to be
   recognized as "a real session"), not something Linux-specific.

   **ANSWERED (Windows session 3): no, zone variety is not required.** Ran
   exactly this test (`usb_capture_session3/g615lr_priming_then_static_hold.ps1`)
   — real priming sequence via `HidSend.cs` directly, bypassing Armoury
   Crate's GUI entirely, then one unchanging zone streamed continuously.
   Methodologically clean run (reset to a confirmed-dark baseline first,
   watched it go from dark to lit with nothing else touching the
   hardware): **it worked, the zone visibly lit up.** So a single static
   zone is sufficient in principle — cross this off the list, the gap on
   Linux is something else. See `HANDOFF.md` "Windows session 3" for full
   details, including an unrelated but major discovery made the same
   session: this repo's zone map had 6 of 16 wire IDs wrong (found via
   ASUS's own Aura Creator device-profile CSV, now in
   `usb_capture_session3/ground_truth/WDL_G615LR.csv`) — doesn't change
   the wire bytes any existing Linux test sent, but worth cross-checking
   `Lightbar2025Zone`'s variant names against that file rather than prose.

3. **Does `SET_IDLE` on interface 1 succeed on Windows?** On Linux it
   consistently comes back `STALL`/`Err(Pipe)` in every test (interface
   0's `SET_IDLE` succeeds fine). Probably benign, but never independently
   confirmed — check what Windows' `HidD_SetFeature`/underlying driver
   stack does here, or whether Windows even issues `SET_IDLE` explicitly
   vs it being implicit in a class driver init step we can't see in a
   packet capture.

4. **Does the priming sequence ever repeat within a single real session**
   (not just once at session start)? Our only data point (`aura.pcap`)
   only covers ~130 seconds from a session start. If you have or can
   capture a much longer real session, check whether priming recurs later
   — e.g. after switching between colour presets, after a sleep/wake
   cycle, or on some periodic timer.

5. **Per-write handle lifecycle**: `HidSend.cs`'s `TrySetFeature` opens a
   fresh `CreateFile` handle for every single write (its own comment notes
   `OpenPersistent`/`SetFeatureOnHandle` exist specifically because
   per-frame handle churn is "wasteful at 20-30fps," implying it's the
   default/simpler path). Our Linux tests open the device once and hold
   the interface claimed for the whole priming+stream sequence. Does it
   matter? If the *real* Armoury Crate traffic in `aura.pcap` shows
   evidence of repeated handle churn around each `0x0304` write (worth
   checking for anything that looks like device re-enumeration or
   `GET_DESCRIPTOR` calls interleaved with the writes), that's a real
   candidate difference we haven't controlled for at all.

## What to send back

Whatever you find — a fresh, precisely-timestamped capture (ideally
covering priming through the first confirmed real colour change, not just
the first `0x0304` packet), and a plain-language note on which of the
above got answered and how. Drop it in a new
`usb_capture_session3/` folder (matching the `usb_capture_session2`
pattern already in this repo) with the same kind of `NOTE_FROM_WINDOWS_CLAUDE.md`
you wrote last time — that format worked well and got real results fast.

Push straight to this shared repo
(`https://github.com/Kris9403/asusctl-wintest.git`) instead of a zip/Drive
handoff this time — see `CLAUDE.md` at the repo root for the workflow.
