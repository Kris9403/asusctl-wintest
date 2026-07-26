# CLAUDE.md — read this first, every session, either OS

This repo is being worked on by two separate Claude Code sessions in
parallel — one on Windows, one on Linux, on the same dual-boot ASUS ROG
Strix G16 2025 (`G615LR`) laptop, collaborating entirely through this git
repo (`https://github.com/Kris9403/asusctl-wintest.git`). No shared memory
between the two sessions — this file, `HANDOFF.md`, `QUESTIONS.md`, and git
history are the *only* channel. Read all of them before touching anything.

## The goal

Get independent per-zone RGB chassis lightbar control (protocol `0x04`,
16 zones, `rog-aura::lightbar_2025`) working on Linux. **It is proven to
work on this exact hardware** — repeatedly demonstrated on Windows with a
live Wireshark capture, including a custom India-flag layout with an
animated breathing effect on two specific zones while the rest stayed
static. This is not a "does the hardware support it" question anymore. It
is purely "why doesn't our Linux code produce the same result yet," and
we are close.

## Source of truth, in order

1. **`git log`** — the real timeline. Commit timestamps are authoritative;
   prose summaries (including this file) can drift stale, commit history
   can't. When in doubt about "what's actually been tried" or "when did X
   happen," check `git log --oneline --all` and read the actual diffs, not
   just what a HANDOFF.md section claims.
2. **`HANDOFF.md`** — the detailed research log. Append-only, one section
   per session (`## Linux session N`, `## Windows session N`), each dated.
   Don't rewrite earlier sessions' sections, even if something in them
   turns out to be wrong — correct it in a new section instead, so the
   reasoning trail stays intact. This is where findings, evidence, and
   ruled-out theories live in full detail.
3. **`QUESTIONS.md`** — the live discussion channel between the two
   sessions, not a one-shot ask list. Answer questions inline (don't
   delete them, add the answer under them), add new ones as they come up,
   treat it like a shared notebook both sides read and write to every
   session.
4. **This file (`CLAUDE.md`)** — kept up to date as the short "what's the
   current state, right now" summary. If something here contradicts
   `HANDOFF.md`, `HANDOFF.md`'s most recent session section wins (this
   file should get fixed to match, it's the one more likely to be stale).

## Where the data lives

- `usb_capture/` — Windows session 1: the original protocol reverse-
  engineering (PowerShell scripts, `HidSend.cs`, multiple `.pcap`/`.pcapng`
  captures including the working India-flag/chakra demo, `README.md` with
  the full narrative writeup).
- `usb_capture_session2/` — Windows session 2: a targeted interface-0
  handshake capture + `NOTE_FROM_WINDOWS_CLAUDE.md`, handed over mid-
  investigation. Turned out to be a different (mode-cycling) capture than
  the one that actually unlocked `0x04`, but real, useful signal.
- `usb_capture_session3/` — Windows session 3 (2026-07-23): the priming/
  static-hold test that answered `QUESTIONS.md` Q2
  (`g615lr_priming_then_static_hold.ps1`), the labeled zone-map diagram
  (`draw_zone_map.py` / `g615lr_zone_map.png`), and
  `ground_truth/WDL_G615LR.csv` — ASUS's own official Aura Creator
  device-layout file for this exact laptop, the source that fixed 6 wrong
  zone IDs in this repo's map. Pull this CSV directly rather than trusting
  zone names in prose anywhere else in this repo.
- `usb_capture_session4/` — Windows session 4 (2026-07-23): two more real
  captures. `multizone_12x_confirmed.pcapng` — 12 of 16 zones set
  simultaneously to distinct colours via direct `HidSend.cs` calls,
  human-confirmed correct on every zone (twice); full byte table in
  `HANDOFF.md`, this is the reference to diff Linux's own packet output
  against. `breathing_mode_capture.pcapng` — the capture behind the major
  `0x0305` discovery (see "Current state" below and `HANDOFF.md`): built-in
  animated effects (Breathing/Strobing/Color Cycle) use a completely
  separate, continuously-streamed protocol, nothing to do with `0x04`.
- `linux_capture_session4/` — Linux session 4 (2026-07-24): raw `usbmon`
  text captures (see that folder's own `NOTE_FROM_LINUX_CLAUDE.md` for
  what each one shows) backing the byte-for-byte wire verification, the
  GUI-traffic confirmation, and the literal-replay test — citable evidence
  for the claims in `HANDOFF.md` Linux session 4, not just prose.
- `rog-platform/examples/g615lr-*.rs` — every Linux-side reproducible
  test binary, runnable directly (`sudo target/debug/examples/<name>`
  after `cargo build --example <name> -p rog_platform`). Each has a doc
  comment explaining what it tests and why. Don't re-run tests already
  covered here expecting a different result — check `HANDOFF.md` first for
  what's already been ruled out.

## Git workflow

Both sessions push to and pull from the shared remote above. Plain
workflow, no special branching scheme needed yet:

```sh
git pull
# ... do work, test on real hardware ...
git add <specific files>   # never `git add -A` — see the CRLF note below
git commit -m "..."
git push
```

One real gotcha already hit once (documented in full in `HANDOFF.md`'s
"Housekeeping" section): if this repo ever gets copied between the two
machines by anything other than `git clone`/`git pull` (e.g. a zip, a
cloud-drive sync), it can pick up CRLF line endings across the whole tree
and make `git status` show ~200 files as "modified" when none of them
really changed. If that happens: `git diff --ignore-space-at-eol --stat`
to confirm it's pure noise, then `git checkout -- .` to clear it. Prefer
`git pull` over any other transfer method going forward to avoid this
entirely.

## Current state (check `git log` for anything newer than this)

- 🎯 **Real progress, NOT fully resolved (2026-07-26, Windows session
  9): first-ever successful `0x04` reproduction from our own code, but
  the follow-up isolation test contradicted itself.** Sent a real
  `count=5` multi-zone packet (5 zones in one write, keyboard zones at
  alpha~0, lightbar zone `back_right` at full alpha with a real colour)
  via raw `HidD_SetFeature` on Windows — the lightbar zone lit up,
  live-confirmed, wire-verified byte-for-byte identical to a real Aura
  capture. Every prior `0x04` test on either OS had used `count=1`
  (one zone per packet); this was the first `count>1` test ever run.
  **But** the obvious follow-up (`count=1` targeting only `back_right`,
  same everything else) gave OPPOSITE results on two consecutive runs —
  first run: nothing; second run, same script: lit up. So `count>1`
  being the real prerequisite is NOT confirmed — could instead be
  carried-over device/EC state from earlier successful writes this
  boot session. **The test that would actually resolve this (not yet
  done)**: full reboot, then `count=1` on `back_right` as the very
  first thing sent to that zone. Do not trust either "count matters" or
  "count doesn't matter" until that clean test runs. See `HANDOFF.md`
  "NOT resolved" section (search for that exact heading) and
  `QUESTIONS.md` for full detail, all scripts, and both capture files.
- ✅ Basic whole-chassis colour/effect control via the classic `0x5d`
  protocol: **shipped and working**, 5 of 12 built-in modes confirmed live
  (`Static`, `Breathe`, `RainbowCycle`, `RainbowWave`, `Pulse`), the other
  7 confirmed as a real firmware limitation on this specific board (not a
  bug — see `HANDOFF.md` Linux session 3 for the ACK-comparison evidence).
- ❌ Independent per-zone control via `0x04`: **not yet working on Linux**,
  but a real single unchanging zone/colour, streamed continuously after
  real priming, **is now confirmed sufficient on Windows** (Windows
  session 3 answered `QUESTIONS.md` Q2 — zone variety is not required).
  So the remaining Linux gap is something else, not "needs more zones."
  See `HANDOFF.md` Windows session 3 and Linux session 3 Part B.
- ✅ **Zone map fixed and doubly verified (2026-07-24)**: the 6 wrong wire
  IDs found in Windows session 3 (back edge `0x04-0x07`, side front/back
  split `0x08/0x09`/`0x0A/0x0B`) are now corrected in
  `rog-aura/src/lightbar_2025.rs` (`Lightbar2025Zone` variant names only —
  wire byte values were never wrong). Verified two ways: a permanent unit
  test matching the human-confirmed 12-zone capture exactly, and a live
  `usbmon` capture proving Linux's own wire bytes match the Rust code's
  intended bytes byte-for-byte. Packet construction is about as exonerated
  as it can be.
- 🧪 **`0x0305` (the animated-effects protocol) tested on Linux for the
  first time (2026-07-24), consistently negative.** Three controlled
  variants (with real priming, without priming, with a real colour
  pre-set) all produced no observable effect beyond whatever the *other*
  mechanism in play was already doing. Also tried streaming it in parallel
  with `0x04` — also negative, and consistent with real Windows captures
  never actually combining the two. Not yet known whether this is a real
  firmware gap or a still-missing prerequisite — see `HANDOFF.md` Linux
  session 4 and `QUESTIONS.md`.
- 🎯 **`0x04` reframed and sharply narrowed (2026-07-24, Windows session 5
  + Linux session 5).** The "priming" `5d b3/b4/b5` triplet is not a
  handshake — it's a real, successfully-applied `0x5d set-effect
  (RainbowCycle) + apply` command. The actual question was never "why
  doesn't `0x04` work," it's **"why doesn't `0x04` override an
  already-active `0x5d` RainbowCycle animation."** Direct evidence: a
  40-second continuous `0x04` stream produces a subtle flicker synced to
  *every single write* — the writes ARE landing, but RainbowCycle's own
  animation refresh loop overwrites the buffer again on its very next
  tick, every time, without ever resetting its own cycle (confirmed by
  direct observation: the rainbow just keeps smoothly progressing through
  its animation, unaware of the writes). No timing threshold to wait out —
  the competing loop never stops running in the first place. **Next test**:
  explicitly cancel the `0x5d` RainbowCycle state (real `Static`) before
  attempting `0x04`, instead of relying on `0x04` to override a still-
  active animation — flagged by Windows, not yet tried by either side.
- ✅ **Real dispatch wiring for `0x04` now exists and works end-to-end**
  (Windows session 6, compiled+fixed+tested Linux session 5): a D-Bus
  method (`WriteLightbar2025Zones`), a CLI command
  (`asusctl lightbar2025 --zone <id>:<hex>`), and a GUI 16-zone canvas —
  all previously orphaned, now reachable by a real user. One compile fix
  needed (a missing Slint re-export, one line). Tested live: round-trips
  cleanly (`"Sent 1 zone(s)"`, no error) but — as expected, since it hits
  the same hardware — produces the same zero visible effect as every raw
  test. Useful, real infrastructure; does not itself change the underlying
  mystery above.
- ✅ **Windows pre-init capture finally done (2026-07-25, Windows session
  7), Linux session 6's ask answered.** Real disable/re-enable of the
  `MI_01` HID collection caught a live `0x5d` "ASUS Tech.Inc." handshake
  for the first time on Windows — matches Linux's kernel-reprobe capture
  where they overlap. But no `0x5a`, no `0x5e`, and no distinct
  "direct mode" command found; traffic just resumed the same `0x0305`
  RainbowCycle stream after the handshake. Real, useful negative — the
  full three-way handshake (`0x5a`+`0x5d`+`0x5e`) still hasn't been
  captured live on Windows, only on Linux's full bus-level reprobe. See
  `HANDOFF.md` Windows session 7, `QUESTIONS.md`.
- 🔍 **Report-descriptor audit + a new hypothesis, tested negative
  (2026-07-25, Linux session 6).** An external maintainer's "3 HID
  devices, only the vendor one accepts `0x04`" claim was checked directly
  against this hardware by fully parsing both interfaces' raw HID report
  descriptors — does **not** hold here, confirmed exactly 2 HID devices
  (matches `lsusb`). Found and tested a genuinely new lead inside
  interface 1's single vendor collection — Report ID `0x06`, a
  boolean-shaped Feature report never tried before, structurally
  resembling a "direct mode" toggle. **Tested, negative**: the write
  succeeds transport-wise but a 10s `0x04` stream on top still produces
  zero visible effect. This is now ~7-8 independently-failed hypotheses
  against the same symptom — per systematic debugging, that pattern means
  we're guessing at an init sequence we've never actually *observed*, not
  narrowing in on the right byte. **Asked Windows (QUESTIONS.md) for the
  one piece of evidence neither side has captured yet**: a Wireshark/
  USBPcap capture that starts *before* device init — disable/re-enable
  the device in Device Manager while capturing — to see the real
  enumeration + init sequence Armoury Crate's driver sends, instead of
  steady-state traffic from an already-initialized session.
