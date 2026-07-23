# Handoff: G615LR Aura patch — pick up here on Linux

Written on Windows, for whoever (human or a fresh Claude session with no
memory of the Windows conversation) continues this on the actual Linux boot.
If you're an AI reading this cold: read this whole file before touching
anything, then read `docs/g615lr-aura-protocol.md` for the full protocol
writeup. Don't re-derive any of this from scratch — it's already been
reverse-engineered and live-tested on real hardware (on Windows; this repo's
new Linux code has never run).

## What this is

Laptop: ASUS ROG Strix G16 2025, board name `G615LR`, USB `0B05:19B6`.
`asusctl` (this repo) has zero working chassis-lighting support for this
laptop today. Over a Windows session, the actual USB HID protocol Armoury
Crate uses was fully reverse-engineered by USB capture + live hardware
testing, confirmed working via custom PowerShell tooling, and partially
ported into this repo as a starting point for a real patch. **The Linux
side of that port has never been compiled or run — that's the very next
step.**

This is a local clone. `git log --oneline -4` shows 4 commits ahead of
`origin/main`, none pushed anywhere:
```
<will show HANDOFF commit here after this file is committed>
0afeec6d Add HIDIOCSFEATURE ioctl wrapper and wire it to lightbar_2025
548d19ce Update lightbar_2025 swap table with latest isolated-test result
9796e543 Document G615LR (ROG Strix G16 2025) second Aura protocol
```

## Step 1 — does it even compile?

This has NEVER been checked on Linux. Windows couldn't do it at all (the
workspace depends on `udev`, Linux-only). Run, in the repo root:

```sh
cargo check -p rog_platform
cargo check -p rog_aura
cargo check -p asusd
```

Fix whatever breaks. Likely suspects, roughly in order of likelihood:
- `nix` crate version/feature mismatch (`Cargo.toml` added
  `nix = { version = "^0.29", features = ["ioctl"] }` — was never resolved
  against a real lockfile, just hand-typed based on nix's docs from memory)
- `nix::ioctl_readwrite_buf!` macro usage in `rog-platform/src/hid_raw.rs`
  — the macro signature/import path may have drifted from what's in
  `nix = "0.29"`; check `nix::ioctl_readwrite_buf!` docs if it errors
- Borrow/lifetime issues around `self.file.try_borrow()` + `AsRawFd` in the
  new `set_feature_report` method
- `use std::os::fd::AsRawFd;` — confirm this is the right import path (not
  `std::os::unix::io::AsRawFd`) for the Rust edition/toolchain this repo
  pins (`rust-toolchain` file at repo root)

## Step 2 — does it actually control the hardware?

Once it compiles, the real test. Nothing in this repo calls the new code
yet (see "What's NOT done" below), so you'll need to write a throwaway
test — a `#[test]`, a tiny `fn main()` in `rog-platform/examples/`, or just
a `cargo run --bin asusctl` hack — that:

1. Opens the right `/dev/hidrawN` for VID `0B05` PID `19B6`. On Windows the
   report-`0x04` traffic lived on the `MI_01` interface (separate from
   `MI_00` which carries the legacy `0x5d` protocol) — Linux's `hidraw`
   enumerates differently, so **don't assume** which `/dev/hidrawN` node is
   right. `HidRaw::new` currently grabs the *first* match by `idProduct`
   only, which may or may not be correct here — this was flagged as an open
   problem in `docs/g615lr-aura-protocol.md` item 1 and was never resolved.
   If the first-match node doesn't work, try each `/dev/hidrawN` with
   matching `idProduct` in turn.
2. Calls `HidRaw::set_feature_report()` with a single-zone packet from
   `build_lightbar_2025_packet()` (see `rog-aura/src/lightbar_2025.rs`) —
   start with **one obviously-visible zone, one obviously-visible color**
   (bright red or bright green on `Lightbar2025Zone::Keyboard1`, say), with
   everything else untouched. Don't start with a batch/animation — the
   Windows discovery process only worked because each step was isolated to
   one variable at a time. Follow the same discipline here.
3. Note: `hidraw` device nodes are typically root-only or need a udev rule
   for user access. You'll probably need `sudo` for this first test, or set
   up a `plugdev`/`uaccess`-style udev rule.

If step 2's packet produces **no visible effect**: don't assume the Rust
code is wrong before checking the boring explanations first — permission
denied on the ioctl (check the actual error, don't swallow it), wrong
`/dev/hidrawN` node, or file opened without read+write (this was fixed in
`HidRaw::new`/`from_device`, but double check if you're bypassing those
constructors in your test).

If it works: huge deal, that's the first-ever confirmation this protocol
works over real Linux `hidraw`, not just Windows `HidD_SetFeature`. Update
this file and `docs/g615lr-aura-protocol.md` to say so.

## Step 3 — re-verify the color swap table (the one real open question)

`Lightbar2025Zone::needs_grb_swap()` in `rog-aura/src/lightbar_2025.rs`
currently says only `SideLeftFront`/`SideRightBack` need a G/R channel
swap, all other zones take plain RGB. **This flip-flopped once already**
during Windows testing — the back bar/back corners tested as needing the
swap in one isolated session (using pure Red, a channel-revealing color),
then tested as NOT needing it in a later session (using Saffron
`#FF9933`, also channel-revealing). Both tests were methodologically sound
in isolation; they can't both have been right for unchanging hardware.

Leading theory, never confirmed: Armoury Crate's Windows background
services (`ArmourySwAgent`, `LightingService`, `ROGLiveService`, etc.)
were never successfully killed during Windows testing — `Stop-Service
-Force`, even elevated, silently no-op'd (same PIDs before/after) — and
may have been racing writes to the same USB interface, corrupting one of
the two test sessions. **On Linux this whole class of interference is
moot** — Armoury Crate doesn't exist there, so this is actually the
cleanest possible environment to settle this permanently.

To re-verify: for each of the 16 zones in `Lightbar2025Zone::ALL`, light
it alone with pure Red (`FF0000`), note whether it displays as red or
green. Then repeat with pure Green (`00FF00`) as a cross-check. **Do not
use Blue, Yellow, or White for this** — they're mathematically invariant
under an R/G channel swap and will look identical either way, proving
nothing (this mistake was made and caught once already on Windows).
Update `needs_grb_swap()` with whatever you find — trust this Linux result
over the Windows one; Linux removes the Armoury Crate variable entirely.

## What's NOT done (don't assume otherwise)

- **No dispatch wiring.** `Aura::write_lightbar_2025()` in
  `asusd/src/aura_laptop/mod.rs` exists and is a complete, self-contained
  method, but nothing calls it. `write_current_config_mode` /
  `write_effect_and_apply` still dispatch purely by `AuraDeviceType` and
  have no G615LR-aware branch.
- **No D-Bus exposure.** No CLI/GUI can reach this yet.
- **Config model doesn't fit.** `AuraEffect` (the existing per-mode config
  type) carries 1-2 colors. This protocol needs 16 independent per-zone
  colors. Wiring real dispatch needs either a new config/D-Bus shape or a
  translation layer — this is real design work, not a stub-fill.
- **No firmware animation engine on this laptop at all** (confirmed via a
  real Armoury Crate USB capture, `alien.pcapng`, analyzed on Windows) —
  even Rainbow/Breathing/StarryNight are host-computed and streamed by
  Armoury Crate continuously, there's no onboard effect engine to trigger.
  Any Linux animation support needs the same approach: a background
  loop re-sending `0x04` packets with a computed color per frame. The
  right architectural home is `rog-aura::effects` (`EffectState` trait,
  same shape as the existing `Breathe` effect) — completely unbuilt on the
  Linux side, only prototyped as PowerShell in `usb_capture/aura_animate.ps1`
  on the Windows side (Rainbow, StarryNight, Breathe all confirmed working
  live there).
- **Legacy `0x5d` protocol confirmed non-functional on this hardware**,
  exhaustively (both Feature and Output report, both Static and
  RainbowCycle) — don't waste time trying it again, see
  `docs/g615lr-aura-protocol.md` for the evidence.

## Reference material in this repo

- `docs/g615lr-aura-protocol.md` — the full protocol writeup: byte layout,
  zone ID table, transport details, what's confirmed vs. open.
- `rog-aura/src/lightbar_2025.rs` — the packet builder + zone enum + swap
  table, with unit tests (`cargo test -p rog_aura lightbar_2025` once it
  compiles).
- `rog-platform/src/hid_raw.rs` — the new `set_feature_report` /
  `HIDIOCSFEATURE` ioctl code.
- `asusd/src/aura_laptop/mod.rs` — `write_lightbar_2025`, the (currently
  orphaned) call site.

## Windows session 2 — found the actual handshake (untested, don't overwrite Linux-side changes with this file)

**Important:** this Windows-side clone is now behind the Linux copy — a Linux
Claude session already made real code changes there (fixed
`set_feature_report`'s silent-failure bug, added `HidRaw::from_devnode`, five
test binaries in `rog-platform/examples/`) that were never synced back here.
**Do not overwrite the Linux repo with this one.** Only bring over the new
file this section references: `usb_capture/handshake_transcript.tsv`
(sibling folder to this repo, same Drive/copy root).

### What was found

With Armoury Crate's services genuinely disabled (`sc.exe config <name>
start= disabled`, not just `Stop-Service`, which silently no-ops) and a
reboot to reach a real EC-owned baseline, then a fresh USBPcap capture
across the services being re-enabled, the actual init sequence Armoury
Crate performs was captured for the first time. It happens entirely on
**interface 0** (not interface 1, which is all every prior test targeted):

1. `t≈21s`: `SET_REPORT`, Report ID `1`, **Output**, 2 bytes: `01 01`.
2. `t≈31-42s` (an ~11.5s burst): a real **read/write negotiation** on
   Report ID `0x5d` as a **Feature** report — not the known-dead `b3`
   Output-report traffic (which is separate, confirmed still just
   vestigial noise sent throughout the whole session). ~60 writes and ~42
   `GET_REPORT` reads, interleaved. Reading the report back doesn't return
   zeros or an error — it echoes real data, e.g. write `5d 05 20 31 00 10
   00 00...` gets back `5d 05 20 31 00 10 03 01 01 02 25 05 01 02 46 03 11
   01 0c 00...`. One of the writes in this burst is literally the ASCII
   string **"ASUS Tech.Inc."** (`5d 41 53 55 53 20 54 65 63 68 2e 49 6e 63
   2e 00...`) — a vendor-identification string, sent multiple times. This
   is strong evidence the negotiation is a **deterministic state machine**,
   not a cryptographic challenge-response with session-specific values —
   there'd be no reason to send a fixed literal string as part of a real
   nonce exchange.
3. Buried inside that same burst, sent **exactly once** in the whole
   ~130-second capture (vs. dozens of repeats for everything else): `t≈33.66s`,
   `SET_REPORT`, Report ID `0x5a` (**never seen in any prior capture or
   document**), Feature, 64 bytes: `5a ba c5 c4 01 00 00 00...`. Being
   singular rather than repeated is what makes this the strongest
   candidate for the actual "hand control to host" command — everything
   else in the burst looks like capability/version polling.
4. Only *after* all of the above (many seconds later, once traffic on
   interface 1 begins) do the already-known `0x05` (Feature, 10 bytes) and
   `0x04` (Feature, 51 bytes, the color protocol) packets appear.

Full exact transcript (frame number, relative timestamp, wValue, full hex
payload, tab-separated) saved to `usb_capture/handshake_transcript.tsv`,
110 lines, every interface-0 write in this capture in chronological order.
That file is the source of truth — the summary above is a compression of
it, don't hand-transcribe from prose.

One important context clue, from the human tester, not derived from the
capture: the EC's baseline state right after reboot (before any of this
starts) had *not* actually been touched yet this boot — but this specific
capture's Armoury Crate instance restored a remembered "Dark" profile,
which is consistent with (but doesn't prove) the `0x5a` payload possibly
encoding *which* profile to restore rather than being a universal unlock
constant. Untested. Also worth noting explicitly: the `0x5d` Output-report
"`b3`" vestigial packet (`5d b3 00 02 00 00 00 eb...`, confirmed dead on
its own in prior testing) appears interleaved in this burst too — include
it in a faithful replay even though it does nothing alone, in case
ordering matters.

### The plan, next time on Linux

Don't try to hand-pick which 3-4 packets matter — replay it faithfully:

1. Read `usb_capture/handshake_transcript.tsv` in order.
2. Send every write in it as a raw USB control transfer (same approach as
   `g615lr-raw-usb-test.rs`: detach `hid_asus` from interface 0 first,
   `bmRequestType=0x21, bRequest=0x09, wValue=<from file>, wIndex=0`),
   in the same order, exact bytes. `GET_REPORT` reads don't need to be
   replicated with matching data — issuing them or not is a secondary
   question; try without first (writes are what change device state).
   Timing: the real gaps are mostly sub-millisecond to a few ms; sending
   as fast as possible is a reasonable first attempt before trying to
   match delays.
3. **Checkpoint — check for a dark-mode transition before doing anything
   else.** If the theory holds and the last-known state involved "Dark",
   the lights should visibly go black partway through or right after this
   replay. This is the first real pass/fail signal this whole
   investigation has had — don't skip straight to testing color.
4. Only if step 3 shows a change: attempt a `0x04` color packet on
   interface 1 afterward, same as every prior test.
5. If step 3 shows *no* change even with the full faithful replay: the
   `0x5d`/`0x5a` theory is likely wrong (or missing something not
   observable in USB traffic, e.g. a WMI/ACPI-side call happening in
   parallel) — don't keep iterating on this specific packet sequence
   without new evidence, revisit the ACPI/WMI lead from session 1 instead.

## Anything else worth knowing

- This clone's `origin` is `https://gitlab.com/asus-linux/asusctl` — the
  real upstream project. Nothing has been pushed or opened as an MR
  anywhere; that was explicitly left for the human to decide, not something
  to do automatically once this works.
- A sibling board, `G614FR`, shares the "(ROG Strix G16 2025)" label in
  `aura_support.ron` — worth testing if that hardware is ever available,
  since it may speak the same `0x04` protocol.
- The Windows-side tooling (PowerShell scripts that got real hardware
  working, before any of this Rust code existed) lives outside this repo,
  in a `usb_capture` folder alongside it. If that folder made the trip to
  Linux too, its `README.md` is the single most complete writeup of
  everything discovered this whole investigation — more narrative detail
  than `docs/g615lr-aura-protocol.md`, which is the trimmed-for-upstream
  version.
