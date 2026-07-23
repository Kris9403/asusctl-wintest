# Note to Linux Claude Code, from Windows Claude Code

You and I are two separate Claude Code sessions, running on the same
human's dual-boot ASUS ROG Strix G16 2025 (`G615LR`) laptop, working the
same investigation from opposite sides of the reboot. We don't share
memory or context — the human is manually relaying findings between us by
copy-pasting and moving files. This note is me (the Windows-side session)
briefing you directly, the way I'd brief a colleague picking up a shift,
because generic docs like `HANDOFF.md` are written for "whoever, whenever"
and lose the "I just did this, minutes ago, here's exactly what happened"
context that matters for judging how much to trust it.

Quick status check so you know where you're reading this from: you (Linux
Claude Code) already did substantial real work I can see evidence of —
compiling the workspace clean, building test binaries in
`rog-platform/examples/`, exhaustively ruling out packet content/transport/
kernel-driver/timing/daemon-interference as causes for why `0x04` color
packets do nothing on this hardware, and fixing a real bug in
`set_feature_report` (silent failure on `try_borrow()`). None of that is
lost — this note is *additive*, not a replacement.

## What I found, this session, on the Windows side

The human's working theory (which you'll have seen too) was that Armoury
Crate's Windows services do something once at startup that "unlocks" the
`0x04` protocol for the EC to actually act on — since USB-level success
without a visible LED change is exactly what you'd see if the device ACKs
a write but its firmware logic is still in "I drive my own lights" mode.

To test this, we did what neither of us had done yet: got Armoury Crate's
services *genuinely* disabled (`sc.exe config <name> start= disabled` —
`Stop-Service` alone silently no-ops, learned that the hard way earlier
this whole investigation), rebooted to reach a real EC-owned baseline,
then ran a fresh USBPcap capture spanning the moment those services were
re-enabled and Armoury Crate actually took over.

**Result: found a real handshake sequence, entirely on interface 0** —
which is significant because every single test either of us has run so
far, on both OSes, only ever touched interface 1. Nobody has sent anything
to interface 0 as part of trying to unlock this.

The sequence, chronologically:

1. `t≈21s`: `SET_REPORT`, Report ID `1`, **Output**, 2 bytes: `01 01`.
2. `t≈31–42s`, an ~11.5 second burst: a genuine **read/write negotiation**
   on Report ID `0x5d` as a **Feature** report (not the `b3` Output-report
   traffic you already proved is dead — that's separate, still just
   vestigial noise in this capture too, consistent with what you found).
   ~60 writes, ~42 `GET_REPORT` reads, interleaved. The device doesn't
   just ack these — reading back after a write returns real data, e.g.
   write `5d 05 20 31 00 10 00 00...` gets back `5d 05 20 31 00 10 03 01
   01 02 25 05 01 02 46 03 11 01 0c 00...`. One write in this burst is the
   literal ASCII string `"ASUS Tech.Inc."`. A fixed vendor string being
   part of this strongly suggests a deterministic capability/version
   negotiation, not session-specific cryptography — there'd be no reason
   for a real nonce exchange to include a hardcoded literal string.
3. Buried in that same burst, sent **exactly once** across the whole
   ~130-second capture (everything else in the burst repeats many times):
   `t≈33.66s`, `SET_REPORT`, Report ID `0x5a` — **never seen in any
   capture either of us has looked at before this**, Feature, 64 bytes:
   `5a ba c5 c4 01 00 00 00...`. Being singular is what makes this the
   strongest candidate for the actual "hand control to host" moment.
4. Only after all of this — many seconds later — does interface-1 traffic
   begin: the already-known `0x05` (10 bytes) then `0x04` (51 bytes, the
   color protocol you've been testing against).

Full exact transcript — every interface-0 write, in order, frame number,
timestamp, wValue, complete hex payload — is in the sibling file
`handshake_transcript.tsv` (same folder as this note, tab-separated, 110
lines). That's the source of truth; treat my prose above as a compressed
summary of it, not a replacement.

One honest caveat, from something the human mentioned partway through:
the profile Armoury Crate restored in this specific capture was "Dark"
(a remembered setting from before this session, not something set fresh).
That's consistent with `0x5a`'s payload possibly being profile-specific
data ("restore: Dark") rather than a universal unlock constant — I don't
know which, and neither do you yet. Worth keeping in mind if a literal
replay produces a *dark* result but color still doesn't stick afterward.

## What I'd suggest you do with this

1. Get the transcript file onto your side — the human will either copy
   `handshake_transcript.tsv` over directly, or has pasted its contents to
   you along with this note.
2. Don't hand-pick 3-4 "important-looking" packets. Replay the *entire*
   interface-0 write sequence, in order, exact bytes, the same way you
   already built `g615lr-raw-usb-test.rs` (detach `hid_asus` from
   interface 0 this time, not 1; raw libusb control transfers,
   `bmRequestType=0x21, bRequest=0x09`, `wValue`/`wIndex=0` per line in the
   file). Reads don't need real replies matched — try without replicating
   them first.
3. **Checkpoint before touching color at all: does the replay make the
   lights go dark?** That's the first positive, falsifiable signal this
   entire investigation has had. If yes — huge, then try a `0x04` color
   packet on interface 1 right after. If no — this specific theory is
   likely wrong (or there's a parallel ACPI/WMI-side component not visible
   in USB traffic at all, which was the other live lead from your session
   1 notes), and I'd stop iterating on this exact packet sequence without
   new evidence rather than guessing further variations blind.
4. Whatever happens, please update `HANDOFF.md` yourself with the real
   result (worked / didn't / partially) before the human takes this back
   to me again — right now my copy of that file is genuinely behind yours
   (I did not touch your `rog-platform/examples/` additions or your
   `set_feature_report` fix, on purpose, specifically to avoid clobbering
   your work), so treat your copy as canonical and just append, the same
   way I appended a "Windows session 2" section to mine instead of
   rewriting what you'd already written.

Good luck. This is the first real lead with a falsifiable checkpoint
either of us has had — worth the one more test, but if it's a dead end,
it's a cheap one to rule out.

— Windows Claude Code
