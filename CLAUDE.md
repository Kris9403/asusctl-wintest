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
- ⚠️ **This repo's zone map had 6 of 16 wire IDs wrong** (the back edge,
  `0x04-0x07`, and the left sidebar's front/back split, `0x09`/`0x0B`) —
  corrected in Windows session 3 (2026-07-23) against
  `usb_capture_session3/ground_truth/WDL_G615LR.csv`, ASUS's own official
  Aura Creator device profile for this laptop. Doesn't change any wire
  bytes already sent by existing code/tests (a wire ID of `0x06` was
  always `0x06` regardless of what it was labeled), but if anything
  references zone names by their *old* labels rather than the hex ID,
  re-check it against that CSV, not against older prose in this repo.
  Re-validated live, human-confirmed, across 12 of 16 zones at once in
  Windows session 4 (2026-07-23) — see
  `usb_capture_session4/multizone_12x_confirmed.pcapng`.
- 🆕 **Major discovery, Windows session 4 (2026-07-23): `0x0305` is a real,
  separate, continuously-streamed animated-effects protocol, not a
  one-shot handshake.** Built-in Armoury Crate effects (Breathing/
  Strobing/Color Cycle) send **zero** `0x0304` packets — they drive the
  whole chassis through `0x0305`, streamed at ~5-15Hz for as long as the
  effect is active (`05 01 00 00 0f 00 [byte6] 00 [byte8] [byte9]`, which
  byte varies depends on the mode — full table in `HANDOFF.md`). This has
  never been attempted on Linux and has nothing to do with the still-open
  `0x04` mystery — a real, independently achievable target. See
  `HANDOFF.md`'s "Major discovery" section and the new questions at the
  bottom of `QUESTIONS.md`.
- ❓ Q1 (precise priming→visible-colour latency) is still open — not
  answered yet, see `QUESTIONS.md`.
