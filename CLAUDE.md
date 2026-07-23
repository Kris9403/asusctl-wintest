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
- `usb_capture_session3/` (create this if it doesn't exist yet) — where
  the next round of Windows-side data/notes should go, matching the same
  pattern: raw capture(s) + a `NOTE_FROM_WINDOWS_CLAUDE.md`-style writeup.
  See `QUESTIONS.md` for exactly what's being asked for.
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

## Current state (as of the commit that added this file — check `git log` for anything newer)

- ✅ Basic whole-chassis colour/effect control via the classic `0x5d`
  protocol: **shipped and working**, 5 of 12 built-in modes confirmed live
  (`Static`, `Breathe`, `RainbowCycle`, `RainbowWave`, `Pulse`), the other
  7 confirmed as a real firmware limitation on this specific board (not a
  bug — see `HANDOFF.md` session 3 for the ACK-comparison evidence).
- ❌ Independent per-zone control via `0x04`: **not yet working on Linux**,
  actively being investigated, real progress made (found and replicated
  the exact real pre-`0x04` priming sequence, confirmed it triggers a real
  hardware reaction) but the actual colour-write still doesn't stick. See
  `HANDOFF.md` session 3 Part B and `QUESTIONS.md` for exactly where this
  stands and what's needed next.
