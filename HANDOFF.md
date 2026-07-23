# Handoff: G615LR Aura patch — pick up here on Linux

Written on Windows, for whoever (human or a fresh Claude session with no
memory of the Windows conversation) continues this on the actual Linux boot.
If you're an AI reading this cold: read this whole file before touching
anything — **especially the "Linux session 1" update at the bottom, which
supersedes several claims above** — then read
`docs/g615lr-aura-protocol.md` for the full protocol writeup. Don't
re-derive any of this from scratch — it's already been reverse-engineered
and live-tested on real hardware (on Windows; the Linux code below has now
compiled and been extensively hardware-tested, see bottom of file, but
still doesn't produce a visible effect — that's the live open problem).

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

## Housekeeping note: a line-ending bug from the Windows→Linux handoff (now fixed)

When this repo made the trip from Windows to Linux (via a Google Drive
copy, not a fresh `git clone`), every tracked file arrived with CRLF line
endings instead of the LF the git blobs actually contain — almost
certainly because the Windows-side git checkout had `core.autocrlf=true`
(or equivalent) converting LF→CRLF on checkout, and the raw checked-out
files were what got copied, not a clean re-clone. The practical symptom on
first opening this repo on Linux: `git status`/`git diff` showed **~200
files as modified**, every single one of them 100% whitespace noise
(verified with `git diff --ignore-space-at-eol`, which showed zero real
differences). This has been fixed on the Linux side
(`git checkout -- .` after confirming no real changes were being
discarded), and the working tree is now clean LF throughout.

**For whoever sets up git checkouts on the Windows side going forward**:
either set `core.autocrlf=input` (checks out LF, converts CRLF→LF on
commit, avoids this class of bug entirely) or `core.autocrlf=false` (no
conversion at all) before checking out this repo, rather than the default
`true`, which is what caused this. Worth a quick check next time you're
setting up a fresh checkout there — not a blocker for anything, just
avoids a repeat of a slightly alarming "200 files changed" moment that
turned out to be nothing.

## Linux session 1 update — compiles clean, hardware-tested extensively, still no visible effect

Everything in "Step 1" above is now done and passed on the real
`G615LR` (`cargo check -p rog_platform -p rog_aura -p asusd`, plus
`cargo test -p rog_aura lightbar_2025` — all green, first try, none of the
predicted suspects hit). The compiled `asusd` was installed as the live
system daemon (`/usr/bin/asusd`, backup at `/usr/bin/asusd.bak-6.3.7`) and
runs stably. **Currently stopped** (`sudo systemctl stop asusd` — it's a
system service, not a user one) as part of debugging; restart with
`sudo systemctl start asusd` if normal daemon function is wanted back.

Two real (non-cosmetic) code changes landed in `rog-platform/src/hid_raw.rs`
beyond what's described in "Step 1"/"docs" above:
- `set_feature_report` used to silently no-op on a failed `try_borrow()`
  instead of erroring — changed to `.borrow()` (panics loudly on conflict
  instead of lying about success). Found by inspection, not yet actually
  triggered by anything.
- Added `HidRaw::from_devnode(path, id_product)` — opens a specific
  `/dev/hidrawN` directly, bypassing `HidRaw::new`'s first-match ambiguity.
  Needed because this laptop has two hidraw nodes under the same
  `idProduct` (`/dev/hidraw1` = `bInterfaceNumber 00`, `/dev/hidraw2` =
  `01`) and `new()` can't tell them apart.

Five throwaway test binaries live in `rog-platform/examples/` (all built
and confirmed compiling; run any with
`sudo target/debug/examples/<name>`, needs root for raw hidraw/USB access):

- `g615lr-lightbar-test.rs` — sends one hand-built zone/color packet via
  `HidRaw::set_feature_report` (the `HIDIOCSFEATURE` ioctl path).
- `g615lr-replay-capture.rs` — same, but the packet bytes are the *literal*
  bytes extracted from `usb_capture/aura.pcap` (a real, visually-confirmed
  Windows capture), not re-derived from the docs — rules out any
  transcription bug in the packet builder.
- `g615lr-raw-usb-test.rs` — bypasses the kernel HID subsystem entirely:
  detaches the `hid_asus` kernel driver from interface 1 via `rusb`
  (libusb) and sends the same captured packet as a raw USB control
  transfer, matching Windows' `HidD_SetFeature` at the wire level exactly
  (`bmRequestType=0x21, bRequest=0x09, wValue=0x0304, wIndex=1`).
- `g615lr-with-handshake.rs` — same raw-USB approach, but first sends a
  previously-undocumented **Feature report ID `0x05`** (10 bytes) that was
  found in `aura.pcap` immediately preceding the first `0x04` packet of
  that capture session — on the theory it's a one-time "enable custom
  lighting" handshake. Payload used: `05 00 08 00 0f 00 00 00 00 01`.
- `g615lr-hold-test.rs` — resends the zone packet continuously for 6
  seconds at ~50fps via raw USB, mimicking Armoury Crate's continuous
  streaming, to rule out "one-shot packet gets overwritten by the next
  frame of a competing animation."

**Result: every single one of the above produces zero visible hardware
effect.** Not "wrong color" — literally nothing changes, ever, on any
zone tried (`Keyboard1`, zone `0x06` back-left corner). This holds on both
an actively-animating rainbow baseline and a static-orange baseline (mode
was changed via a physical hotkey mid-session — confirms an EC-firmware
default owns these LEDs independent of any host software, since no OS-side
tool caused that change).

What's been **definitively ruled out** as the cause, each independently
verified:
1. **Packet content wrong** — `g615lr-replay-capture.rs` sends literal
   captured-good bytes, byte-for-byte. Also independently confirmed the
   zone/color offset layout (bytes 3-4 zone ID, byte 19+ color) against
   `aura.pcap` using a small Python parser — matches
   `build_lightbar_2025_packet` exactly.
2. **Wrong report length** — pulled the *live* HID report descriptor
   straight from this exact hardware
   (`/sys/bus/hid/devices/0003:0B05:19B6.*/report_descriptor`, hand-parsed
   the HID item stream) and confirmed report ID `0x04` really is declared
   as 51 bytes (50 data + 1 ID) and report `0x05` as 10 bytes, matching
   what's sent exactly.
3. **Wrong interface / first-match ambiguity** — tried both
   `/dev/hidraw1` (`bInterfaceNumber 00`) and `/dev/hidraw2` (`01`)
   explicitly via `from_devnode`; also confirmed via `udevadm` that `01`
   is genuinely `MI_01`, matching the docs.
4. **`HIDIOCSFEATURE`/hidraw-specific transport bug** — bypassed entirely
   via raw `libusb` control transfers (`g615lr-raw-usb-test.rs`), same
   null result.
5. **`hid_asus` kernel driver intercepting/filtering the report** — this
   device binds to the in-tree `hid_asus` driver, not generic
   `hid-generic` (`/sys/bus/hid/devices/*/driver` → `asus`). Detached it
   via `rusb::detach_kernel_driver` before sending raw USB — no change.
6. **Missing one-time init/handshake** — found and tried sending report
   `0x05` first (see above) — no change.
7. **Competing continuous animation overwriting a one-shot write** — tried
   continuous 6-second streaming at ~50fps — no change. Also tried on a
   static (non-animating) baseline — still no change.
8. **`asusd` (or anything else on this box) fighting for the device** —
   confirmed `asusd` stopped (`systemctl is-active` → `inactive`) during
   the later tests — no change.

**Leading unresolved theory**: some ASUS-specific ACPI/WMI-level "hand
control to host" call that Armoury Crate's background service issues once
(on Windows this needed `ArmourySwAgent`/`LightingService`/etc. to be
*running*, just set to "Dark mode" — never fully closed — during all
original successful testing, which is exactly consistent with this). Real
findings so far, not just speculation:
- `usb_capture/probe_wmi.ps1` / `probe_wmi2.ps1` reference ASUS's generic
  ATK WMI class `AsusAtkWmi_WMNB` with `DSTS`/`DEVS` methods and candidate
  device IDs (`LIGHTBAR 0x00050025`, `TUF_RGB_MODE 0x00100056`,
  `TUF_RGB_MODE2 0x0010005A`, `TUF_RGB_STATE 0x00100057`) — these were
  *guessed* from other ASUS models' known IDs, never confirmed for
  `G615LR` specifically.
- `usb_capture/wmitrace.etl`/`.xml` (an attempted Windows ETW capture of
  this) is a dead end — only 99 generic session-header events, zero actual
  `Asus`/`WMNB` activity captured. Armoury Crate most likely talks to the
  ATK ACPI device via a direct IOCTL, not through the traced WMI service
  layer, so this file doesn't help.
- The underlying ACPI method **does exist** on this exact machine: decompiled
  the live DSDT (`sudo acpidump -b` + `iasl -d`) and found
  `\_SB.ATKD.WMNB(Arg0, Arg1, Arg2)` — a `Serialized` method dispatching on
  a 4-byte code in `Arg1` (`0x54494E49`="INIT", `0x53545344`="DSTS",
  `0x53564544`="DEVS", plus others), with `Arg2` a 20-byte buffer
  (`CreateDWordField` into `IIA0..IIA4`) — `IIA0` is the device ID. This
  matches the Linux `asus_wmi` driver's own known internal convention
  exactly (same dispatch shape it uses for e.g. `KBD_BACKLIGHT`).
- Installed `acpi-call-dkms` and probed `DSTS` (read-only status query,
  `Arg1=0x53545344`, `IIA0=<device id>`) for all four candidate lighting
  IDs above, **plus `KBD_BACKLIGHT (0x00050021)` as a sanity check** since
  that ID is confirmed working today via the existing
  `/sys/class/leds/asus::kbd_backlight` sysfs control (driven by the
  in-tree `asus_wmi` kernel driver, which must be calling this exact same
  ACPI method successfully under the hood). **All five, including the
  known-working sanity check, returned `0xFFFFFFFE`** — ASUS's own
  standard "unsupported device ID" sentinel. This is ambiguous: either
  none of these IDs are real on this firmware (plausible for the 4 guessed
  ones, **not plausible for `KBD_BACKLIGHT`**), or the raw `acpi_call`
  invocation has an encoding bug (wrong arg width/type, or `acpi_call`
  not respecting the method's `Serialized` locking) that makes every call
  fail before it even reaches the `IIA0` comparison. Getting the
  known-working ID to also come back "unsupported" points at #2, but this
  was not resolved before pausing.

**Concrete next steps, in order of likely value**:
1. Get a **fresh Windows USB *and* WMI capture bracketing the actual
   handoff moment** — cold boot or a fresh Armoury Crate launch from a
   state where lighting is EC-owned (not just color changes within an
   already-controlled session, which is all every existing capture in
   `usb_capture/` shows). This is the one thing no existing artifact
   covers and would directly confirm or kill the WMI-handoff theory.
2. Debug the `acpi_call` encoding until `KBD_BACKLIGHT`'s `DSTS` probe
   returns something other than `0xFFFFFFFE` (a real status value) —
   proves the call mechanism itself works, at which point the same
   mechanism against `LIGHTBAR`/`TUF_RGB_MODE`/`TUF_RGB_STATE` becomes
   trustworthy. Candidates for what's wrong: `Arg0`'s actual purpose
   (hardcoded to `0` throughout, never confirmed), whether `acpi_call`
   needs integers passed with explicit width, whether the buffer literal
   syntax `{0x21, 0x00, ...}` is being parsed the way expected.
3. If a real "enable custom lighting" `DEVS` call is ever found this way,
   note it'll need a `DEVS` invocation (not just `DSTS`), which is a
   **write**, not read-only — treat with more caution than the probes
   above.
4. Don't re-try anything from the "ruled out" list — it's exhaustively
   covered and reproducible via the five example binaries above.

## Linux session 2 update — real breakthrough: basic keyboard color control WORKS

A second Windows-side Claude Code session (working in parallel, different
boot of the same physical laptop) captured a fresh interface-0 handshake
sequence and handed it over — see `usb_capture_session2/` (its own note,
`NOTE_FROM_WINDOWS_CLAUDE.md`, and the raw transcript,
`handshake_transcript.tsv`). Replaying that sequence (see
`g615lr-iface0-handshake-replay.rs` and the shorter
`g615lr-core-handshake-then-color.rs` in `rog-platform/examples/`) produced
the first-ever *real, visible* reaction from the hardware in this whole
investigation — static orange transitioning to rainbow during the replay,
reverting when it stopped — but never actually unlocked `0x04` color
control. That thread is **not the actual fix** (see below for what was) but
is preserved since it's real signal, just not the relevant signal.

**The actual breakthrough came from a completely different angle**: using
`rog-control-center` (the GUI, already in this repo) to change modes, while
capturing with `usbmon`, showed *real, working* traffic on the classic
`0x5d` protocol — direct contradiction of the original Windows
investigation's "confirmed non-functional" finding for that protocol.
Chasing why the GUI worked but `asusctl` (CLI) didn't led to the actual
root causes, both mundane:

1. **The installed `asusctl` CLI (`v6.3.7`) and the patched `asusd`
   daemon (`v6.3.8`, built this session) were version-mismatched.** The
   old CLI was silently failing to get color-set requests through — no
   error surfaced, it just did nothing. Rebuilding `asusctl` from this
   same repo (`cargo build --release -p asusctl`, matching `asusd`'s
   version) immediately fixed this. (One build hiccup along the way: a
   stale/corrupted incremental artifact in `target/release/` produced an
   all-zeros non-ELF binary and a bogus "panic runtime" link error on the
   first attempt — resolved by clearing just the affected `target/release/
   deps/{asusd,asusctl}*` files and rebuilding, no full `cargo clean`
   needed.)
2. **This hardware silently drops short `0x5d` Output-report writes.**
   `write_effect_and_apply` in `asusd/src/aura_laptop/mod.rs` (lines
   ~105-123) already pads every `0x5d` write to the full 64-byte Output
   report size declared in the HID descriptor — a fix that predates this
   investigation entirely, added for a different laptop (`G533QS`, per the
   inline comment) that happens to also fix `G615LR`. The original Windows
   investigation almost certainly tested with the shorter, unpadded
   17-byte (`AURA_LAPTOP_LED_MSG_LEN`) packets and got silently ignored —
   hence "confirmed non-functional," which was true only for that specific
   (unpadded) attempt, not the protocol in general.

**Confirmed working, live, reproducibly**: `asusctl aura effect static -c
<hex>` now visibly sets keyboard color (tested red, blue, green, all
worked) via the **existing, unmodified upstream dispatch** — no G615LR
patch code involved at all. Covers `AuraEffect`'s `Static`/`Breathe`/
`RainbowCycle`/`RainbowWave` modes and the 4 keyboard zones (`Key1-4`) per
`aura_support.ron`'s existing entry. `asusctl`/`asusd` are now installed
system-wide as matching versions (`/usr/bin/asusctl`, backup at
`/usr/bin/asusctl.bak-6.3.7`, alongside the earlier `/usr/bin/asusd.bak-
6.3.7`).

**The `0x5a` "handshake" mystery is also resolved, and turned out to be
unrelated to any unlock sequence**: it's not constructed anywhere in this
Rust codebase (`grep` across `rog-platform`/`rog-aura`/`asusd` for `0x5a`
finds nothing). `set_led_mode_data`'s handler always calls `set_brightness`
right after writing the effect, which goes through
`rog_platform::keyboard_led::KeyboardBacklight` — a **plain sysfs write**
to `/sys/class/leds/asus::kbd_backlight/brightness`. The kernel's own
`hid_asus` driver is what turns that sysfs write into the `0x5a` HID
report, entirely inside the kernel, invisible to any userspace code here.
The "singular mysterious `0x5a` packet" in the original Windows capture was
almost certainly Armoury Crate syncing keyboard brightness as a routine
side effect of a mode change, not a special "enable custom lighting"
handshake. The entire ACPI/WMI investigation (`acpi_call`, DSDT
decompilation, `\_SB.ATKD.WMNB`) in the "Linux session 1" section above was
a reasonable hypothesis at the time but is now understood to be chasing the
wrong mechanism — harmless (all read-only probes), just not the answer.

**What this does and does not resolve**:
- ✅ Basic single/dual-color keyboard effects (4 zones, `Key1-4`) — solved,
  works today, zero new code needed.
- ❌ The actual goal of this whole patch — independent per-zone color
  across all 16 zones including the 12 chassis/lightbar segments via the
  new `0x04` protocol (`rog-aura::lightbar_2025`,
  `Aura::write_lightbar_2025`) — **still unresolved**. The classic `0x5d`
  protocol's `Key1-4` zones don't reach the chassis lightbar at all; this
  is genuinely separate hardware/protocol territory. Every finding in
  "Linux session 1" about `0x04` producing zero visible effect still
  stands — nothing in session 2 changed that. The `0x5a` red herring does
  NOT need to be sent before `0x04` packets; drop it from any future
  `0x04` test sequences.

**Suggested next step for the `0x04`/chassis-lightbar goal specifically**:
now that a real, working, padded-Output-report precedent exists for `0x5d`,
worth checking whether `0x04` (a **Feature** report, different type) has
an analogous "must match declared size exactly, silently dropped
otherwise" requirement that's already satisfied (51 bytes was confirmed
against the live descriptor in session 1, so probably not this) — or
whether `HidRaw`'s `HIDIOCSFEATURE` path has some other subtle mismatch
against how `write_bytes`'s Output-report path succeeds. Given how mundane
the actual `0x5d` fix turned out to be (padding + version match, not a
handshake), it's worth re-examining `0x04` for an equally mundane
explanation before assuming another deep protocol mystery.

## Linux session 3 update — 12-mode verification, and the closest look yet at 0x04

Written 2026-07-23, ~11:50 IST. Timeline below is reconstructed from real
timestamps (git commit times, file mtimes on the test binaries as each was
written and immediately run) — not estimated after the fact:

| When (2026-07-22/23, IST) | What |
|---|---|
| 07-22 16:06 – 16:11 | Windows handoff commits land: `0afeec6d` (HIDIOCSFEATURE + lightbar_2025 wiring), `97fb9ff5` (HANDOFF.md) |
| 07-22 20:29 – 20:43 | Linux session 1: first hardware tests (`g615lr-lightbar-test.rs` → `g615lr-with-handshake.rs`) — packet content, transport, interface, `hid_asus` driver, timing all ruled out as the cause of `0x04` producing no visible effect |
| 07-22 21:47 – 21:57 | Linux session 2: Windows-side interface-0 handshake (from a *different*, mode-cycling capture) replayed (`g615lr-iface0-handshake-replay.rs`, `g615lr-core-handshake-then-color.rs`) — real rainbow reaction confirmed, colour still not unlocked |
| 07-23 10:56 | Commit `147fbcc6` — sessions 1+2 findings, test binaries, and the CRLF-noise cleanup landed |
| 07-23 11:13 | `g615lr-5d-then-04.rs` — proven `0x5d` static-colour immediately followed by `0x04`; discovered `0x5d` drives the *whole chassis*, not just keyboard |
| 07-23 11:20 – 11:35ish | 12-mode live verification via `asusctl aura effect <mode>` (not a standalone test binary, done via CLI) — 5 of 12 confirmed working |
| 07-23 11:28 | `g615lr-mode-compare.rs` — Pulse-vs-Comet ACK comparison, confirms the 7 failing modes are a real firmware gap, not a packet-construction bug |
| 07-23 11:37 | `g615lr-real-priming-sequence.rs` — ground-truth priming sequence mined directly from `aura.pcap`, replayed exactly; confirms the "dead/vestigial" `5d b3 00 02...` packet is real (triggers genuine RainbowCycle) |
| 07-23 11:40 | `g615lr-prime-then-stream.rs` — priming + 8s continuous `0x04` streaming; still stuck on rainbow, current dead end |
| 07-23 ~11:50 | This section written; `QUESTIONS.md` and `CLAUDE.md` added; repo pushed to the shared GitHub remote for Windows to pull from directly |

**IMPORTANT FRAMING, read this before anything else in this section**: the
`0x04`/per-zone chassis lightbar problem is **not a hardware limitation**.
This is not speculation — it was directly, repeatedly, reproducibly
demonstrated on Windows: individual zones were painted different colours
simultaneously, a custom India-flag layout was built with the physical
chassis split into three colour bands, and a live breathing animation was
run on just two specific zones (`kbd2`/`kbd3`, the "chakra") while the rest
stayed static — all captured on video, all repeatable, all via the exact
`0x04` protocol this repo implements. Whatever is blocking this on Linux is
a **gap in our own understanding or code**, not a ceiling the hardware
imposes. Every future session picking this up should start from that
premise, not from "maybe it just doesn't work on Linux."

### Part A: `basic_modes` widened and empirically verified (12 legacy `0x5d` modes)

With the `0x5d` breakthrough from session 2 in hand, `aura_support.ron`'s
`G615LR` entry was temporarily widened from the original conservative
4-mode list to the full 12 (matching `G634J`/`G635L`), then every mode was
tested live, one at a time, via `asusctl aura effect <mode> ...`:

**Confirmed working** (5): `Static`, `Breathe` (colour1 only — `colour2` is
silently ignored by this hardware/firmware, worth fixing in the CLI/UI
expectations but not a blocker), `RainbowCycle` (genuinely animates,
autonomously, continuously — the whole chassis, not just keyboard),
`RainbowWave`, `Pulse`.

**Confirmed NOT working** (7): `Star`, `Rain`, `Highlight`, `Laser`,
`Ripple`, `Comet`, `Flash` — tried individually, zero visible effect each.

`aura_support.ron` has been corrected back down to just the 5 verified
modes (not left at 12) so the CLI/GUI don't offer options that silently
no-op. See the inline comment on the `G615LR` entry for the full rationale.

Also confirmed live: **the classic `0x5d` protocol drives the entire
chassis as one unit** (keyboard + full lightbar together, matching
`power_zones: [Keyboard, Lightbar]`) — there is no independent per-zone
control through this protocol, only a single global colour/effect. This
was discovered by accident: a combined "0x5d then 0x04" test turned the
keyboard blue as expected, and the chassis corner turned blue too, even
though the follow-up `0x04` packet asked for red on that specific zone —
i.e. the `0x5d` write alone accounted for the whole visible result, and the
`0x04` write on top of it did nothing detectable.

**Is "7 modes don't work" a code bug or a real firmware gap?** Checked
directly, not assumed. `AuraModeNum`'s enum values
(`rog-aura/src/builtin_modes.rs:260`) are `Static=0, Breathe=1,
RainbowCycle=2, RainbowWave=3, Star=4, Rain=5, Highlight=6, Laser=7,
Ripple=8, [value 9 is skipped entirely], Pulse=10, Comet=11, Flash=12` —
note the gap at 9. The working set is exactly `{0,1,2,3,10}`; the failing
set is exactly `{4,5,6,7,8,11,12}`. Built a comparison test
(`rog-platform/examples/g615lr-mode-compare.rs`, uses the REAL
`AuraEffect`→bytes conversion from `rog-aura`, not hand-rolled bytes) that
sends a working mode (`Pulse`) and a failing one (`Comet`) back to back
with a `usbmon` capture running. Result: **both get byte-for-byte identical
ACK sequences from the device** (`5d ec b3` / `5d ec b5` / `5d ec b4` on
the interrupt-IN endpoint, once per command, for both). The only
difference between the two packets is a single byte (the mode number).
Since the device acknowledges both identically, this looks like a genuine
firmware limitation on this specific 2025-refresh EC (smaller mode table
than `G634J`/`G635L`) rather than anything wrong in how the packets are
built or sent. Live side-observation: sending `Comet` while `Pulse` was
mid-animation didn't switch to Comet's colour, it just froze Pulse's
animation on its last frame — consistent with the firmware accepting the
command structurally (enough to interrupt whatever it was doing) but
having no actual handler for mode 11 to hand off to.

### Part B: the closest look yet at why `0x04` doesn't work — real progress, not yet solved

Two new things found this session, both from directly mining the real,
working `usb_capture/aura.pcap` capture (not guessing):

**1. `0x04` never gets an interrupt-IN ACK — but neither does it on
Windows, even when working.** Checked directly: in `aura.pcap`, the
nearest interrupt-IN packet after any real, working `0x0304` SET_REPORT is
17-19 **seconds** later, and it's just the generic idle heartbeat, totally
unrelated in timing. So the absence of an ACK for `0x04` on Linux (checked
via `rog-platform/examples/g615lr-raw-usb-test.rs` + a `usbmon` capture) is
**not** diagnostic of failure — it's normal behaviour for this report on
any OS. Ruled out cleanly, not just assumed.

**2. Found and replicated the EXACT wire sequence that precedes the first
successful `0x04` write in a real session** — extracted by chronologically
scanning every control transfer in `aura.pcap` before that first write,
not reconstructed from theory:

```
SET_IDLE            iface 1
SET_IDLE            iface 0
SET_REPORT 0x0201   "01 01"                        iface 0  (2 bytes)
SET_REPORT 0x025d   "5d b3 00 02 00 00 00 eb..."    iface 0  (64 bytes, padded)
SET_REPORT 0x025d   "5d b4 00..."                   iface 0  (64 bytes, padded)
SET_REPORT 0x025d   "5d b5 00..."                   iface 0  (64 bytes, padded)
SET_REPORT 0x0305   "05 00 08 00 0f 00 00 00 00 01" iface 1  (10 bytes)
SET_REPORT 0x0304   <zone data>                     iface 1  <- the real write
```

Two important corrections to earlier assumptions this uncovered:
- The `5d b3 00 02 00 00 00 eb` packet is the exact one the *original*
  Windows investigation dismissed as "dead/vestigial, always identical
  regardless of mode" (see `usb_capture/README.md`'s "No firmware effects"
  section). **It is not dead.** Its mode byte (`02`) is a real
  `AuraModeNum::RainbowCycle` value, and replaying just this priming
  sequence (`rog-platform/examples/g615lr-real-priming-sequence.rs`) 
  visibly puts the ENTIRE chassis into genuine, continuous RainbowCycle
  animation on Linux, live-confirmed. It's real, it's just not what it was
  taken for — it's routine session-priming boilerplate that happens to be
  interpretable as (and does trigger) a real global mode-set, sent once
  per session, not something to skip as inert.
- The real `b3`/`b4`/`b5` order in this priming sequence is `b3, b4, b5`
  — **not** `b3, b5, b4`, which is the order `write_effect_and_apply` in
  `asusd/src/aura_laptop/mod.rs` and every prior `0x5d` test in this repo
  used. Worth a closer look at whether order matters for the priming
  triplet specifically (it apparently doesn't matter for the *effect*
  triplet, since `b3,b5,b4` demonstrably works for real colour-setting —
  but this is a different, one-time-per-session packet, not necessarily
  governed by the same rule).

**Chronological analysis of the full capture confirms this priming
sequence is sent exactly ONCE per session**, at the very start, never
repeated — followed by a continuous, rapid stream of `0x04` zone writes
(roughly every 200-800ms, for the entire ~40+ second window examined,
cycling through different single/multi-zone combinations, consistent with
either a live demo cycling zones or a host-computed animation).

**Tested, in order**:
1. Priming sequence + single one-shot `0x04` write
   (`g615lr-real-priming-sequence.rs`): chassis visibly enters RainbowCycle
   (confirming the priming packet is real), the single `0x04` write after
   it has no visible incremental effect — corner never shows the requested
   colour.
2. Priming sequence + continuous `0x04` streaming for 8 seconds at ~4/sec
   (`g615lr-prime-then-stream.rs`), on the theory that Windows' own
   continuous stream is what overrides/suppresses the RainbowCycle the
   priming triggers: **still stuck on rainbow for the full 8 seconds**,
   never resolved to the requested colour.

So the theory that "continuous streaming after priming is sufficient" did
**not** pan out as tested — this is a real negative result, not yet
explained. Open possibilities, none confirmed:
- Streaming rate/duration insufficient (Windows' actual rate right after
  priming was not independently re-measured beyond the general
  200-800ms figure — worth checking the first few post-priming writes
  specifically, they may be denser/faster than the steady-state rate later
  in the capture).
- Something Linux-side about the detach/reattach or multi-interface
  claim/release cycle introduces enough latency between priming and the
  start of streaming to matter, where Windows' single persistent handle
  wouldn't. Or something else entirely, not yet identified, that only
  shows up once you're actually mid-stream (nothing in this session tested
  what a much longer stream, e.g. 30-60s, does — 8s may simply not be
  enough if the EC has its own multi-second internal timeout/settle
  behaviour).
- `SET_IDLE` on interface 1 fails with `Err(Pipe)` (STALL) on this
  hardware in every test this session — interface 0's `SET_IDLE` succeeds.
  This is presumably benign (many HID devices don't implement `SET_IDLE`
  for Feature-only interfaces and STALLing it is normal/expected), but it
  was never independently confirmed as harmless — worth checking whether
  Windows' `SET_IDLE` on interface 1 also fails/is skipped, or succeeds
  differently.
- The specific zone/colour data in the `0x04` packets being streamed was
  NOT varied to match what the real capture's own stream was doing
  (cycling through many different zones per packet) — every Linux test
  this session sent the exact same single zone (`0x06`, red) repeatedly.
  Worth trying to replicate the ACTUAL cycling pattern from the capture
  (see the zone-ID sequence in the "Part B" write-up above) instead of one
  static zone, in case the EC's firmware expects to see zone IDs actually
  changing to recognize "an active per-zone session is in progress."

### Reproducible test binaries (all in `rog-platform/examples/`, run via `sudo target/debug/examples/<name>`)

- `g615lr-lightbar-test.rs`, `g615lr-replay-capture.rs`,
  `g615lr-raw-usb-test.rs`, `g615lr-with-handshake.rs`,
  `g615lr-hold-test.rs` — session 1 tests, see that section.
- `g615lr-iface0-handshake-replay.rs`,
  `g615lr-core-handshake-then-color.rs` — session 2's Windows-handshake
  replay tests (a DIFFERENT capture/handshake than session 3's, from mode-
  cycling rather than zone-painting — produced a real rainbow reaction but
  never unlocked colour either).
- `g615lr-5d-then-04.rs` — proven `0x5d` static-colour sequence immediately
  followed by a `0x04` zone write (session 3). Confirmed the whole-chassis
  finding above.
- `g615lr-mode-compare.rs` — Pulse-vs-Comet ACK comparison (session 3 part A).
- `g615lr-real-priming-sequence.rs` — the ground-truth priming sequence
  extracted from `aura.pcap`, one-shot `0x04` write after (session 3 part B).
- `g615lr-prime-then-stream.rs` — same priming, then 8s of continuous
  `0x04` streaming (session 3 part B).

### For whoever picks this up next (any OS, any session)

Do not conclude `0x04` is unsolvable. The hardware proof from Windows is
solid and repeatable. The most promising untried angles, in rough priority
order:
1. A **much longer** stream after priming (30-60s+, not 8s) — cheap to
   test, rules out a settle-time theory.
2. Replicate the ACTUAL cycling-zone pattern from the capture during the
   stream, not one static zone.
3. Get a fresh Windows capture that specifically instruments/logs exactly
   when (wall-clock, relative to the priming sequence) the FIRST visible
   colour change happened, to get a real target latency/rate to match,
   rather than inferring it from packet spacing alone.
4. Consider capturing with `usbmon` running continuously across a full
   priming+stream Linux test (not just checking before/after) to see the
   complete interrupt-IN timeline during the stream itself, not just
   immediately after — may reveal periodic traffic during sustained
   streaming that a short single-shot check would miss.

## Windows session 1 — closed the missing-`usb_capture` gap, answered Q3/Q5, exact priming-sequence timing

Written 2026-07-23. Picked this up via a human relaying messages between the
two sessions (no direct channel), then switched to working from this repo
directly once it existed.

**Housekeeping fix**: `usb_capture/` (the original session-1 raw data —
`aura.pcap` and friends, `aura_control.ps1`/`aura_animate.ps1`/
`HidSend.cs`, every `.pcap`/`.pcapng`) had never actually been committed to
this repo, despite being referenced constantly throughout this file and
`QUESTIONS.md` — it only ever existed as a local-only copy on each machine
from an earlier ad-hoc Drive/zip handoff, which quietly broke the "git is
the only shared channel" model `CLAUDE.md` describes. Added and pushed
(`1eb3410b`). If anything in it looks different from what you remember
using locally (timestamps, an extra file, whatever) — that's expected, fix
it forward in a new section rather than treating this one as wrong; this
was reconstructed from a local scratch copy, not guaranteed byte-identical
to whatever copy Linux sessions 1-3 were actually reading from.

**Q3 answered, no new test needed**: already had this in an existing
capture. `SET_IDLE` on interface 1 **succeeds** on Windows
(`USBD_STATUS_SUCCESS`) — doesn't `STALL` the way it consistently does on
Linux. Real platform difference, not something to wave off as benign
without checking, which is exactly why the question was worth asking.

**Q5 answered, no new test needed**: `aura_control.ps1` opens a fresh HID
handle per write; `aura_animate.ps1` holds one persistent handle for an
entire session. Both are confirmed working live, on real hardware, for
real color control (`aura_animate.ps1`'s persistent handle exists for
*performance* at 20-30fps, per its own code comment calling per-frame
handle churn "wasteful" — not because the churn broke correctness). Handle
lifecycle is very unlikely to be the `0x04` blocker.

**Exact priming-sequence bytes and timing, pulled directly from
`aura.pcap` via `tshark` (not re-typed from prose)**:

```
t=7.791911  SET_IDLE  iface 1
t=7.791934  SET_IDLE  iface 0
t=7.793118  SET_REPORT 0x0201  "01 01"                                    iface 0
  ── ~4.08s gap ──
t=11.875611 SET_REPORT 0x025d  "5d b3 00 02 00 00 00 eb 00...(64B)"       iface 0
t=11.877360 SET_REPORT 0x025d  "5d b4 00 00...(64B, all zero after b4)"   iface 0
t=11.879505 SET_REPORT 0x025d  "5d b5 00 00...(64B, all zero after b5)"   iface 0
t=11.916336 SET_REPORT 0x0305  "05 00 08 00 0f 00 00 00 00 01"            iface 1
t=11.917548 SET_REPORT 0x0304  <first real write, 8-zone batch>           iface 1
t=12.690948 SET_REPORT 0x0304  <second write>                             iface 1
t=12.938433 SET_REPORT 0x0304  <third write>                              iface 1
```

Confirms the `b3`/`b4`/`b5` bytes Linux session 3 extracted are exactly
right (independently re-derived, not just trusted). New information this
adds: **the gap from the last priming packet (`0x0305`) to the first real
`0x0304` write is ~1.2 milliseconds** — essentially immediate, no
deliberate delay. The gap from `b3` (first priming write) to the first
color write is ~42ms total. Steady-state write cadence after that is
roughly 250-770ms between writes (matches the earlier "200-800ms"
estimate). **This weakens the "Linux just didn't wait long enough" theory**
— if real Windows needs ~0ms of settle time between the last priming
packet and a working color write, an 8-second Linux stream timing out
unresolved is unlikely to be explained by "priming needs more time to take
effect internally"; if it were a pure timing/settle issue you'd expect
Windows to need a real gap too, and it doesn't.

Also worth flagging: the first real `0x0304` write is an **8-zone batched
packet** (`04 08 01 00 00 01 00 02 00 03 00 04 00 05 00 06 00 07 00 ...`),
not a single-zone write. Every Linux test so far (per `QUESTIONS.md` Q2)
streamed one static single zone. Combined with the "does zone variety
matter" open question, this is one more data point toward testing with
real multi-zone batches instead of a lone zone — worth trying before or
alongside the single-zone Q1/Q2 test below.

**In progress**: a controlled Q1+Q2 test — replay this exact priming
sequence via `HidSend.cs` directly (bypassing Armoury Crate's GUI
entirely, so timing is fully under script control), immediately followed
by one unchanging zone/color streamed continuously for 60+ seconds (long
enough to rule out "8 seconds wasn't long enough" outright), with a live
USBPcap capture running the whole time and the human watching the
physical zone to report exactly when/whether it visibly changes.

## Windows session 3 — Q2 answered (yes, a single static zone works), and the real zone map was hiding in ASUS's own installed software

Written 2026-07-23. Continuation of session 1's in-progress test, plus an
unrelated but major discovery made digging through installed ASUS
software while waiting.

### The Q1/Q2 controlled test: ran, real result, packet capture never worked

`usb_capture_session3/g615lr_priming_then_static_hold.ps1` — sends the
exact priming sequence from session 1's table (`0x0201`, `0x5d`
`b3`/`b4`/`b5`, `0x0305`, all via `HidSend.cs` directly, bypassing Armoury
Crate's GUI entirely) then streams **one unchanging zone/colour**
continuously for up to 90 seconds.

**Result, methodologically clean (reset the zone to black first, human
watched a confirmed-dark baseline, then ran the script, confirmed it went
from dark to lit with nothing else touching the hardware in between)**:
**the zone visibly lit up.** This directly answers `QUESTIONS.md` Q2 --
**a single static zone streamed continuously, following real priming,
does resolve to a visible colour on Windows.** Zone variety/cycling is
*not* required. If Linux's equivalent test (`g615lr-prime-then-stream.rs`)
still doesn't work with a single static zone, the remaining gap is
something else -- environment, exact byte-level difference, or something
not yet identified -- not "needs zone variety," which can be crossed off
the "for whoever picks this up next" list in the earlier session-3 Linux
section above.

**Q1 (precise latency) was not cleanly answered.** The intent was to
correlate a live USBPcap capture against the exact moment of the visible
change. That capture **never worked, across many attempts** -- root cause
turned out to be picking the capture interface by numeric index (`tshark
-i 7`), which is **not stable**: interface numbers shift as other adapters
(VPNs, virtual switches, Bluetooth devices) connect/disconnect, confirmed
directly when an elevated capture explicitly requested as `-i 7` came back
`Capturing on 'Wi-Fi 2'` instead of `USBPcap1`. **Always select USBPcap by
its literal name** (`-i "\\.\USBPcap1"`), never by index, on this machine
--this cost most of this session's real time. Even after fixing that, no
capture actually landed correctly correlated with a successful visible-change
run before this section was written -- Q1 is still open for whoever
continues this.

### The bigger discovery: ASUS's own software already has the real zone map

While waiting between test runs, went looking through installed ASUS
software (`C:\ProgramData\ASUS`, `C:\Program Files\ASUS`,
`C:\Program Files (x86)\ASUS`) for anything that might describe the
chassis lightbar's real protocol or layout, since none of this has ever
been vendor-documented.

`RogAura30`'s own device-capability files (`GetDeviceCap.xml`,
`GetDeviceStatus.xml`, `GetDeviceStatusNew.xml`) turned out to be a dead
end for this purpose -- they only know about the 4-zone keyboard
(`WDL_NB_KB_4ZONE_RGB_LIGHTING`) and a virtual "WallPaper" software
lighting group. The 12-zone chassis lightbar isn't registered as a device
in that SDK's model *at all* on this machine, and `GetDeviceStatus.xml`'s
`effect_path_order` list shows a formal `LightBar` device-type category
existing in ASUS's schema with `order=-1` (present in the schema, not
actually registered/active here) -- consistent with the chassis lightbar
being handled by something outside Armoury Crate's normal RogAura30-based
device pipeline entirely.

That "something else" is **Aura Creator**, a separate UWP app mentioned
all the way back at the very start of this whole investigation (the
original human request referenced an "Aura Creator XML dump"). Its package
data lives at
`C:\Users\<user>\AppData\Local\Packages\B9ECED6F.AURACreator_qmba6cd70vzyy\LocalState\Devices\`,
and inside it, a folder literally named `G615` contains
**`WDL_G615LR.csv`** -- ASUS's own official per-device zone layout profile
for this exact laptop model, straight from Aura Creator's own device
configuration, not reverse-engineered or empirically derived. Copied into
this repo at `usb_capture_session3/ground_truth/WDL_G615LR.csv`.

The CSV is an 8-column x 5-row physical grid (`GridWidth`/`GridHeight`,
real `phy_x`/`phy_y` coordinates in what's presumably cm, matching
`PhyWidth=35.4`/`PhyHeight=26.4`) with a `lamp_id` column per populated
cell. Decoding it (full derivation: physical y=0 row is the back/hinge
edge since it's closest to the keyboard row, which sits at `phy_y=9.9`,
about 37% of the way down from the back edge -- consistent with normal
laptop ergonomics; `lamp_id` values 0-3 land exactly on 4 evenly-spaced
positions in that keyboard row, confirming `lamp_id` uses the *same
numbering* as this repo's known 0x00-0x0F wire zone IDs) against
`aura_core.ps1`'s zone map (as it stood before this session) found six
zones were wrong:

| Wire ID | This repo previously claimed | **Ground truth (ASUS's own file)** |
|---|---|---|
| `0x04` | back_bar_**left** | back_bar_**right** |
| `0x05` | back_bar_**right** | back_bar_**left** |
| `0x06` | back_corner_**left** | back_corner_**right** |
| `0x07` | back_corner_**right** | back_corner_**left** |
| `0x09` | left_bar_**front** | left_bar_**back** |
| `0x0B` | left_bar_**back** | left_bar_**front** |

Keyboard (`0x00-0x03`), `0x08`, `0x0A`, and the entire front edge
(`0x0C-0x0F`) were already correct. **This exactly explains this session's
own test result**: sent wire zone `0x06` expecting `back_corner_left`
(per the old map), the *physically correct* `back_corner_right` lit up
instead -- a perfect match against this ground-truth file, independently
confirmed live before the CSV was even found. This is very likely a real
contributor to a chunk of this whole project's long-running "zone/colour
flip-flop instability" that was never conclusively explained across
multiple earlier sessions (both this repo's and the original Windows
investigation before it existed) -- not necessarily the *whole*
explanation (the R/G channel swap question is a separate axis from zone
ID), but a genuine, previously-unknown source of confusion layered on top
of it.

**Fixed as of this session**: `usb_capture/aura_core.ps1`'s zone map
(collapsed the old confusing two-hop `$PHYSICAL_MAP` -> `$INTERNAL_ZONES`
indirection into a single direct `$PHYSICAL_ZONES` physical-name -> wire-ID
table, sourced straight from the CSV, with the six corrected entries
called out inline), `aura_control.ps1`/`aura_animate.ps1` (updated to use
the renamed/restructured table), and
`usb_capture_session3/draw_zone_map.py`/`g615lr_zone_map.png` (the
labeled zone diagram, regenerated with corrected positions).

**Not yet done, worth doing**: the `$NO_SWAP_ZONES` G/R-swap table in the
same file was never re-verified against this corrected zone map -- it was
originally derived through testing that had the wrong zone-ID assumptions
baked in, so it's plausible some of *those* results were actually testing
a different physical zone than believed at the time. Re-verifying swap
behaviour per zone against the now-correct map (Red/Green only, per the
existing methodology) is a reasonable next step if colour-channel issues
come up again.

**For Linux**: `usb_capture_session3/ground_truth/WDL_G615LR.csv` is now
in this repo -- pull it. If `rog-aura::lightbar_2025`'s `Lightbar2025Zone`
enum or any test binary encodes physical zone assumptions (variant names
like `BackBarLeft`/`SideLeftBack` were inherited from the same originally-
wrong map), cross-check them against this file rather than against prose
in this doc. Also note: the *wire byte values* sent by any existing Linux
test were never wrong (a wire ID of `0x06` is `0x06` regardless of what a
human calls it) -- this bug only affected human-readable labels/interpretation
of results, not actual protocol bytes on either OS, so it doesn't by
itself explain why Linux's own zone writes still produce zero visible
effect. What it does provide: an authoritative, first-party-sourced zone
table to build from, and a clean confirmation (see Q2 above) that a single
static zone is sufficient in principle.

### Extra context: ASUS's own software doesn't fully support this device either

Digging a bit further, `AppData\Local\Packages\B9ECED6F.AURACreator_qmba6cd70vzyy\LocalState\DebugLog_2026-07-23.log`
(Aura Creator's own live debug log, from earlier today) shows the app's
device list reporting, for the `G615LR` entry specifically: `AURA Kit : 0`,
`HAL : 0` (both zero/absent -- contrast with the `WallPaper` device in the
same list, which shows `AURA Kit : 1` and real version numbers), and the
UI repeatedly triggers a `[MaskManager] ShowMask type : NoSupportDevice`
mask for this laptop's device entry specifically.

Reading: Aura Creator's own official support plugin/HAL for this exact
laptop model isn't currently installed, and the app's own UI actively
flags it as an unsupported device. This isn't a new mechanism or a fix --
it's confirmation/context for something already suspected since the very
start of this investigation ("genuinely undocumented, no vendor
documentation exists"). It does *not* block this repo's approach (which
never goes through Aura Creator's gated pipeline, only raw HID writes),
but it's worth knowing that even ASUS's own consumer software doesn't
consider this laptop's chassis lightbar fully supported yet -- so "vendor
docs will eventually cover this" isn't something to wait on.

`LastScript.xml` (Aura Creator's last-saved effect script) independently
cross-validates the ground-truth CSV: it references LEDs by the CSV's row
index (e.g. `led key="6"`), and `WDL_G615LR.csv`'s "LED 6" row is
`lamp_id=4` (`back_bar_right` per the corrected map) -- consistent with
everything above, no new information beyond confirming the CSV's row
numbering is the same numbering Aura Creator's own script format uses
internally.

### Unconfirmed lead, flagged but not verified -- don't treat as a finding

`C:\ProgramData\ASUS\EC_Logs\EC_Update.txt` contains, twice (2026-06-30
and 2026-07-21): `[CheckArmouryCrateStaticField] Shipping_Year is not
support m_ArmouryCrateStaticFieldYear = 2025`, immediately followed by a
`WriteDLLVersionRegistry` that succeeds and a `WriteLegacyPlatformRegistry
fail outData 7` that doesn't. Read at face value, this is ASUS's own EC
update tooling explicitly saying it doesn't have support data for
2025-model-year laptops, with a registry write failing right after --
tempting to read as *the* missing "host claims control" mechanism this
whole investigation has been looking for.

**Did not confirm this is actually about lighting.** Tried to find which
binary logs this string to establish scope (the EC subsystem covers fan
curves, power profiles, and other non-lighting features too, all through
`Armoury Crate Service`'s many plugin DLLs -- `GPUMode`, `ThrottlePlugin`,
`HWPlugin`, etc., alongside `AuraPlugin`) -- the search either matched
implausibly broadly (consistent with a shared logging string compiled into
a common base library across every plugin, not something lighting-
specific) or timed out before completing cleanly; registry search for the
literal value names mentioned in the log (`ProjectYear`, `StaticField`)
came up empty in both `HKLM\SOFTWARE` and `HKCU\SOFTWARE`, for what that's
worth (the actual value name is probably not literally either of those
strings). `AsIO3`'s own log for `ArmouryCrate.Service.exe` is present but
empty, no help either way.

Confirmed the plugin DLLs (`ArmouryCrate.*.dll`) are native PE binaries,
not .NET (no CLR markers, `[System.Reflection.Assembly]::LoadFile` throws
`0x80131018`) -- a .NET decompiler wouldn't help here, would need a real
disassembler (Ghidra etc.), not attempted.

Tried catching the actual registry write live with Sysinternals Process
Monitor (elevated, headless capture via `/AcceptEula /Quiet /Minimized
/BackingFile`), triggered by restarting the relevant ASUS services. **The
log's `LastWriteTime` did not change after a plain service restart** --
so this check doesn't fire on every service start.

**Follow-up that actually resolved it**: the registry search had been
looking in the wrong place -- the real ASUS vendor key is
`HKLM\SOFTWARE\WOW6432Node\ASUSTek Computer Inc.` (note capitalization,
`WOW6432Node`, and no period after "Inc" vs the `ASUSTeK Computer Inc.`
guessed earlier), found by listing `HKLM\SOFTWARE\WOW6432Node` directly
rather than guessing the exact key name. It only contains two empty
version-marker subkeys (`AC_MainSDK\1.00.0000`,
`ASUS Framework Service\3.0.0.4`) with no values at all -- confirmed this
isn't where the relevant data lives either, not just an unsearched gap.

More usefully: cross-referenced the exact `EC_Update.txt` timestamp
(2026-07-21 08:17:39) against Windows' own Application and System event
logs. The Application log shows `AsusAppService` events firing right at
that moment, wrapped inside a `RestartManager` session spanning
08:17:13-08:17:35 (the pattern for an active install/update process, not
routine runtime activity) -- consistent with why a plain service restart
never reproduces it. The System log for the same window is unambiguous:
this was a general software-maintenance burst -- Windows Update installing
multiple packages (`Microsoft.WindowsAppRuntime.1.8`, `DesktopAppInstaller`,
a Defender definitions update), TPM/Secure Boot certificate updates, and
critically, **the `AsusSAIO` service being installed twice from the driver
store** (`asussci2.inf`, `ASUSSystemAnalysis\AsusSAIO.sys`) at 08:17:34 and
08:18:04 -- both essentially simultaneous with the `EC_Update.txt` line.
`AsusSAIO` ("ASUS System Analysis I/O") is a general hardware-diagnostics/
telemetry driver, not anything Aura/lighting-related.

**Conclusion, with real evidence behind it this time**: the
`Shipping_Year is not support` check is part of ASUS's routine software/
driver update-and-registration cycle (tied to `AsusAppService` performing
package maintenance, correlated with an unrelated diagnostics driver
reinstall happening in the same window), not a lighting-specific gate and
not something that fires during normal `0x04` operation. **Closing this
thread with actual confidence** -- real, documented, reproducible gap in
ASUS's tooling, but the evidence points away from it being connected to
the lightbar protocol, not just "unconfirmed either way."

### 12-zone real capture, byte-perfect, human-confirmed correct on every zone

The strongest evidence produced this session. Same approach as the Q2
test (priming via `HidSend.cs`, bypassing Armoury Crate entirely) but
instead of one static zone, sent **12 of the 16 zones simultaneously**,
each a distinct, unambiguous colour, via `aura_control.ps1` (using the
corrected `$PHYSICAL_ZONES` map from earlier this session) -- first an
explicit all-black reset, then the real colours, both while a live
USBPcap capture ran. The human confirmed **every single zone matched**
what was sent, on the physical hardware, twice (once before the capture
pipeline was confirmed working, once after, both attempts visually
identical). Saved at
`usb_capture_session4/multizone_12x_confirmed.pcapng`.

Real captured bytes (`t=37.37-37.40s`, chronological, one `0x0304` write
per zone):

| Wire ID | Physical zone | Colour sent | Raw bytes (zone id + colour slot) |
|---|---|---|---|
| `0x00` | kbd1 | `FF0000` | `04 01 01 00 00 ... ff 00 00 ff` |
| `0x01` | kbd2 | `00FF00` | `04 01 01 01 00 ... 00 ff 00 ff` |
| `0x02` | kbd3 | `0000FF` | `04 01 01 02 00 ... 00 00 ff ff` |
| `0x03` | kbd4 | `FFFFFF` | `04 01 01 03 00 ... ff ff ff ff` |
| `0x05` | back_left | `FF0000` | `04 01 01 05 00 ... ff 00 00 ff` |
| `0x04` | back_right | `00FF00` | `04 01 01 04 00 ... 00 ff 00 ff` |
| `0x07` | back_corner_left | `0000FF` | `04 01 01 07 00 ... 00 00 ff ff` |
| `0x06` | back_corner_right | `FFFF00` | `04 01 01 06 00 ... ff ff 00 ff` |
| `0x08` | right_bar_back | *(untouched, forced black by `aura_control.ps1`)* | `04 01 01 08 00 ... 00 00 00 ff` |
| `0x09` | left_bar_back | *(untouched)* | `04 01 01 09 00 ... 00 00 00 ff` |
| `0x0A` | right_bar_front | *(untouched)* | `04 01 01 0a 00 ... 00 00 00 ff` |
| `0x0B` | left_bar_front | *(untouched)* | `04 01 01 0b 00 ... 00 00 00 ff` |
| `0x0C` | front_corner_right | `FF8000` | `04 01 01 0c 00 ... ff 80 00 ff` |
| `0x0D` | front_corner_left | `FFFFFF` | `04 01 01 0d 00 ... ff ff ff ff` |
| `0x0E` | front_bar_right | `00FFFF` | `04 01 01 0e 00 ... 00 ff ff ff` |
| `0x0F` | front_bar_left | `FF00FF` | `04 01 01 0f 00 ... ff 00 ff ff` |

(`aura_control.ps1` always writes all 16 zones every call -- the four
side-bar zones weren't in the requested list this run, so they were sent
as explicit black rather than skipped, which is why they're in the
capture too and confirms they don't need separate testing to prove the
send path works for them.)

This is the single richest, most-validated piece of evidence in the
entire investigation: real wire bytes, real distinct colours across
12 of 16 zones at once, direct human visual confirmation of every one,
matching the corrected zone map exactly, all in one capture file. If
anything Linux tries produces different bytes than this table for the
same physical zones, that's the bug -- this is now the reference to
diff against, not prose.

**One tooling gotcha worth recording** (cost real time this session):
launching multiple parallel `tshark` captures via PowerShell's
`Start-Process` from within an automated/scripted invocation is
unreliable -- of three launched together, only one reliably survived,
even though all three showed as running processes momentarily. Launching
each capture as its own independent foreground-attached background
process (not spawned via `Start-Process` from inside another script)
worked reliably every time. Separately: passing a literal
`\\.\USBPcap1`-style device path through Git Bash mangles it (collapses
to a single backslash, `\.\USBPcap1`, which `tshark` rejects outright) --
use a native PowerShell invocation for anything with that path syntax,
never Bash.

### Major discovery: `0x0305` is a real, separate, continuously-streamed animated-effects protocol -- not a handshake packet

Prompted by a direct question: is there anything else driving the chassis
besides `0x04`, and do built-in modes like Breathing use different
hardware bytes entirely? Answer: **yes, completely different mechanism,
never previously characterized.**

Captured a live session (`usb_capture_session4/breathing_mode_capture.pcapng`,
120s window, human switched Armoury Crate through Breathing → Strobing →
Color Cycle → Static Blue) and found:

- **Zero `0x0304` packets in the entire capture.** Built-in animated
  effects never touch the per-zone protocol at all.
- **184 `0x0305` packets** -- the same report previously catalogued as a
  one-shot "handshake" sent once before `0x04` traffic (see Windows
  session 1/Linux session 3's priming-sequence table). It is not a
  handshake. It's a **continuously-streamed, compact 10-byte effect-
  parameter packet**, sent at roughly 5-15Hz for the entire duration an
  animated mode is active, structured as:

  ```
  05 01 00 00 0f 00 [byte6] 00 [byte8] [byte9]
  ```

  Bytes 0-5 and the trailing structure stay constant; which of
  bytes 6/8/9 actually varies -- and how -- depends on the active mode:

  | Mode | What varies | Pattern observed |
  |---|---|---|
  | Breathing | `byte[9]` | Smooth ramp `0x00→0xff→0x00`, ~3s period -- textbook sine-wave brightness envelope |
  | Strobing | `byte[9]` | Same envelope shape, much shorter period (faster oscillation) |
  | Color Cycle | `byte[6]` | Ramps `0x00→0xff` then wraps; `byte[8]`/`byte[9]` locked to `0xff 0xff` (max saturation/value while hue rotates) |
  | Static (any colour) | -- | Streaming **stops** -- confirms it's genuinely animation-only, not a periodic keepalive needed for `0x04` or anything else |

  Each of the four mode switches was immediately preceded by the exact
  same `5d b3 00 02 00 00 00 eb...` / `b4` / `b5` triplet already known
  from the priming sequence -- **always with mode byte `0x02`
  (`RainbowCycle`'s `AuraModeNum` value) regardless of which mode was
  actually being switched to.** So that triplet is not "set mode to X" --
  it's some kind of generic reset/re-init step using a hardcoded template,
  sent before every mode change no matter the target. Worth remembering
  next time that packet's exact role gets re-examined.

  `byte[4]`'s constant value `0x0f` is unexplained -- could be a
  zone/target selector (`0x0f` = 15 = highest zone ID, maybe a "target:
  all zones" broadcast sentinel), could be something else entirely. Not
  verified either way this session.

**Why this matters more than it might first look**: this is a
self-contained, fully-characterized, comparatively simple protocol that
drives real hardware animation on the whole chassis, has nothing to do
with the still-unsolved `0x04` per-zone mystery, and was never tested on
Linux at all -- every Linux test so far has only ever attempted `0x04`.
Implementing hardware Breathing/Strobing/Color Cycle via `0x0305` streaming
could be a genuinely achievable, real win independent of whether `0x04`
ever gets solved, and might also turn out to shed light on `0x04` by
comparison once both are better understood (e.g. checking whether `0x04`
needs similarly *continuous* streaming rather than the priming to be the
missing piece -- worth revisiting with this framing in mind).

**Not yet done**: didn't test whether Armoury Crate's UI speed/intensity
setting changes the streaming *rate* rather than the byte values
themselves (plausible reading of "level 0-3" style UI controls); didn't
capture the other 7 built-in modes confirmed dead via `0x5d`
(`Star`/`Rain`/`Highlight`/`Laser`/`Ripple`/`Comet`/`Flash`) to check
whether they *also* try to stream `0x0305` and just get ignored by the
firmware, which would be an easy independent cross-check of the "real
firmware gap, not a code bug" conclusion from Linux session 3 Part A;
didn't determine what `byte[4]=0x0f` means.

### Full "Basic Effects" mode inventory (Armoury Crate's actual menu, 12 tiles)

Only 4 of these were actually captured this session. Listing the complete
menu here so nobody assumes more coverage than there is, and so future
capture sessions know exactly what's still uncharacterized:

| Mode | Protocol (known/suspected) | Status |
|---|---|---|
| Static | `0x5d` (whole-chassis) or `0x04` (per-zone, via `aura_control.ps1`) | Both confirmed working |
| Breathing | `0x0305` continuous stream (`byte[9]` ramp) | **Captured, characterized** (this session) |
| Strobing | `0x0305` continuous stream (`byte[9]` ramp, faster) | **Captured, characterized** (this session) |
| Color Cycle | `0x0305` continuous stream (`byte[6]` hue ramp) | **Captured, characterized** (this session) |
| Rainbow | Almost certainly `0x5d` `AuraModeNum::RainbowCycle` (mode `0x02`) -- the same mode byte the generic priming/reset triplet always hardcodes | Confirmed working via `0x5d` (Linux session 2/3), **not** separately captured via `0x0305` this session -- worth checking whether Rainbow *also* streams `0x0305` like the others, or is genuinely `0x5d`-only autonomous |
| Starry night | Likely `AuraModeNum::Star` -- one of the 7 modes already confirmed **dead** via `0x5d` (identical ACK regardless of mode, see Linux session 3 Part A) | Not captured via `0x0305` -- **good cross-check candidate**: if it also tries to stream `0x0305` and gets ignored by firmware, that independently confirms the "real firmware gap" conclusion from a second angle |
| Music | Host-computed from live audio (WASAPI-style capture + FFT), documented conceptually since the very start of this whole investigation | Protocol never actually captured -- unknown whether it streams via `0x04`, `0x0305`, or something else entirely |
| Smart | Undocumented -- likely some context/sensor-adaptive mode, never investigated at all | Completely uncharacterized |
| Adaptive Color | Host-computed from screen content (display capture + colour sampling), documented conceptually since the start of this investigation | Protocol never actually captured -- same open question as Music |
| Dark (Off) | Presumably an all-zero write via whichever protocol, or a dedicated off command | Never captured directly |
| AI Aura Lighting | Undocumented, never investigated | Completely uncharacterized |
| INDIA | The human's own custom saved profile/scene (the original India-flag layout from the very first Windows session) -- almost certainly per-zone `0x04`, since that's what built it originally | Known working (it's literally `usb_capture/aura_india.ps1`'s target), not re-captured fresh this session |

If anyone captures Music, Smart, Adaptive Color, AI Aura Lighting, or
Starry night, the same methodology as this session's `breathing_mode_capture.pcapng`
applies directly: start a named-interface `tshark` capture, switch modes
in Armoury Crate, stop, and scan for `0x0304` vs `0x0305` traffic (or
something new entirely) the same way.

### Visual reference: the zone-map diagram

`usb_capture_session3/g615lr_zone_map.png` (generated by
`usb_capture_session3/draw_zone_map.py`, matplotlib) is a labeled
top-down diagram of all 16 zones -- physical name plus wire hex ID on
every zone, laid out spatially to match the real chassis. This is what
resolved the back-left/back-right ambiguity that caused real confusion
earlier this session, and it reflects the *corrected* zone map (not the
original wrong one). Point at it instead of describing zones in prose
when reporting which physical zone did what -- that's exactly what fixed
the ambiguity last time.

## Linux session 4 update — zone-map fix verified, first real 0x0305 test (negative result)

Written 2026-07-23/24. Picked up all of the above after pulling Windows
sessions 1/3/4 from the shared repo.

**Zone map fixed and permanently regression-tested.** Independently
re-derived the corrected zone map straight from the raw
`WDL_G615LR.csv` grid coordinates (not just trusted the summary table),
cross-checked against the labeled diagram, and against the human-confirmed
12-zone capture -- all three agreed exactly. Renamed the 6 wrong
`Lightbar2025Zone` variants in `rog-aura/src/lightbar_2025.rs` (wire ID
values unchanged, only names), updated `needs_grb_swap()` to keep
targeting the same two empirically-tested wire IDs under their corrected
names, and added `matches_human_confirmed_capture` -- a permanent test
that builds a packet for every zone/colour pair from
`multizone_12x_confirmed.pcapng` and asserts exact byte match. All pass.
Packet construction is now about as verified as it can be without new
hardware evidence.

**First Linux test of the `0x0305` animated-effects protocol Windows
session 4 discovered -- negative result, but a clean one.** Two variants
tried, both via `rog-platform/examples/`:

1. `g615lr-0305-breathe-stream.rs` -- real priming (`SET_IDLE` x2, `0x0201`,
   the `b3/b4/b5` triplet, real bytes) followed by 10 seconds of continuous
   `0x0305` streaming with a triangle-wave `byte[9]` ramp matching the real
   captured pattern from `usb_capture_session4/all_0305.txt` exactly
   (`05 01 00 00 0f 00 ff 00 00 [ramp]`, ~16Hz). **Result: chassis went
   RainbowCycle, identical to every other priming test -- no
   distinguishable breathing/pulsing on top.**
2. `g615lr-0305-only-stream.rs` -- same `0x0305` stream, but deliberately
   *without* the `b3/b4/b5` triplet (just `SET_IDLE` + `0x0201`), against a
   plain dark/static-black baseline, to rule out the triplet's own
   RainbowCycle animation masking a subtler effect. **Result: nothing
   changed at all, stayed dark for the full 10 seconds.**

**Interpretation, not yet conclusive**: `0x0305` alone does nothing
observable; `0x0305` after the `b3/b4/b5` triplet produces exactly what
the triplet alone produces, no more. Two live possibilities, not
distinguished by this test:
- This specific EC firmware genuinely doesn't implement `0x0305`-driven
  animation at all -- consistent with the broader pattern from Linux
  session 3 Part A, where 7 of 12 classic `0x5d` modes turned out to be a
  real firmware gap, not a code bug, on this specific board.
- Something else Windows sends is still missing. The Windows capture that
  characterized this protocol
  (`usb_capture_session4/breathing_mode_capture.pcapng`) never identified
  where the actual *colour* being modulated comes from -- zero `0x0304`
  traffic during Breathing, and the triplet's own colour field is black --
  so there's an acknowledged gap in Windows' own understanding of this
  protocol too, not just Linux's reproduction of it. It's possible a
  colour needs to be established through some mechanism neither side has
  found yet before `0x0305` has anything to modulate.

**Tried the "set colour first" idea, third negative result**:
`g615lr-0305-with-color-first.rs` -- set a real red via the proven-working
`0x5d` Static sequence (`b3,b5,b4` order), confirmed visibly red, *then*
minimal priming (`SET_IDLE`+`0x0201` only, deliberately skipping the
RainbowCycle-forcing triplet so it can't clobber the colour), then the
`0x0305` handshake and breathing stream. **Result: stayed solid red for
the full 10 seconds, no breathing/pulsing at all.** Three independent,
controlled tests now agree: `0x0305` alone, `0x0305` after the priming
triplet, and `0x0305` after establishing a real colour all produce zero
observable effect beyond whatever the *other* mechanism already in play
was doing. This is no longer "we haven't found the right precondition" --
it's consistent, controlled negative evidence across every reasonable
precondition tried.

**Current conclusion**: either this specific EC firmware doesn't implement
`0x0305`-driven modulation (matching the broader "real firmware gap, not a
code bug" pattern already established for 7 of 12 classic `0x5d` modes in
session 3 Part A), or there's a genuinely unidentified prerequisite neither
side of this investigation has found yet -- Windows' own capture never
established where the modulated colour comes from either, so this gap
isn't unique to the Linux reproduction. Parking this specific protocol for
now; pivoting to testing whether *combining* `0x0305` streaming with `0x04`
zone writes (a different hypothesis -- not "does 0x0305 animate on its
own," but "does keeping it alive change whether 0x04 finally sticks") does
anything, per `QUESTIONS.md`'s Windows-session-4 question 2.
