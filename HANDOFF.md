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

**Tried, negative**: `g615lr-0305-parallel-0304.rs` -- real priming, then
10 seconds of `0x0305` (steady handshake-style bytes) interleaved with
continuous `0x04` zone-`0x06` red writes (~15 writes/sec each,
alternating). **Result: stayed on RainbowCycle the whole time, identical
to every other priming test, zero incremental effect from the zone
writes.** Consistent with the observation going into this test that real
Windows captures never actually show these two mechanisms combined
(`0x04` sessions send `0x0305` exactly once; `0x0305` animated-mode
sessions send zero `0x0304`) -- this specific combination doesn't appear
to be how the real protocol works, and testing it confirmed that rather
than revealing something new. Crossing this off; the answer isn't
"stream both at once."

**Tried an 8-zone batched write, negative, plus a real byte-for-byte wire
verification.** Windows session 1 found the real first `0x0304` write
after priming is an 8-zone batch, not a single zone -- every prior Linux
test streamed one static zone. `g615lr-8zone-batch.rs` batches 8 zones
(the 4 keyboard + 4 back-edge zones, each a distinct colour) using the
*real* production packet builder (`rog_aura::build_lightbar_2025_packet`,
not hand-rolled bytes) after real priming, held for 10 seconds. **Result:
stayed on RainbowCycle, identical to every single-zone test.**

At this point the pattern of "always exactly the same result regardless of
what's in the `0x04` payload" raised a fair question (asked directly): is
this actually a bug in the Rust code, not a protocol mystery? Checked
directly rather than assumed -- captured this exact test run with
`usbmon` and compared the program's own printed intended packet bytes
against the literal bytes recorded going out on the wire.
**Byte-for-byte match, confirmed.** (One wrinkle: usbmon's text interface
only displays the first 32 of the 51 bytes per line by default -- a known
display limitation, not a real truncation; the same URB record's own
`wLength=51` field and every test's own `Ok(51)` "bytes accepted" return
value independently and consistently confirm the full packet actually
transfers.) **This rules out "wrong bytes reaching the wire" as thoroughly
as it can be ruled out** -- the zone IDs, structure, and colour data the
Rust code intends to send are provably identical to what's recorded
leaving the machine on the real USB bus. If there's still a code-level bug
anywhere, it isn't in packet construction or in whether `write_control`
actually transmits what's asked -- it would have to be something about
call semantics/sequencing/timing this session hasn't identified yet, not
"the bytes are wrong."

**Confirmed directly: `rog-control-center` (GUI) sends byte-identical
`0x5d` traffic to the CLI, no separate/different mechanism.** Captured a
40-second window with `usbmon` while manually clicking through modes in
the GUI (dark baseline first, `asusd` left running normally, not one of
the raw test binaries). Same exact `b3`/`b5`/`b4`/`5a` four-packet
sequence per mode change, same structure, as every CLI test already
established -- this was previously assumed (both go through the identical
`set_led_mode_data` D-Bus method) but is now directly verified with real
capture data rather than just inferred from source.

**False-alarm regression, real explanation found and confirmed fixed, not
a code bug**: mid-session, `Static`/`Breathe`/`Pulse` appeared to stop
working via the GUI (only `RainbowCycle`/`RainbowWave` still visibly
worked), raising a real concern that this session's patches had broken
something. The capture above explains it cleanly: every `b3` packet in
that window had `colour1 = 00 00 00` (black), because
`asusctl aura effect static -c 000000` (run deliberately for a clean dark
baseline before the capture) set asusd's cached "last colour" state, and
clicking mode tiles in the GUI without separately re-picking a colour for
each one reused that same black. For `Static`/`Breathe`/`Pulse`, colour is
literally what renders -- black is indistinguishable from off. For
`RainbowCycle`/`RainbowWave`, `colour1` is irrelevant (procedural
animations, not literal-colour modes), so they displayed fine regardless.
**Confirmed fixed**: setting real colours (`static -c ff0000`,
`pulse -c 0000ff`) immediately worked normally again. Nothing in this
session's actual code changes was involved -- this would reproduce on
stock, unpatched `asusctl` too if you set black then expected non-colour
modes to still show something.

**`tshark` installed and used to independently cross-validate all prior
capture analysis.** One packaging gotcha: Ubuntu's `wireshark-common`
restricts `tshark` to the `wireshark` group (`sudo usermod -aG wireshark
<user>`, needs a fresh shell/login to take effect); separately, this
sandboxed environment's `tshark` process couldn't traverse
`/home/krishna` itself (`drwxr-x---`) even as the owning user for reasons
not fully root-caused -- worked fine once the target file was copied to a
world-accessible scratch path instead. Once working, used `tshark -r
multizone_12x_confirmed.pcapng -V` (the exact tool Windows has been using
all along, not a hand-rolled parser) to independently re-extract and
decode all 32 `0x0304` writes in that file. **Result: matches the
already-documented 16-zone table exactly** -- same zones, same colours,
both the black-reset pass and the real-colour pass. Four independent
sources now agree: the original Python/USBPcap-header parser, `tshark`,
the human-confirmed table in this file, and the `matches_human_confirmed_capture`
Rust unit test. Packet-content analysis is about as thoroughly
cross-validated as it can get.

**Final, most rigorous `0x04` test this session: replayed the LITERAL
captured bytes (via `tshark`, not our own packet builder's re-derived
output) for all 16 zones from `multizone_12x_confirmed.pcapng`'s
real-colour pass, after real priming.** Every byte in
`g615lr-literal-12zone-replay.rs` was copied directly out of the actual
human-confirmed-working Windows capture file, not regenerated by any of
our code -- this removes even the theoretical possibility of a subtle
packet-construction discrepancy between our builder and the real thing.
**Result: still just RainbowCycle, identical to every other test this
session.** This is as close to "send Windows' own exact bytes" as
possible from Linux, and it makes no difference. Combined with the
byte-for-byte wire verification from earlier in this session, `0x04`
packet *content* can now be considered fully and conclusively exonerated
-- whatever is blocking this is not in the payload, at any level of
scrutiny applied so far.

Real capture backing this specific result, citable:
`linux_capture_session4/usbmon_literal_12zone_replay.txt` -- all 16
`0x0304` writes present, each returning success, matching the program's
own printed `Ok(51)` output for every write. (A live `tshark`-captured
`.pcapng` -- matching Windows' exact capture format -- was attempted for
this run too but blocked by a Wireshark privilege-dropping quirk even
under `sudo`; see `linux_capture_session4/NOTE_FROM_LINUX_CLAUDE.md` for
the full explanation. The `usbmon` text capture is fully sufficient
evidence on its own.)

**Raw capture data added to the repo**: `linux_capture_session4/` (two
`usbmon` text captures plus a note, matching the `usb_capture_session3`/`4`
pattern Windows established) -- see that folder's own
`NOTE_FROM_LINUX_CLAUDE.md` for details on both files and the `usbmon`
32-byte text-display-truncation gotcha discovered while producing them.

## Windows session 5 -- reframing "why does it always end up on RainbowCycle"

Prompted by a direct question after reading Linux session 4's results in
full: why RainbowCycle *specifically*, every single time, regardless of
what the `0x04` payload contains? The answer was already sitting in data
gathered earlier this investigation, just not stated explicitly.

**The "priming" `5d b3/b4/b5` triplet is not a handshake or precondition
-- it is a complete, valid, successfully-applied `0x5d` command.** `b3`'s
payload is `AuraEffect`'s real wire encoding: mode byte `0x02` is the
literal `AuraModeNum::RainbowCycle` value, and `b4`/`b5` are the same
apply/commit pair `write_effect_and_apply` uses for every other real
`0x5d` effect-set call. Confirmed hardcoded to `0x02` regardless of
target, across all 4 real mode switches in Windows session 4's
`breathing_mode_capture.pcapng` (Breathing/Strobing/Color Cycle/Static all
preceded by the identical `mode=0x02` reset). So every test in this
investigation that "primes" the device by sending this triplet is, as a
side effect, **actually issuing and successfully completing a real
`0x5d set-effect(RainbowCycle) + apply` command** -- not sending inert
setup data before the "real" protocol starts.

**This reframes the whole `0x04` question.** It was never really "why
doesn't `0x04` produce a visible effect" -- every test's baseline already
has a real, successfully-applied, whole-chassis animation running via
`0x5d` before any `0x04` byte is ever sent. The actual question is **why
doesn't `0x04` override an already-active, already-applied `0x5d`
RainbowCycle state.** On Windows, something clearly does override it --
every real captured working session shows a genuine colour after this
exact sequence. On Linux, nothing has, in any test run so far, including
the literal-byte-replay test that eliminates payload content as a
variable entirely.

**One thing this doesn't yet explain**: raw timing alone doesn't obviously
account for the difference. The real Windows capture
(`aura.pcap`, Windows session 1) shows only ~42ms between the last priming
packet and the first `0x0304` write -- if "RainbowCycle needs to actually
start animating before `0x04` can interrupt it" were the mechanism, that
window looks almost too short for the EC to have visibly begun animating
at all. Windows session 3's own controlled test
(`g615lr_priming_then_static_hold.ps1`) used a comparably tight,
un-delayed gap between priming and the first `0x04` write and still
succeeded. Linux's literal-replay test structure is similarly tight. So
"give `0x04` more of a head start before RainbowCycle takes hold" is not
obviously the missing piece by itself -- but it's a much more specific,
testable question than the vague "why doesn't `0x04` work" framing this
investigation started with, and worth carrying forward as the operating
question rather than reverting to the old framing.

**Not yet tested, worth trying next**: does explicitly *cancelling* the
`0x5d` RainbowCycle state first (e.g. a real `Static` `0x5d` command, or
whatever the genuine "turn off the classic effect engine" signal turns
out to be) before attempting `0x04`, rather than relying on `0x04` to
implicitly override it, change anything? This was never isolated as its
own variable -- every test so far either sends the RainbowCycle-triggering
triplet immediately before `0x04`, or (Linux's `g615lr-0305-only-stream.rs`)
skips it entirely and gets a dark/inert baseline instead, never "a
different, non-animating `0x5d` state, then `0x04`."

## Q1 finally answered: real visible-colour latency is ~8-12 seconds, not near-instant

Same session. Fixed the methodology that kept failing before: ran
`g615lr_priming_then_static_hold.ps1` **as a background task** instead of
a blocking foreground call, specifically so the human's "NOW" report could
arrive and be checked *while the script was still running*, rather than
only being visible after the whole run (and thus the whole timing window)
had already completed. A live USBPcap3 capture ran throughout.

**The measurement**: the human reported the corner visibly changing;
checked the running script's own log at that exact moment and it was at
its internal `t≈12.66s`. Cross-referenced against the live capture to
establish a clean, offset-independent timeline: the capture's first
priming packet (`t=19.81s` in the capture's own clock, since `tshark`
was started a few seconds before the script) lines up exactly with the
script's own `t=0.01s` (its first priming send) -- a ~19.8s clock offset
between the two. Accounting for ordinary human reaction+typing latency
before the report could be checked (a few seconds, not measured
precisely), **the real visible colour change happens somewhere around
8-12 seconds after the `0x04` streaming begins.**

**Why this matters directly**: every Linux `0x04` streaming test so far
used **8 seconds** (`g615lr-prime-then-stream.rs` and everything built on
it). If the real threshold is genuinely in the 8-12s range, those tests
may simply not have run long enough -- not a protocol or platform
difference at all, just an insufficiently long observation window. This
was flagged as a real possibility as far back as Linux session 3's "for
whoever picks this up next" list ("a much longer stream after priming,
30-60s+, not 8s") but never actually tested with real timing data behind
it until now.

**Caveats, stated plainly**: this is not a millisecond-precision lab
measurement -- it's bounded by ordinary message/reaction latency in a
human-in-the-loop test, roughly a 4-5 second uncertainty window. It is,
however, real, repeated (this exact test has now visibly succeeded 5+
times across this session, always within a comparable timeframe going by
how quickly confirmations came back each time), and vastly more precise
than the prior state of knowledge ("somewhere in a 30-90 second window,
never pinned down"). **The clear, high-value next test for Linux**: run
`g615lr-prime-then-stream.rs` (or any of the other negative `0x04` tests)
for 20-30+ seconds instead of 8, on the theory that they were stopped
just before the real threshold rather than testing a genuinely different
outcome. Cheap to try, directly informed by this measurement, and was
already on Linux's own "not yet tried" list independently.

## Confirmed, real, separate gap: the actual application has no per-zone invoker at all

Prompted by a direct question, verified against source rather than
assumed. `write_lightbar_2025` (`asusd/src/aura_laptop/mod.rs`) is
referenced **exactly once in the entire codebase -- its own definition.**
No D-Bus method, no CLI subcommand, no GUI control calls it, anywhere.

Traced what `rog-control-center`'s actual UI does when a mode tile is
clicked (`rog-control-center/src/ui/setup_aura.rs`): its only "zone"
concept is `PowerZones::Keyboard` / `PowerZones::Lightbar` /
`PowerZones::KeyboardAndLightbar` -- coarse *device-capability* flags
(does this laptop have a keyboard, a lightbar, or both, used to decide
which power-state toggles to show), not per-LED addressing of any kind.
Every effect selection converts to a single `rog_aura::AuraEffect` (the
1-2 colour, whole-device struct already known from earlier sessions) and
flows into `write_effect_and_apply` -- the `0x5d` protocol, always,
regardless of which mode tile was clicked. **There is no code path
anywhere in this repo's actual user-facing application that could reach
`Lightbar2025Zone`/`0x04` control, even in principle.**

**Important, don't conflate this with the hardware mystery above**: every
single `0x04` test that produced a negative result this whole
investigation (`g615lr-lightbar-test.rs` through
`g615lr-literal-12zone-replay.rs`) calls `rog_platform::hid_raw::HidRaw`'s
raw transport functions **directly**, bypassing `rog-control-center` and
`asusd`'s dispatch layer entirely -- same real bytes hitting the real USB
wire, zero GUI/CLI/D-Bus involvement. So this confirmed dispatch gap is
real and worth fixing eventually, but it is **not** why those tests fail
-- those tests never touch this code at all. Two separate, both-real
problems: (1) the low-level `0x04` protocol doesn't produce a visible
effect for reasons still under investigation above, and (2) even if it
did, there's currently no way for a real user to invoke it through the
actual shipped application. Fixing (2) is straightforward, ordinary
feature work once (1) is solved -- wire a per-zone D-Bus method and a
GUI control that takes 16 colours instead of 1-2. Not worth doing before
(1), since there'd be nothing working yet to expose.

## Windows session 6 -- wired up the missing invoker (gap 2 above), isolated from shared code

Written the same session, after the gap above was confirmed. Since it's
real, useful infrastructure regardless of whether `0x04` currently
produces a visible effect (and genuinely useful for continued testing --
a real CLI command beats writing a new `rog-platform/examples/*.rs`
binary for every test), implemented it now rather than waiting.
**Deliberately isolated from every shared code path** per explicit
instruction -- this is local-only, not proposed upstream, scoped to this
one laptop, and structured so it cannot affect any other device's
behaviour even accidentally:

- `rog-aura/src/lightbar_2025.rs` -- added `TryFrom<u8> for Lightbar2025Zone`
  (validates raw wire IDs, rejects anything outside `0x00-0x0F` rather than
  silently truncating).
- `asusd/src/aura_laptop/trait_impls.rs` -- new D-Bus method
  `write_lightbar_2025_zones(&self, zones: Vec<(u8,u8,u8,u8)>)`, added
  next to (not replacing or modifying) the existing `direct_addressing_raw`
  method. Converts and validates each `(zone_id, r, g, b)` tuple, batches
  into <=8-zone groups (the hardware's real per-packet limit), calls the
  already-existing-but-previously-orphaned `Aura::write_lightbar_2025` for
  each batch. Does not touch `AuraEffect`, `write_effect_and_apply`, or
  any code path any other laptop's dispatch uses.
- `rog-dbus/src/zbus_aura.rs` -- matching proxy trait method
  (`WriteLightbar2025Zones` on the wire, zbus's standard snake_case ->
  PascalCase name mapping).
- `asusctl/src/aura_cli.rs` + `cli_opts.rs` + `main.rs` -- new **top-level**
  CLI command, `asusctl lightbar2025 --zone 0:ff0000 --zone 6:00ff00 ...`
  (repeatable `zone:RRGGBB` pairs). Deliberately a new top-level
  `CliCommand` variant, not nested inside the existing `aura`/
  `AuraSubCommand` tree, so it's trivially greppable/removable and doesn't
  risk the shared command surface other laptops' users see.
- `rog-control-center` -- one new button, "Test G615LR Lightbar
  (experimental)", next to the existing Power Settings button on the Aura
  page. Sends a hardcoded test pattern via the new D-Bus method: the exact
  same 12 zone/colour pairs already human-confirmed correct on real
  hardware (Windows session 3,
  `usb_capture_session4/multizone_12x_confirmed.pcapng`), so a successful
  click has a known-correct result to visually check against, not an
  arbitrary pattern. New Slint callback (`cb_lightbar_2025_test`) kept
  completely separate from `led_mode_data`/the colour-slider wiring --
  doesn't share state with the effect controls above it.

**Honesty check, same as every other Rust change from this side of the
investigation**: none of this has been compiled. Windows can't build this
workspace at all (Linux-only `udev` dependency). Reviewed carefully
against real, confirmed patterns already in the codebase for every
piece -- the D-Bus error variant used (`ZbErr::Failed`) was cross-checked
against its actual usage elsewhere in this exact file rather than assumed
(an earlier draft used `ZbErr::InvalidArgs`, which doesn't appear
anywhere else in this codebase and was swapped out rather than risking
it); `Colour`'s field names, the `Aura`/`AuraZbus` wrapping pattern, the
proxy macro's naming convention, and the Slint callback/button pattern
were all matched against existing working examples in the same files,
not guessed. But "carefully reviewed" is not "compiled," let alone
"tested on hardware." **First thing to do on the Linux side: `cargo
check -p rog_aura -p asusd -p rog-dbus -p asusctl`, then
`cargo build -p rog-control-center` separately (Slint code generation
can fail in ways plain `cargo check` on other crates won't catch), fix
whatever doesn't compile, then actually run `asusctl lightbar2025 --zone
0:ff0000` and confirm it round-trips to `write_lightbar_2025_zones` on
the wire before trusting any of this UI-side.**

## Windows session 6, part 2 -- full 16-zone manual canvas, matching Aura Creator's shape

Same session, prompted by a direct question comparing the single test
button above against Aura Creator's actual UI (a real interactive
per-zone canvas, not a fire-a-hardcoded-pattern button). Attempted a
closer equivalent, same isolation rules as everything else in this
section -- own files, own global, zero shared state with the classic
`AuraEffect` controls:

- `rog-control-center/ui/types/lightbar_2025_types.slint` (new) --
  `Lightbar2025Data` global, **16 separate named colour properties**
  (`colour_kbd1` .. `colour_front_bar_right`), not one array. Deliberately
  more repetitive than an array-indexed approach would be, in exchange for
  every read/write being a plain property access -- the lowest-risk Slint
  pattern available given none of this can be compile-checked before
  committing.
- `rog-control-center/ui/widgets/lightbar_2025_canvas.slint` (new) -- 16
  independently-named `ZoneRow` elements (swatch + hex `LineEdit` each),
  absolutely positioned to roughly match the physical layout from
  `usb_capture_session3/g615lr_zone_map.png` (back edge top, front edge
  bottom, sides left/right, keyboard middle), plus a "Send to Lightbar"
  button. Caught and fixed one real layout bug before committing (the send
  button was initially placed at the same y-coordinate as the front-edge
  zone row, would have visually overlapped it).
- `rog-control-center/ui/pages/aura.slint` -- imports and places the new
  canvas below the existing test button, both kept (one for a known-good
  one-click sanity check, one for full manual per-zone control).
- `rog-control-center/src/ui/setup_aura.rs` -- wires `Lightbar2025Data`'s
  `hex_to_colour` callback (reuses the existing `decode_hex` helper, not a
  new implementation) and `apply_lightbar_2025` (reads all 16 `colour_*`
  properties via their Slint-generated getters, builds the
  `(wire_zone_id, r, g, b)` tuple list in the exact order/IDs
  `rog_aura::lightbar_2025::Lightbar2025Zone` uses, sends via the same
  `write_lightbar_2025_zones` D-Bus method the single test button already
  uses).

**Same honesty caveat as part 1, more so here**: this is meaningfully more
Slint surface area (16 repeated element blocks, absolute positioning
math, a new global, new Rust-side getters for 16 properties) than the
single button, and I have zero ability to preview or compile any of it.
Every individual piece was checked against a real, working pattern
already in this codebase (property getters match `get_led_mode_data()`'s
confirmed convention, `Color::red()/green()/blue()` matches the existing
`decode_hex`/hex-formatting usage, absolute `x`/`y` positioning matches
the existing close-button pattern in `aura.slint`) -- but the *composition*
of 16 of these together, across 3 new/modified files, has no equivalent
this session could directly copy wholesale. Treat this specifically as
"reviewed piece-by-piece against known-good patterns," not "confident
this compiles clean on the first try." If it doesn't build, the layout
math (the `x`/`y` pixel values in `lightbar_2025_canvas.slint`) is the
most likely place for a real Slint compiler error to point -- that part
was hand-calculated, not derived from any working example.

### Final cross-check before handing this off (Windows session 6, part 3)

Did a dedicated pass specifically to verify the invoker addition (parts 1
and 2 above) before calling it done, rather than just re-reading it.
Actually grepped/diffed rather than eyeballing:

- **D-Bus method name** (`write_lightbar_2025_zones`) spelled identically
  across all 6 definition/call sites (`rog-dbus` trait, `asusd` impl, both
  GUI callback sites, CLI, error message). Confirmed via grep, not memory.
- **All 16 Slint property names** (`colour_kbd1` .. `colour_front_bar_right`)
  match character-for-character across the types file, the canvas widget,
  and the Rust getters -- no typos, confirmed via grep across all three
  files simultaneously.
- **The zone-ID mapping itself, the actual safety-critical part** -- both
  hardcoded `(wire_id, r, g, b)` tuple lists (the single test button and
  the canvas's send-all) cross-checked byte-for-byte against the
  authoritative `Lightbar2025Zone` enum values and the
  `matches_human_confirmed_capture` test table. All 16 correct in both
  places. This is the one check worth trusting most -- everything else is
  "will it compile," this is "will it send the right bytes to the right
  zone if it does."
- No naming collisions between the two new Slint files and anything
  already in `rog-control-center/ui/`.

**One real gap surfaced, not buried**: the CLI's `Vec<Lightbar2025ZoneArg>`
for the repeated `--zone` flag has **zero precedent anywhere else in this
codebase** -- every other `argh` option here takes a single value, so
there was nothing local to cross-check the "repeat a flag, each
occurrence parses and appends to a `Vec`" pattern against. `argh` documents
this as supported, and it's a standard, common pattern for the crate, but
it's the one piece of this whole addition that couldn't be verified
against something already proven to compile in this exact repo, the way
everything else was. **If the build fails, check this and the hand-
calculated canvas layout math (previous section) first** -- those are the
two specific, named places most likely to be the actual problem, not a
vague "something in the new code."

## Linux session 5 -- 40s continuous stream test: a subtle flicker, matching the reframing exactly

Directly acted on Windows session 5's two findings. Built
`g615lr-literal-30s-stream.rs` (continuously re-sends all 16 literal
zone bytes from `multizone_12x_confirmed.pcapng`, real priming first, for
40 seconds instead of the 8s every prior test used) to test the newly-
measured 8-12s latency window with real continuous streaming rather than
a one-shot burst. ~2750 full 16-packet cycles sent over 40s (i.e.
`0x0304` writes going out about as fast as the transport allows, far
faster than any natural refresh rate).

**Result: a subtle flicker in the RainbowCycle animation, synced to every
packet write, for the entire 40 seconds -- never resolving to a real
colour.** Human-observed, careful watching required to catch it, but
real and consistent with every single write attempt, the whole run.
**Important clarification from direct observation**: the rainbow did NOT
restart/reset its cycle at each flicker -- it kept smoothly progressing
through its own animation exactly as if uninterrupted, just visibly
perturbed for an instant each time. This means the RainbowCycle engine's
internal state/timing is entirely independent of and unaware of the
`0x04` writes -- it isn't reacting to them at all (no reset, no pause), it
is simply redrawing over them on its own fixed schedule, and the flicker
is only the brief window between our write landing and its next scheduled
redraw. **This is strong, direct evidence for Windows session 5's reframing**:
`0x04` writes are not being silently ignored -- each one visibly perturbs
the display for an instant -- but the EC's own RainbowCycle animation
engine has an internal refresh loop that overwrites the LED buffer again
on its very next tick, before the `0x04` write can persist. Streaming
faster or longer doesn't help, because the competing refresh loop never
stops running in the first place -- there's no race to win, `0x04` always
loses the very next frame.

**This makes the next test unambiguous**, and it's exactly what Windows
session 5 already flagged as untried: explicitly *cancel* the active
`0x5d` RainbowCycle state (a real, non-animated `Static` `0x5d` command)
*before* attempting `0x04`, instead of relying on `0x04` to implicitly
override an animation that's still actively running. Every test so far
either triggers RainbowCycle via the priming triplet immediately before
`0x04`, or skips the triplet and gets an inert dark baseline -- never "a
real, deliberately non-animating `0x5d` state, confirmed settled, then
`0x04`."

Pivoting to compiling and testing Windows session 6's dispatch-wiring
code (D-Bus method, CLI, GUI canvas) before further raw-hardware
iteration, per direct instruction.

## Linux session 5, continued -- compiled and tested Windows session 6's dispatch wiring

**Compiled clean, essentially on the first try.** `cargo check -p rog_aura
-p asusd -p rog_dbus -p asusctl` succeeded immediately, no errors at all.
`cargo build -p rog-control-center` (the Slint GUI) hit exactly one error:
`Lightbar2025Data` wasn't re-exported from `main_window.slint` (every
other Slint global visible to Rust via `include_modules!()` has an
explicit `import`+`export` pair there; the new file added the type but
the top-level re-export line was missed). One-line fix, matching the
existing `AuraPageData` pattern exactly. Both flagged risk areas from
Windows' own review (`argh`'s `Vec<T>` repeated-`--zone` flag, the
hand-calculated Slint canvas layout math) compiled correctly with zero
issues -- the one real bug was somewhere neither flag pointed at.

**Installed and tested the new CLI command through the real dispatch
path** (`asusctl lightbar2025 --zone <id>:<hex>`) for the first time --
every previous `0x04` test this whole investigation went through raw
`rusb`/`HidRaw` directly, bypassing `asusd`'s D-Bus layer entirely. First
attempt looked promising (whole chassis turned static red after
installing+restarting `asusd`) but turned out to be a red herring, same
shape as the earlier false-alarm regression: `asusd` restarting
re-applied a *cached* `Static` red config from earlier in this session via
the classic `0x5d` protocol (which lights the whole chassis as one unit),
completely unrelated to the new command. Re-tested cleanly: dark reset
first (confirmed silent/no visible change), then *only*
`asusctl lightbar2025 --zone 0:00ff00` with nothing else in between.
**Result: `"Sent 1 zone(s)"`, no error, and zero visible effect.**

**This is a genuinely useful negative result, not a wasted test**: it
rules out "maybe the raw test binaries were doing something subtly wrong
that the real application dispatch path avoids." The full, real,
production code path -- D-Bus method, `Aura::write_lightbar_2025`,
`HidRaw::set_feature_report`, same `HIDIOCSFEATURE` ioctl every raw test
also used -- produces the exact same nothing. The dispatch-wiring gap
(Windows session 6's actual contribution) is now closed and confirmed
working end-to-end; the underlying `0x04`/RainbowCycle-override mystery
(Linux session 5's flicker finding, Windows session 5's reframing) is
completely unaffected by it, as expected, since both ultimately hit the
same hardware behavior.

**Next test, unchanged from before this detour**: cancel the `0x5d`
RainbowCycle state explicitly (real `Static`) before attempting `0x04`,
rather than relying on `0x04` to override a still-actively-animating
state. Now has a real CLI command to test it through if useful, though
the raw test binaries remain equally valid for this.

## Linux session 5, continued -- cancel-RainbowCycle test run, plus a second real bug found and fixed

**Ran `g615lr-cancel-rainbow-then-04.rs`** (prime into RainbowCycle -> wait
2s -> explicitly cancel with a real, proven-working `Static` `0x5d`
command, `b3,b5,b4` order -> wait 2s -> stream the literal 16-zone `0x04`
bytes for 20s). **Result, human-observed**: RainbowCycle visibly started
around the 2s mark (confirming priming), then **the cancel step
genuinely worked -- the chassis went dark**, confirming for the first
time that RainbowCycle's animation loop CAN be deliberately stopped on
command, not just raced against. But then: **the entire 20-second `0x04`
streaming phase produced zero visible effect of any kind -- not even the
flicker seen in every prior test.** Stayed uniformly dark the whole time,
and remained dark after streaming stopped.

**This is a different, more specific failure mode than before, and
narrows the theory further.** Every prior streaming test (with
RainbowCycle left running) showed a flicker on every write -- direct
evidence the writes were landing and being displayed for an instant.
This test, with RainbowCycle genuinely stopped first, shows *no* flicker
at all -- not "writes land but get overwritten," but "writes produce no
visible output whatsoever." Working theory, not yet confirmed: the EC's
LED output may not be a simple "buffer + continuous refresh" model where
`0x04` writes a colour and *something* keeps displaying it -- it may be
that display only happens as a side effect of an ACTIVE `0x5d`
mode's own refresh tick reading whatever's currently in the shared colour
buffer. With no mode actively ticking, nothing ever reads that buffer, so
`0x04` writes go in but are never displayed at all, however briefly. This
would mean the flicker isn't "briefly winning" against a competing
writer -- it's the *only* mechanism by which anything gets displayed at
all, and it depends on some *other* active mode still running underneath.

**Second real, independently-fixable bug found and fixed this session**,
via a direct `aura_manager.rs` code audit (not hardware testing): `asusd`'s
`DeviceManager::init_all_hid` deliberately deduplicates hidraw interfaces
to only the *first* one enumerated per USB parent device (comment: avoids
a USB reset loop on hardware with genuinely redundant interfaces). For
G615LR, this is wrong -- interface 0 and interface 1 are NOT redundant,
they speak different protocols entirely, and the dedup logic happened to
keep interface 0 (confirmed via `journalctl`: `self.hid`'s writes were
landing on `/dev/hidraw1`, `bInterfaceNumber 00`, the classic-`0x5d`-only
interface). This meant the new `write_lightbar_2025_zones` D-Bus method
(Windows session 6) could **never** have reached the right interface,
independent of the RainbowCycle question entirely -- a second, separate,
real bug. **Fixed**: added `HidRaw::new_with_interface(id_product,
interface_number)` (`rog-platform/src/hid_raw.rs`) matching by both
`idProduct` and `bInterfaceNumber` instead of first-match; changed
`Aura::write_lightbar_2025` to open its own dedicated handle targeting
interface 1 explicitly every call, instead of using the shared,
dedup-affected `self.hid`. Compiles clean. Not yet verified live at time
of writing -- in progress, see next section if one exists, or `git log`
for the actual install+test commit.

**Important scoping note**: this interface-binding fix only affects the
*new* CLI/D-Bus dispatch path. Every raw-`rusb` test binary in this repo
(`g615lr-*.rs`) already explicitly targeted interface 1 correctly via
`claim_interface(1)` -- so this bug does not explain any of the raw-test
negative results, including the cancel-then-stream darkness above. It's a
real, separate, worthwhile fix for the shipped application, but the core
protocol mystery is unaffected by it and was already being tested
correctly all along.

**Confirmed live, through the real (now correctly-wired) dispatch path**:
dark baseline (`asusctl aura effect static -c 000000`, confirmed dark),
then `asusctl lightbar2025 --zone 0:00ff00` alone -- **zero visible
effect, stayed dark.** Identical outcome to the raw-`rusb`
cancel-then-stream test, now independently reproduced through the actual
application with the interface bug fixed. Two independent transports
(raw USB, real D-Bus dispatch) now agree: against a non-actively-animating
baseline, `0x04` writes are completely invisible, not just overwritten.
This is strong, twice-confirmed support for the "display only happens as
a side effect of an active mode's own refresh tick" theory above.

## Linux session 6 (2026-07-25)

**New external input**: user relayed a Discord conversation with
asus-linux maintainer "NeroReflex", who claimed (a) the N-Key device is
actually 3 HID devices, only one of which ("the vendor one") accepts
`0x04`; (b) the `0x5a`/`0x5e`/`0x5d` identification handshake is *not*
the missing piece ("every modern kernel sends this... no problem"); (c)
what's actually missing is a distinct "go to direct mode" command,
separate from any mode-selection packet.

**Checked (a) directly against this exact hardware.** `lsusb -d
0b05:19b6 -v` already showed `bNumInterfaces=2` (contradicting "3 HID
devices" at the USB level). To rule out a 3rd HID *collection* hidden
inside one of the 2 known interfaces (common for vendor multi-collection
HID devices), dumped and fully parsed both raw report descriptors --
`/sys/bus/hid/devices/0003:0B05:19B6.0006/report_descriptor` (interface
0) and `...0007/report_descriptor` (interface 1), both world-readable, no
sudo needed. Wrote a small standalone HID item parser
(`scratchpad/hid_parse.py`, not committed -- throwaway tool) since no
`hidrd`/similar was installed; first pass had the Main-item tag bits
wrong (used 0-4 instead of the correct 0x8-0xC for
Input/Output/Collection/Feature/EndCollection) and produced garbage,
fixed and reran.

**Result: confirmed exactly 2 HID devices, matching `lsusb`.**
- Interface 0 (`0006`): 9 separate top-level Application collections --
  boot keyboard (report 1), the `0x5a` handshake, the classic `0x5d`
  effect protocol, consumer control (report 2), and others. Notably has
  its own, *unrelated* Report ID `0x04` under Usage Page 0x1 / Usage 0x80
  (System Control, sleep/wake buttons, usage range 0x81-0x83) -- a pure
  numeric coincidence with the lightbar's Report ID 0x04 on the other
  interface; don't confuse the two if grepping captures for "04" near
  interface 0 traffic.
- Interface 1 (`0007`): exactly ONE top-level Application collection
  (Usage Page `0x59`, non-standard/vendor), with report IDs 1 through 6
  all nested inside it as Logical sub-collections. This is the actual
  single "vendor" collection NeroReflex described -- it's just not a
  separate USB interface or HID device on this SKU/firmware, it's this
  one hidraw node (matches everything tried so far: interface 1 was
  already the right target throughout this whole investigation).

**New lead found inside that single collection: Report ID `0x06`, never
tried before.**
```
ReportID = 0x6
Usage = 0x70
  Usage = 0x71, LogicalMin=0, LogicalMax=1, ReportSize=8, ReportCount=1, Feature
```
A single-byte Feature report with a boolean logical range (0/1), its own
report ID, sitting right next to `0x04`'s zone-colour report and `0x05`
(a smaller, structurally similar report -- Usage 0x60, 4 zones instead of
8, same `0x51/0x52/0x53/0x54` per-zone usage pattern as `0x04` -- an
apparent second, smaller batch-write variant, also never tried). `0x06`'s
shape -- tiny, boolean, colocated with the zone writes -- matches
NeroReflex's "go to direct mode" description closely enough to be worth
testing before anything else.

Report IDs 2 and 3 on the same interface (Usage 0x20/0x21, 0x22-0x2d)
look structurally like profile/DPI-style settings (typical of ASUS's
shared ROG vendor HID protocol across mice and keyboards) -- lower
priority, not obviously related to lighting.

**Action taken**: wrote `rog-platform/examples/g615lr-directmode-report6.rs`
-- GET_FEATURE report 6 (see the real current value first), SET_FEATURE
it to 1, GET_FEATURE again to confirm the write landed, then stream the
same known-good literal `0x04` zone packets used in every prior test, on
a clean dark baseline (deliberately *not* priming RainbowCycle first, to
keep the already-confirmed animation-overwrite confound out of this
result). Compiles clean, ran live.

**Result: hypothesis refuted as tested.** `SET_IDLE iface1` stalled
(`Err(Pipe)`, consistent with prior sessions -- task #5, still unexplained
but apparently harmless, Windows doesn't need it either per earlier
findings). `GET_FEATURE report 6` stalled both before *and* after the
write (`Err(Pipe)`, buf stayed all-zero) -- this report ID doesn't support
GET_REPORT, or `0x0306` isn't the right wValue for reading it back; can't
confirm the write's effect via readback either way. `SET_FEATURE report 6
= 01` itself succeeded cleanly (`Ok(2)`, no stall -- the device accepted
the write at the transport level). Then streamed the known-good `0x04`
zone packets for 10s (631 full cycles) on top of it: **zero visible
effect, nothing at all** -- same outcome as every non-animating baseline
tried before. Naive "just flip it to 1 once" version of the direct-mode
hypothesis does not work as tried.

**Reassessing after this failure, per systematic-debugging**: this is
roughly the 7th-8th independent hypothesis to fail against the same
symptom (priming variants, cancel-then-stream, pulse-then-stream,
`0x0305` alone/parallel, 8-zone batch, the interface-binding fix through
real dispatch, now report 6). That pattern -- many different plausible
mechanisms, all producing the identical "nothing happens" result -- points
away from "we haven't found the right byte yet" and toward "we're
guessing at a sequence we've never actually observed." Every capture in
this repo, including `multizone_12x_confirmed.pcapng`, starts *after*
Windows' real vendor driver (Armoury Crate's service) has already put the
device into whatever state makes `0x04` take effect -- none of them show
device *enumeration*/*initialization* itself. NeroReflex's own words
support exactly this: "probably because the driver is sending it way
before you start wireshark." **The actual missing evidence isn't another
guessed report ID -- it's a capture of the real init sequence itself.**
Concretely: on Windows, disable then re-enable the ASUS N-Key device in
Device Manager *while* Wireshark/USBPcap is already running, so the
capture catches full enumeration + whatever the driver sends immediately
after, not just steady-state traffic. This can only be done from the
Windows side (Linux's `hid-generic`/`hid_asus` has no knowledge of this
vendor protocol to replicate). Asked in `QUESTIONS.md`.

**Major methodology upgrade + a definitive negative result (2026-07-25,
still later same session).** Got live Wireshark GUI capture working on
Linux for the first time all session (`usbmon5`, run as the logged-in
user -- unlike our earlier `sudo tshark` CLI attempts, which kept hitting
AppArmor/dumpcap confinement, the GUI app itself already has the right
capabilities/group setup and just works). This finally lets Linux-side
`0x04` tests be verified at the wire level the same way Windows' captures
always have been, instead of relying on visual observation alone.

Fresh data also arrived from Windows this session (`25/123.xml`+
`usb_data.txt`, `25/test123.xml`+`all_usb_data.txt` -- real Aura Creator
captures, both a project XML export and the corresponding USB capture).
Cross-referencing the XML against `usb_capture_session3/ground_truth/
WDL_G615LR.csv`'s `lamp_id` column (index -> wire ID) gave a THIRD
independent confirmation the `Lightbar2025Zone` wire-ID map is correct,
and revealed real structure: each Aura Creator "layer" batches its own
zone group into `0x04` packets (e.g. one layer = all 6 right-side
lightbar zones in one packet, matching the observed `04 06 01 04 00 06
00...` batches exactly). More importantly, it proved animation for this
protocol is **entirely host-rendered** -- the firmware has no onboard
animation engine for `0x04` at all. Real "Breathing" effects are just
Armoury Crate continuously recomputing and streaming a fresh RGBA frame
(alpha channel ramping smoothly, e.g. `06->18->35->58->80->a7->cb->e7->fb
->ff->f5->e0->c2->9d`, a triangle wave) at ~30fps. Every prior `0x04` test
this whole investigation sent byte-for-byte IDENTICAL repeated packets
(constant alpha=0xFF) -- never a genuinely changing frame.

**New hypothesis tested**: maybe this firmware only redraws on an actual
value CHANGE, silently no-op'ing exact repeats. Built
`rog-platform/examples/g615lr-alpha-ramp.rs`: primes, then streams a
SINGLE zone (kbd3, 0x02) with the same real triangle-wave alpha pattern
observed in Windows' capture, at matching ~30ms cadence, for 15s (484
frames). Captured the ENTIRE run live with the newly-working Wireshark
GUI (`usbmon5`) -- first time this session an `0x04` test has been
independently wire-verified on Linux, not just visually judged.

**Result: the capture proves every single one of the 484 writes left the
host correctly** -- `04 01 01 02 00...ff 00 00 [ramping alpha]`, exact
structure, exact zone, exact colour, alpha genuinely changing frame to
frame exactly as intended, steady ~31ms spacing. Byte-for-byte
indistinguishable in structure from a real animated Aura Creator frame.
**Visually: still nothing distinguishable** -- chassis showed plain
RainbowCycle (from the priming step) the whole time, no visible effect
attributable to the alpha-ramping stream. This is the strongest negative
result of the investigation: it definitively rules out "our packets are
being silently dropped/coalesced/malformed before reaching the wire" as
an explanation -- confirmed correct at the USB level, not just assumed
correct from our own packet-builder code. Whatever's missing is
confirmed to be firmware/device-side, not a transport or software bug on
the Linux side. Strengthens the case for the still-pending Windows
pre-init capture being the actual remaining path forward.

**Follow-up, maximally isolated (2026-07-25, still later same session):
priming/animation-engine hypothesis space now definitively closed.**
Built two more tests, asusd fully stopped, NO `0x5d` priming triplet at
all (so the classic animation engine is never triggered into
RainbowCycle or anything else -- removes the confound every other `0x04`
test this session has had), each with an explicit real dark reset
(Static black, zone=None, non-priming order) to confirm a clean starting
baseline:
- `g615lr-corner-no-priming.rs`: front-left corner (wire `0x0D`,
  `CornerFrontLeft`, a real lightbar zone not keyboard), ramping-alpha
  stream (483 frames, same wire-verified-correct shape as the alpha-ramp
  test above).
- `g615lr-kbd1-static-no-priming.rs`: kbd1 (wire `0x00`), constant static
  green, alpha always 0xFF, identical packet repeated 75 times (the
  "classic" single-shot style every prior test this whole investigation
  used, before the alpha-ramp discovery).

**Both failed -- zero visible effect on either.** This closes out the
priming/animation-engine hypothesis space definitively: every
combination of {primed / never primed} x {static constant / ramping
alpha} x {keyboard zone / lightbar zone} has now been tried, and none of
them produce any effect. Notably, this argues AGAINST the remaining gap
being a Linux-side sequencing/software bug specifically -- every
sequencing variable within our control has been varied, independently,
repeatedly, with no change in outcome. Strengthens the case that
whatever's missing is a genuine firmware-side gate (an undiscovered
init/"direct mode" command, matching NeroReflex's original claim, or
something the Windows pre-init capture will reveal) rather than anything
fixable by changing what/when we send from the Linux userspace side.

**Accidental but major discovery (2026-07-25, still later same session):
captured the kernel's own real init sequence for the first time.** When
`g615lr-corner-no-priming.rs` was re-run with a live Wireshark capture
running, releasing the interface at the end (via `RestoreGuard`) let the
kernel's `hid_asus` driver reprobe the device normally -- and Wireshark
caught its ENTIRE real initialization sequence, live, for the first time
all session:
```
5a d0 4e 01                              (query)
5a "ASUS Tech.Inc."                      (0x5a handshake)
5a 05 20 31 00 08 / 5a ec 02 00 00 00    (status + ack)
5d "ASUS Tech.Inc."                      (0x5d handshake)
5e "ASUS Tech.Inc."                      (0x5e handshake -- the one that fails)
5a ba c5 c4 03                           (recurring mystery packet, seen before)
5d b3 00 00 00 00 eb...                  (restore Static blue, real SET/APPLY order)
5d bd 01 aa 1e 00 00                     (restore power states, matches set_power_states' exact format)
```
Confirms NeroReflex's claim precisely -- `0x5a`, `0x5d`, AND `0x5e` are
all genuinely attempted by the real driver, in that order. Critical
realization: **every raw `rusb` test this entire session called
`detach_kernel_driver()` first**, which means this real sequence never
ran during any of them -- detaching the kernel driver prevents it from
running at all. Every prior negative result happened on a device that
never got its real handshake treatment during the test itself (only
whatever happened at actual system boot, hours earlier, in a completely
different context).

**Immediately tested the obvious implication**: sent a single `0x04`
write via the exact production path (`HIDIOCSFEATURE` ioctl on
`/dev/hidraw2`, kernel driver NOT detached at all) moments after this
real reprobe completed -- device genuinely freshly initialized, real
handshakes just finished, using the same access method
`Aura::write_lightbar_2025` actually uses in the shipped code. **Zero
visible effect, same as every other test.**

This closes the last remaining open variable. Full list of things
independently tested and ruled out this session: packet construction
(byte-exact match against Windows' own captures), wire transmission
(Wireshark-verified reaching the device correctly), priming vs. no
priming, static vs. genuinely-animated changing frames, keyboard zone vs.
lightbar zone, and now kernel-driver-detached vs. kernel-driver-attached
via the real production dispatch path. None of them change the outcome.
This is about as exhaustive as Linux-side testing can get without new
external evidence -- strong confirmation this is not a Linux dispatch/
sequencing/packet bug of any kind. The remaining gap is confirmed to be
either the still-failing `0x5e` handshake genuinely gating something we
haven't identified, or a command/sequence neither side has captured yet
-- the Windows pre-init capture (asked in `QUESTIONS.md`) remains the
most promising untried source of new evidence.

**Stale-hidraw-node possibility raised and ruled out.** Good catch mid-
session: the manual `HIDIOCSFEATURE` test above hardcoded `/dev/hidraw2`
from an earlier check, but every reprobe cycle creates a fresh HID device
instance -- a hardcoded node number could go stale if another reprobe
happened in between, silently writing to a disconnected/zombie device
instead of the live one. Re-ran with the interface-1 node resolved fresh
via udev immediately before writing (exactly matching
`HidRaw::new_with_interface`'s own dynamic lookup, never a hardcoded
path), captured live with Wireshark. Wire confirms the packet reached the
device correctly (`04 01 01 0d 00...ff 00 00 ff`, `SET_REPORT`, correct
node). Still zero visible effect. Rules out stale-node ambiguity as an
explanation too -- this negative result holds with zero possible doubt
about which node was actually written to.

## BREAKTHROUGH (2026-07-25, still later same session): a THIRD protocol,
already implemented in this repo for sibling hardware, actually works.

Dug into the repo itself rather than guessing new bytes, per direct
instruction. Found `rog-aura/src/keyboard/advanced.rs`'s `LedUsbPackets`
-- a complete, real, ALREADY-IMPLEMENTED "custom mode" protocol: `0x5d`
with mode byte `0xbc` (distinct from `0xb3` "builtin", used for every
"priming"/effect-apply this whole session). `get_init_msg()`'s own doc
comment: "Initialise and clear the keyboard for custom effects, this
must be done every time mode switches from builtin to custom." Never
sent once this entire investigation.

**The smoking gun**: G615LR's own `aura_support.ron` entry has
`layout_name: "g634j-per-key"` -- it already explicitly references the
G634J per-key layout. G634J and G635L (closest sibling models, same
generation, same `basic_modes`/`power_zones`) both have `advanced_type:
PerKey`, meaning they already get real per-key/zoned direct addressing
through this exact mechanism. G615LR's `advanced_type` was simply left
as `r#None` -- nobody had ever tried routing it through this
already-working path. All prior investigation focused entirely on the
separate `0x04` protocol.

**Tested directly** (`rog-platform/examples/g615lr-perkey-zoned-protocol.rs`):
sent the real init message (`5d bc 00...`), then `LedUsbPackets::new_zoned(true)`'s
packet format (`5d bc 01 01 04...`) with all 4 keyboard zones
(`ZonedKbLeft/LeftMid/RightMid/Right`, offsets 9/12/15/18) and all 6
lightbar codes (`LightbarRight/RightCorner/RightBottom/LeftBottom/
LeftCorner/Left`, offsets 27/30/33/36/39/42) set to distinct colours.

**Result: keyboard zones lit up correctly, independently, for the first
time all session.** Lightbar codes did not light (unlike the classic
`0x5d b3` zone1/zone4 "bleeds into lightbar" bug -- this time it's a
clean non-response, not a wrong-zone bleed, suggesting the 6-lightbar-
code addressing this protocol uses for G634J/G635L's hardware simply
doesn't map onto G615LR's actual lightbar wiring, consistent with
G615LR likely having a genuinely different, more granular 16-zone
lightbar that needs the separate `0x04` protocol specifically -- the
existing doc comment on G615LR's `aura_support.ron` entry may have been
right about that part all along, just wrong to conclude the KEYBOARD
side needed `0x04` too.

**Corrected course before testing true per-key**: G615LR is a genuine
4-zone backlit keyboard, not per-key RGB hardware (`basic_zones:
[Key1,Key2,Key3,Key4]`, and directly confirmed by the user) -- there is
no physical way for 90+ individual keys to display different colours on
wiring that's only ever split into 4 zones. Skipped the full per-key
test as physically meaningless for this hardware; pivoted to more
targeted follow-ups instead.

**Custom-mode-init immediately before `0x04`, tested and refuted**: given
`5d bc` (custom mode) just proved real and working for keyboard zones,
tested whether `0x04` shares the same prerequisite -- real custom-mode
init, then the same wire-verified-correct `0x04` alpha-ramp stream.
Zero effect, same as every other combination.

**Power-zone-disabled theory, refuted from already-captured data (no new
hardware test needed)**: decoded the real kernel restore command
captured earlier tonight (`5d bd 01 aa 1e 00 00`) against
`LaptopAuraPower`'s documented bit layout (`rog-aura/src/keyboard/
power.rs`). `aa`=Keyboard boot/awake/sleep/shutdown all on, Logo all off
(correct, no logo). `1e`=**Lightbar boot/awake/sleep/shutdown ALL ON
too** -- already fully powered by the device's own remembered defaults,
restored automatically. Not a disabled-power-zone issue.

**Physical wiring explanation for the classic-protocol zone1/zone4
lightbar-bleed finding from earlier tonight**: the classic `0x5d b3`
protocol addresses a whole physical LED *strip* by a single zone byte
(no per-LED-position addressing); the newer `0x5d bc` custom-mode
protocol has distinct byte positions for `ZonedKbLeft` vs
`LightbarLeft`/etc (genuine per-position addressing within a chain).
That difference explains the bleed exactly: kbd1's LEDs and the entire
left lightbar chain are almost certainly on the same physical
addressable-LED daisy-chain (kbd4 + right lightbar likewise), with
zones 2/3 on their own separate short chain. A whole-strip-only protocol
necessarily lights the entire chain; a position-addressed protocol can
target just one LED's position within it -- consistent with kbd1 lighting
in isolation under `0x5d bc` tonight. Matches the physical zone map too
(kbd1 sits at the leftmost keyboard position, adjacent to where the left
lightbar chain begins).

**Scoped, safety-conscious brute force of the `0x5d bc` byte-position
space, tested and exhausted**: user asked directly whether this risks
bricking anything -- answered no, provided report ID (`0x5d`) and mode
byte (`0xbc`) are held constant and only the RGB-value byte *position*
varies; this stays entirely within the already-proven-safe "set LED
colour" command space, never touches a different report ID/subcommand
(sleep/power controls, etc -- the only real risk category). Built and
ran two sweeps:
- `g615lr-bruteforce-offset.rs`: every plausible 3-byte-aligned offset
  (5-61, the maximum possible within a single 64-byte packet) in the
  `new_zoned()` packet, skipping the known-working keyboard offsets.
  **Nothing.**
- `g615lr-bruteforce-row11.rs`: extended past the single-packet limit
  using real code-grounded leads -- `new_per_key()` only allocates 11
  packet rows (indices 0-10), but its own `rgb_for_led_code` match arms
  reference row 11 for every lightbar/lid `LedCode` in non-zoned mode
  (out-of-bounds, never actually reachable in the existing code -- this
  path was never finished upstream). Swept row 10 (the one legitimately-
  allocated row never tried) at all its own column positions, then
  manually constructed the referenced-but-never-built "group 11" packet
  and tested the EXACT column positions the existing code already points
  at for `LightbarRight/RightCorner/RightBottom/LeftBottom/LeftCorner/
  Left` and `LidLogo/LidLeft/LidRight`. **Nothing.**

This closes out the reasonably-guessable `0x5d bc` byte-position search
space. Every offset within the single zoned packet, the one real spare
per-key row, and the exact positions the existing (incomplete) code
itself pointed at for lightbar/lid have all been tried. Whatever
addresses G615LR's actual lightbar under this protocol family -- if
anything does -- is not in the parts of the byte space explored tonight.

**Extended further** (`g615lr-bruteforce-allgroups.rs`): swept the
previously-untested group values 12-15 (the group byte at offset 6 is
`group << 4`, a 4-bit field -- 15 is the maximum possible value, fully
novel territory) plus every unmapped/undocumented offset within groups
0-9 (the other per-key rows, skipping only the positions already
documented as real keyboard keys in `rgb_for_led_code`). User observed,
live: groups 0, 12, 13, 14, and 15 all produced the IDENTICAL result --
the same 4 keyboard zones lighting up whenever the offset was 9/12/15/18,
regardless of group value. Combined with groups 10/11 showing the same
pattern earlier, this spans effectively the entire possible group-byte
range (0-15) with a consistent, well-evidenced result.

**Conclusion**: G615LR's EC firmware almost certainly ignores the group
byte entirely for this protocol. It physically can't implement true
per-key/per-row addressing -- the hardware only has 4 zones' worth of LED
circuitry (`basic_zones: [Key1,Key2,Key3,Key4]`) -- so it very likely
just always reads a fixed set of byte positions (9/12/15/18) as "the 4
zone colours," no matter what group/row value a packet claims to target.
The group byte is a vestige of the shared protocol definition used by
genuine per-key models (G634J/G635L), carried over but functionally
inert on this board. This also explains every negative result in this
whole `0x5d bc` brute-force effort at once: every other offset in every
group corresponds to individual-key LED data that simply doesn't exist
as physical hardware here -- not a wrong guess, just no LED to address.

**Strongest implication**: the lightbar is very likely not reachable
through `0x5d bc` in ANY group or offset -- the keyboard EC that handles
this protocol almost certainly has no wiring to the lightbar at all,
consistent with it being a physically separate controller chip. This
closes out the `0x5d bc` protocol family as a path to the lightbar
specifically; further brute-forcing this exact byte space is unlikely to
find it. Reinforces that `0x04` (or a still-undiscovered protocol) really
is the correct track for the lightbar, independent of tonight's real win
on the keyboard-zone side.

**Further repo-digging (2026-07-25, still later same session), per direct
instruction to keep digging rather than blind brute-force.** Checked
every other LED/lighting-adjacent subsystem in this codebase:

- `rog-slash` (`rog-slash/src/usb.rs`): a real, different protocol on
  report `0x5d` with never-before-tried subcommands (`0xd2/0xd3/0xd4/
  0xd7/0xd8`, 32-byte packets) for the "Slash" scrolling LED strip
  feature on certain 2024/2025 ROG lid designs (GA403/GA605/GU605/G614F).
  Its `report_id()` maps our exact product ID (`0x19b6`) to report `0x5d`
  for several of these models -- confirms ASUS reuses the same USB PID
  across many distinct physical laptop generations, a useful general
  caveat. But `SlashMode`'s variants (`Bounce`, `Flow`, `BitStream`,
  `Transmission`, `Spectrum`, `GameOver`, `Buzzer`...) are clearly an
  animated display-strip feature, not a chassis lightbar -- genuinely
  different hardware. Confirmed via `journalctl` earlier tonight that
  G615LR is already correctly excluded by board name
  (`get_slash_type()`) -- a deliberate, correct exclusion, not an
  oversight like the per-key `advanced_type` was. Not tested against
  hardware; insufficient justification versus the per-key lead.

- `rog-anime` (`rog-anime/src/usb.rs`): report ID `0x5e` (`DEV_PAGE`) is
  a REAL, actively-used data report for the AniMe Matrix LED-matrix
  display protocol (subcommands `0xc0/0xc2/0xc3/0xc4/0xc5`), not just an
  identification handshake. Its own `pkts_for_init()` sends the exact
  same `0x5e` + "ASUS Tech.Inc." handshake pattern found failing on our
  device tonight -- confirms `0x5e` is ASUS's shared generic vendor-
  handshake report number, reused across multiple different product
  lines, not something unique or lightbar-specific. Explicit code
  comment: "The currently known USB device for the AniMe Matrix is
  `0x193b`... `0x19b6` is a different ASUS USB device (the N-KEY keyboard
  interface) -- historical comment was incorrect." Different physical USB
  product ID entirely -- not testable against G615LR, correctly excluded.

- `ctrl_platform.rs`/`asus_armoury.rs`: zero HID/USB involvement at all --
  entirely sysfs-based (`rog_platform::platform::RogPlatform`,
  `FirmwareAttributes`, the `asus-nb-wmi`/ACPI kernel interface). Fan
  curves, performance profiles, GPU MUX -- a completely separate
  subsystem, no lead here.

**New hypothesis directly inspired by AniMe Matrix's separate PID,
checked and ruled out**: does G615LR's lightbar live on its OWN separate
USB device/product ID, the same way AniMe Matrix (`193b`) is physically
distinct from the N-Key keyboard device (`19b6`)? Checked `lsusb -d
0b05:`: only ONE ASUS vendor device exists on this system at all -- the
N-Key device, same one used all session. No separate lightbar device.
Combined with the full byte-level report-descriptor parse of both
interfaces done earlier tonight (every report ID on both interfaces
already accounted for, no hidden third collection), this rules out "a
device we haven't found yet" as an explanation -- whatever controls the
lightbar, if anything on this exact wire, must go through a report ID
already known about (most likely still `0x04`), not an undiscovered
device elsewhere on the bus.

**One more AniMe angle checked and ruled out concretely**: `rog-anime`
has a "STRIX-class" (G635L/G835L) variant using report `0x5e` as a REAL
data protocol (`[0x5e, 0xc0, 0x02, START_LO, START_HI, LEN_LO, LEN_HI]` +
raw colour data, addressing an 810-LED matrix) -- briefly looked
extremely promising, since G635L is one of G615LR's closest siblings and
this would have explained why our `0x5e` handshake doesn't echo like
`0x5a`/`0x5d` (if `0x5e` were actually this different data protocol, not
a handshake, on our hardware too). Ruled out on closer inspection:
`PROD_ID` in `rog-anime/src/usb.rs` is a fixed constant (`0x193b`)
regardless of which `AnimeType` board is detected -- even for STRIX-class
G635L, this protocol only ever talks to the separate AniMe display panel
device, never the N-Key keyboard device our lightbar lives on. Same dead
end as AniMe generally, just reached from a more specific angle; not
tested against hardware given this doesn't apply to a `19b6`-only system.

**Fixed a real accuracy problem while digging**: `docs/g615lr-aura-
protocol.md` is a stale snapshot from the very first Windows-only phase
of this investigation (predates any Linux hardware testing) and still
claimed report `0x5d` "does NOT produce any visible effect" -- flatly
contradicted by this session's confirmed-working whole-chassis modes.
Added a stale-document notice pointing to `HANDOFF.md`/`CLAUDE.md` as
the authoritative source, left the file in place since its `0x04` packet
format documentation is still accurate.

**"Does the lightbar ever get woken up?" -- checked via kernel source AND
a live capture, both say no (2026-07-25, still later same session).**
User raised a sharp hypothesis: keyboard zones lit up tonight, lightbar
never did -- what if there's a "wake the lightbar" step that just never
happens, the same way a keyboard backlight often needs an explicit
restore after sleep? Checked two ways:

1. Fetched the real Linux `hid-asus.c` kernel driver source in full
   (verified verbatim, not paraphrased). `asus_resume()` (the PM resume
   callback) sends exactly `[FEATURE_KBD_REPORT_ID(0x5a), 0xba, 0xc5,
   0xc4, stored_brightness]` -- this is the exact "`5a ba c5 c4`" packet
   found mysterious earlier tonight, now fully explained: it's the
   keyboard-backlight-brightness restore-after-resume command, trailing
   byte = the actual brightness value. **Confirms the kernel driver has
   ZERO lightbar-awareness anywhere** -- not filtered/suppressed, just
   never written with any knowledge this separate protocol exists on this
   board. `asus_raw_event` filtering `0x5d`/`0x5e` only discards
   unsolicited INBOUND reports (interrupt IN noise), unrelated to
   outbound command capability. Same gap confirmed in this repo's own
   code: the sleep/wake handler in `asusd/src/aura_laptop/trait_impls.rs`
   only calls `write_current_config_mode` (classic `0x5d` only) on wake --
   `write_lightbar_2025` isn't wired into any automatic path at all, 100%
   manual-trigger only.

2. **Tested live**: captured a real suspend-to-idle/resume cycle with
   Wireshark on `usbmon5` (first time this exact scenario tried this
   session -- every prior capture came from process-level interface
   detach/reattach, never a genuine PM sleep/wake). Filtered to just our
   device (address 2, since `usbmon5` also captures the Bluetooth adapter
   and webcam sharing the same bus -- their traffic was initially
   miscategorised as false-positive "0x04"/"0x06" reports before
   filtering by device address caught the mistake). **Result: caught
   `asus_resume()` firing live, multiple times** (`5a ba c5 c4 00`, `5a ba
   c5 c4 00`, `5a ba c5 c4 03` -- brightness value changing between
   events), confirming the kernel source finding on the actual wire for
   the first time. **Nothing else happens** -- no `0x5e`, no `0x5d`
   handshake, no `0x04`, nothing lightbar-related at all. Notably LESS
   than the earlier kernel-reprobe capture (which had the full `0x5a`/
   `0x5d`/`0x5e` three-way handshake) -- PM resume and a full USB re-probe
   are architecturally different events; resume just changes power state
   on an already-connected device, it's a lighter event than the fresh
   re-enumeration our `detach_kernel_driver` cycling was actually
   triggering.

**Closes the Linux side of this hypothesis definitively**: Linux's own
resume path does nothing for the lightbar, confirmed live, not just
inferred from source. **Still genuinely open**: whether Windows'
Armoury Crate does something lightbar-specific on an actual suspend-to-
RAM/resume cycle -- Windows has only tried a Device Manager disable/
enable of the single `MI_01` HID collection so far (Windows session 7),
never a real sleep/wake. That's the one clean, concrete, still-untried
ask remaining for whoever picks this back up.

## Linux session 6, responding to Windows session 9's BREAKTHROUGH (2026-07-26)

Windows pushed a major result: a real, wire-verified, live-confirmed
`0x04` lightbar activation from code for the first time this entire
investigation, using a `count=5` multi-zone packet (`kbd1-4` at
near-zero alpha, `back_right` lightbar zone at full alpha yellow-green).
See `HANDOFF.md` "BREAKTHROUGH (Windows session 9)" above for the full
decode and packet bytes, and "NOT resolved" for the open question their
own isolation retest raised (`count=1` on the same zone gave opposite
results on two consecutive runs).

**Immediately replicated on Linux**, byte-for-byte, via raw `libusb`
(`rog-platform/examples/g615lr-count5-multizone.rs`, real `b3/b4/b5`
RainbowCycle priming matching what Windows used): **result differs from
Windows.** Whole chassis (keyboard AND lightbar) just continued
RainbowCycle, no independent override -- same confound as every
`count=1` test all session, not the clean "lightbar yellow-green,
keyboard off" result Windows got with the identical bytes. Retried the
exact same script a second time (Windows' own isolation test flipped
between runs with no code change, so a retry was worth checking before
concluding anything) -- **consistent negative both times**, rules out
simple run-to-run non-determinism as the explanation on our end.

**Tried without priming too** (`g615lr-count5-no-priming.rs`, real dark
reset instead of the RainbowCycle-triggering triplet) -- **zero effect,
stayed dark.** So neither priming nor no-priming produces the Windows
result; priming state isn't the variable that explains the platform
difference (Windows succeeded WITH priming, we failed both ways).

**New, real, reproducible discrepancy found while chasing this**: every
test above used raw `libusb` with `detach_kernel_driver()` -- the
Linux kernel's HID driver is completely unbound from the device while we
write to it, unlike Windows' `HidD_SetFeature`, which goes through the
always-attached HID class driver stack. Tested the identical `count=5`
packet via `HIDIOCSFEATURE` on `/dev/hidraw2` instead (kernel driver
NOT detached, closest match to what Windows actually does) -- **fails at
the transport level with a genuine `EPROTO` (USB stall)**, confirmed NOT
caused by `asusd` contention (retried with `asusd` fully stopped, same
error). This is significant: a `count=1` packet succeeded fine via this
exact same `HIDIOCSFEATURE` path earlier tonight
(`hidraw_fresh_lookup_wire_verified.pcapng`) -- so something about the
*attached* kernel driver specifically rejects multi-zone (`count>1`)
Feature report content that a single-zone packet doesn't trigger, while
bypassing the kernel driver entirely (raw `libusb`) lets the identical
bytes through at the transport level (just with no visible effect).

**Not yet resolved, real lead for whoever picks this up**: GitHub rate-
limited both `WebFetch` and direct `curl` access to `hid-asus.c` before
`asus_report_fixup` could be checked (the one place this driver actively
rewrites/validates outgoing report data, most likely explanation for a
content-dependent -- not length-dependent, both are 51 bytes -- rejection
like this). Check that function first in a future session; if it
restricts/rewrites Feature reports based on zone count or similar, that
would directly explain why `count>1` behaves differently under the
attached driver specifically.

**Where this leaves the count>1 hypothesis**: genuinely still open, now
with MORE evidence than before but no resolution. Windows' own account
already flagged their `count=1` isolation test as contradicting itself
between runs, unresolved on their end too. Given tonight's new finding
(attached driver actively rejects `count=5` via `HIDIOCSFEATURE`,
detached `libusb` accepts the bytes but shows nothing), the cleanest
next test for whoever continues this: try `count=5` via `HIDIOCSFEATURE`
successfully first (once whatever's causing the `EPROTO` is understood
or worked around), since that's the closest possible match to Windows'
own successful transport path -- raw `libusb` with a detached driver may
simply never have been capable of reproducing this regardless of packet
content, since it's a fundamentally different code path than what
Windows/the real working Linux `asusd` dispatch (`HidRaw::
set_feature_report`, also `HIDIOCSFEATURE`-based) would use.

**Real hazard recurrence, noted for future sessions**: the ~6-minute
`g615lr-bruteforce-allgroups.rs` sweep got interrupted mid-run (Ctrl+C),
which killed the built-in keyboard again -- `SIGINT` terminates the
process immediately without unwinding the stack, so even the `Drop`-based
`RestoreGuard` never runs (that only fires on a clean return or a panic,
not a signal kill). Fixed the same way as before (`echo -n "5-4:1.0" |
sudo tee /sys/bus/usb/drivers/usbhid/bind`, same for `5-4:1.1`). Worth
adding a `Ctrl+C` signal handler to these long-running examples in a
future session so this stops recurring on any interrupted run, not just
panics.

**Also worth relaying**: a real, live Linux capture of classic-protocol
GUI mode switching (`testtt.pcapng`, RainbowWave -> Pulse via
`rog-control-center`) confirmed `write_effect_and_apply` genuinely never
sends the `b3/b4/b5` "priming" triplet for classic mode changes -- that
sequence is Armoury-Crate-specific behavior seen in the original Windows
capture, not a universal wire-level requirement. It IS still present in
Windows' real `0x04` capture though (checked `usb_data.txt` again:
priming triplet fires twice before the first `0x04` write), so it's
correctly included in `g615lr-alpha-ramp.rs` and other `0x04` tests --
just worth being precise that its role is specific to `0x04` sessions,
not a general "wake the device" requirement.

**Follow-up hypothesis (2026-07-25, later same session), refuted**: live
interactive user testing of the CLASSIC `0x5d` protocol's `AuraZone`
(`Key1-4`/`Logo`/`BarLeft`/`BarRight` -- a completely different, older,
7-value zone system from `Lightbar2025Zone`'s 16 wire IDs) surfaced a real
finding: once ANY animated mode is running globally (zone=None), NEW
zone-scoped `0x5d` writes on top of it get ABSORBED into that
already-running animation loop rather than creating independent state --
live-confirmed: with `breathe --zone 1` (blue) already animating, sending
`static --zone 2 -c ffff00` (a STATIC command) made zone 2 start
BREATHING green, synced with zone 1. (Caveat found along the way: an
earlier "entire keyboard breathing" result was a false alarm caused by a
stray `rog-control-center` GUI process left running in the background
from earlier testing, independently issuing its own D-Bus calls and
corrupting `AuraConfig.multizone_on` -- always `pkill -f
/bin/rog-control-center` before isolated CLI zone testing.)

Given `0x04` streaming against an active RainbowCycle produces a flicker
(writes land, get overwritten next tick) but RainbowCycle is purely
procedural with no colour parameter to read, the natural next hypothesis
was: does an active **Breathe** loop (which demonstrably re-reads colour
data every tick, per the zone-2 finding above) actually pick up and
persistently render `0x04` zone writes on top of it, unlike RainbowCycle?
Tested directly (`rog-platform/examples/g615lr-breathe-then-04.rs`): dark
reset -> global Breathe (red) via real `b3,b5,b4` -> confirmed animating
-> streamed `0x04` for a single zone (kbd3, green) for 15s on top.
**Result: completely inert, not even a flicker.** The classic protocol's
"absorb new writes into the active loop" behaviour does NOT cross over to
`0x04` -- the two protocols are using genuinely separate internal
state/rendering in firmware, not one shared engine either can feed. Rules
this specific idea out; does not change the core conclusion above (still
need the Windows pre-init capture).

**Real, separate hazard hit and fixed while building this test**: the
test binary panicked once (an off-by-one hex string, 52 bytes instead of
51) AFTER `detach_kernel_driver(0)` but before the cleanup code that
reattaches it -- interface 0 is the SAME USB interface the physical
keyboard's boot-input collection lives on, so this silently killed the
built-in keyboard until manually fixed via `echo -n "5-4:1.0" | sudo tee
/sys/bus/usb/drivers/usbhid/bind` (restarting asusd does NOT fix this,
kernel driver binding is independent of any userspace daemon). Fixed the
test itself with a `Drop`-based `RestoreGuard` that reattaches both
interfaces' kernel drivers unconditionally, even during a panic unwind --
worth retrofitting onto every other `g615lr-*.rs` raw-`rusb` example that
detaches kernel drivers, since every one of them has the same latent risk.

## Windows session 7 (2026-07-25) -- the pre-init capture Linux asked for, finally done

Answered Linux session 6's `QUESTIONS.md` ask directly: capture the real
device init/enumeration sequence on Windows, which no capture in this
repo had ever shown live before now (only Linux's kernel-reprobe capture
had, and that's a different OS's driver stack).

**Method note for future sessions**: `USBPcapCMD.exe` run with no
arguments to interactively list devices hangs forever waiting on stdin
when launched through a non-interactive tool -- don't do that, kill it
immediately if it happens (it can leave the USBPcap driver in a bad state
that makes every subsequent `tshark -i "\\.\USBPcapN"` capture fail with
"File type is neither a supported pcap nor pcapng format (magic =
0x00000000), 0 packets captured" until the stuck process is killed).
Also: **never force-kill (`taskkill /F`) a live `tshark -w` capture** --
it discards the buffered pcapng data, leaving only the 288-byte section
header with zero packets. Always give `tshark` a fixed `-a duration:N`
so it exits and flushes on its own.

**First attempt, real negative result**: restarted "ASUS AURA SYNC
lighting service" (`LightingService`, the actual Windows service that
owns the vendor HID protocol) while capturing on the correct interface
(`USBPcap3` -- root hub numbering shifted again, `USBPcap1`/`USBPcap2`
both captured zero ASUS traffic this session, matching the known
instability). **Result: no handshake at all.** The capture shows a plain
device redescribe (`GET_DESCRIPTOR` x3, standard enumeration boilerplate)
immediately followed by the SAME already-known `0x0305` `SET_REPORT
Feature ReportID=5` stream resuming (`05 01 00 00 0f 00 ff 00 00 [phase
byte]`, ~60ms cadence, only the last byte changing -- RainbowCycle's own
free-running hue counter), byte-for-byte identical before and after the
restart, sampled across the whole ~35s window. A driver-service restart
does NOT trigger the real init handshake -- confirms that handshake is
tied to actual device-level (re)connection, not to the userspace service
process restarting.

**Second attempt, the real thing**: Device Manager, disabled then
re-enabled the SPECIFIC HID collection carrying the vendor protocol
(`HID\VID_0B05&PID_19B6&MI_01\...`, "HID-compliant device", the one
matching Linux's own interface-1 report-descriptor parse from session 6
-- confirmed isolated from the physical keyboard/mouse, which live under
the separate `MI_00` composite subtree and correctly showed "Disable"
greyed out when tried first). Captured live on `USBPcap3` across the
whole cycle.

**Result: a real, live-captured `0x5d` handshake, the first time this
exact sequence has been caught on the Windows side rather than inferred**
-- fired twice, back-to-back:
```
5d bf 00 00 00 00 00...                         (query)
5d 41 53 55 53 20 54 65 63 68 2e 49 6e 63 2e...  ("ASUS Tech.Inc.")
5d 05 20 31 00 10 00...                          (status)
5d 05 20 31 00 10 03 01 01 02 25 05 01 02 46...  (extended status, interrupt IN)
5d ec 02 00 00 00                                (ack, interrupt IN)
```
Plus a genuine `GET_DESCRIPTOR(String)` read returning `"ASUSTek
Computer Inc."` (UTF-16LE) -- a real enumeration-level request, not
something a driver would fabricate in software alone. This matches Linux
session 6's kernel-reprobe `0x5d` block structurally, byte for byte on
the parts that overlap.

**But**: no `0x5a` query/handshake anywhere in the capture, no `0x5e`
handshake anywhere, and -- the actual thing we were hoping to find --
**no distinct "go to direct mode" command**. Searched the full 552-packet
capture for any `SET_REPORT` to report `0x04` or `0x06` (the two
candidates from Linux session 6): neither appears. Once the `0x5d`
handshake block finished, traffic went straight back into the identical
`0x0305` RainbowCycle stream, same structure as every other capture this
whole investigation. No lightbar write, no toggle, nothing new.

**Interpretation**: disabling/re-enabling a single HID collection
(interface 1 only, not the whole composite USB device) is evidently
enough to make the driver/service layer notice and replay ITS OWN `0x5d`
init sequence in software, but is not the same as the full bus-level
re-enumeration Linux's kernel reprobe caught (which showed `0x5a` AND
`0x5d` AND `0x5e`, all three). If `0x5a`/`0x5e` only fire on a genuine
hardware-level bus reset across the whole composite device, disabling
just one child collection may not be enough to trigger them -- the
`0x04` prerequisite, if there is one, might specifically live inside
whatever `0x5a` or `0x5e` are supposed to accomplish and never got a
chance to run here.

**Real, honest conclusion**: this closes out "has Windows ever captured
its own real init sequence" (yes, now it has, and it matches Linux's
capture where they overlap) but does NOT close out the underlying
mystery -- no direct-mode command was found, and the two most complete
handshakes (`0x5a`, `0x5e`) still remain uncaught on Windows in this
exact scenario. **Suggested next step for whichever side picks this up**:
try disabling the WHOLE composite USB device (not just the MI_01
collection) via Device Manager's "Devices by connection" view, if
Windows permits it without also dropping the physical keyboard for an
unacceptable duration -- that would be the closer match to what actually
produced `0x5a`/`0x5e` on Linux's kernel reprobe.

Capture: `usb_capture_session6/pcap3_real_disable_enable.pcapng`
(45s window, 552 packets, `USBPcap3`). The earlier LightingService-restart
capture was a real negative result too but wasn't retained as a raw file
-- the finding (byte-for-byte identical `0x0305` stream, no handshake) is
fully described above instead.

## Windows session 8 (2026-07-25) -- a genuinely new, untested candidate found by diffing the `25/` real captures

User pushed back on treating any "it's impossible" read of the accumulated
negative results as settled, and specifically asked to diff the user's own
`25/test123.pcapng` and `25/123.pcapng` -- real, working Aura Creator
sessions, captured live, that nobody had actually diffed byte-for-byte
against our failed reproduction attempts before now (they'd only been
used previously for the zone-ID triple-confirmation in Linux session 6).

**Method**: tallied every `SET_REPORT`/class-request (`bmRequestType ==
0x21`) in both captures by total URB length, to find anything that isn't
one of the four already-fully-characterized shapes (`8` = zero-data
request like `SET_IDLE`, `72` = the `0x5d` handshake's 64-byte payload,
`18` = the `0x0305` stream's 10-byte payload, `59` = the `0x04` zone
write's 51-byte payload). Both files have a small number of leftover
frames that don't fit any of those buckets.

**Found**: a `SET_REPORT` to **Report ID 1, Report Type = Output (not
Feature -- every prior test all investigation used Feature exclusively)**,
`wIndex = 0`, `wLength = 2`, data `01 01`. Full bytes: `09 01 02 00 00 02
00 01 01`. Confirmed in BOTH real captures, at the same structural
position each time:
- `test123.pcapng`: fires once very early (frame 31, t=2.30s, right after
  the initial descriptor reads, before the priming triplet even starts),
  then fires again at frame 107, t=20.566482s -- **5 microseconds after**
  frame 106's first real `0x04` zone write (t=20.566477s). A third
  occurrence at frame 537, t=25.52s, fires paired with a second, distinct
  report -- see below -- in the middle of what looks like a full `0x5d`
  handshake replay mid-session (surrounded by more 72-byte `0x5d`
  SET/GET pairs, unrelated to the very first one at session start).
- `123.pcapng`: fires at frame 56, t=15.931347s, with the first real
  `0x04` write following immediately after at frame 71, t=15.984701s
  (~0.05s later).

**Also found, only in `test123.pcapng`, paired with the third report-1
occurrence**: `SET_REPORT` to **Report ID 0, Report Type = Output**,
`wIndex = 0`, `wLength = 1`, data `01`. Full bytes: `09 00 02 00 00 01 00
01`. Frame 536, t=25.521717s, immediately followed by frame 537's report-1
write one millisecond later (t=25.521748s) -- a paired write, both firing
together during that mid-session `0x5d` replay.

**Correction, same session, before this got written up wrong**: first
draft of this section claimed the `ReportID=1` Output write was
completely untested. False -- checked the actual test code before
finalizing and found `rog-platform/examples/g615lr-alpha-ramp.rs` line
84 already sends exactly this (`send!("0x0201 (01 01) iface0", 0x09,
0x0201u16, 0u16, &[0x01, 0x01]);`), and Windows'
`g615lr_priming_then_static_hold.ps1` (session 3) has sent it since
session 3 too, labeled "wake" in a comment. Both already tried it, as
part of priming, and both still got zero visible effect. Correcting the
record here rather than silently editing it away, per this file's
append-only convention.

**What's actually still untested, after that correction**: every existing
script (`g615lr-alpha-ramp.rs` included) sends the `ReportID=1` Output
write exactly ONCE, before the `0x5d` priming triplet, then never touches
it again for the whole streaming run. The real captures don't do that --
they send it again a SECOND time, specifically right at the moment `0x04`
zone traffic actually starts (5 microseconds before frame 106's first
`0x04` write in `test123.pcapng`; ~50ms before the first `0x04` write in
`123.pcapng`), not just once at overall session start. `test123.pcapng`
also shows a third occurrence, paired with the `ReportID=0` write, ~5s
later mid-stream (t=25.52s) while `0x04` traffic is already flowing
continuously (checked: no gap in the surrounding `0x04` stream, so this
isn't a clean "new layer" boundary, more likely a UI-driven event on
Aura Creator's side -- inconclusive on its own).

**So the real, still-open, worth-testing question**: does re-sending
`SET_REPORT(Output, ReportID=1, wIndex=0, data=[0x01,0x01])` a SECOND
time, immediately before the first `0x04` write (i.e., right after
priming finishes, right before streaming starts -- not just once before
priming like every script currently does), change the outcome? Nobody
has tried the repeated-invocation timing, only the single-invocation-
before-priming timing that's already failed. Low-cost to test: one extra
line in an existing script, no new hardware access needed. Given it's
already failed once in this exact form, treat this as a low-confidence
lead worth a quick try, not a strong candidate -- the earlier framing in
this section overstated its novelty.

Raw analysis was done against `25/test123.pcapng` and `25/123.pcapng`
directly (both already in the repo, pushed by the user); no new capture
files were added this session, only this write-up.

**Real hardware test run, real hazard hit and noted for future sessions.**
Ran the repeated-invocation test above live (`usb_capture_session6/
test_repeated_report1.ps1`) against a RainbowCycle-primed baseline. Both
`0x0201` Output writes failed at the Windows API level this run
(`HidD_SetOutputReport` returned `ERROR_INVALID_FUNCTION`, err=1) -- the
priming triplet and `0x0305` handshake still succeeded. User observed a
real, visible, non-RainbowCycle-explainable effect during/after the run:
kbd3 turned green (matching the streamed `0x04` write) AND several
lightbar zones (left/right/front) went dark -- RainbowCycle cycles
through colours, it doesn't turn zones off, so this wasn't just
coincidental colour-cycling. **This state persisted through**: a
`LightingService` restart, AND a full USB composite-device disable/
re-enable (not just the single `MI_01` collection -- the deeper reset
from Windows session 7). Neither cleared it. Recommended and the user is
doing a full reboot, which should clear it since nothing sent this
entire investigation, on either OS, has ever been a persistent/flash
write -- every command found in every capture has been a volatile
SET_REPORT/Feature write.

**Not yet understood, flagged for whoever picks this up next**: was the
stuck state caused by the `0x0201` API failure itself (Windows' HID
stack partially processing an invalid-function request in a way that
corrupts EC-side state), by something in the priming/streaming sequence
unrelated to the failed writes, or is "kbd3 green + specific lightbars
dark" actually a REAL, if incomplete, positive signal for `0x04`
finally partially working, that just happened to also leave the device
in a state normal software can't override? Genuinely unclear from this
one uncontrolled data point. If reproducible after reboot with a clean
methodology (capture running, static baseline instead of RainbowCycle,
one variable changed at a time), this could be either a real lead or a
red herring -- needs controlled reproduction before drawing conclusions
either way. Capture from the recovery attempt (full composite disable/
enable, which did NOT clear the stuck state) is saved as
`usb_capture_session6/pcap3_full_composite_disable_enable.pcapng` for
whoever wants to check whether it shows anything unusual compared to
the session 7 single-collection capture.

## Windows session 9 (2026-07-26) -- real sleep/resume capture, answering Linux's exact question with new protocol surface

User did this one entirely themselves: started a USBPcap capture, closed
the lid, let Windows actually go to sleep, then opened it back up and
saved the result (`SLEEP.pcapng`, moved into this repo as
`usb_capture_session7/sleep_resume_capture.pcapng`). This is exactly what
Linux asked for in `QUESTIONS.md` ("does Windows do anything lightbar-
specific on a genuine sleep-to-RAM/resume cycle") -- a real suspend, not a
Device Manager disable/enable.

**Timeline**: continuous `0x0305` `SET_REPORT Feature ReportID=5`
RainbowCycle stream from t=0 to t=9.88s (normal pre-sleep baseline), then
a real ~25.7s gap (t=9.88s to t=35.56s) where the capture process itself
was suspended along with the rest of the system, then traffic resumes.

**On resume, real, substantial handshake traffic -- much richer than
anything captured before this session**, decoded in exact order:
```
09 01 02 00 00 02 00 01 03            (ReportID=1 Output, data=01 03 -- NOT 01 01!)
09 5a 03 00 00 40 00 5a ba c5 c4 00   (brightness restore, brightness=0)
09 5a 03 00 00 40 00 5a ba c5 c4 03   (brightness restore, brightness=3)
09 5d 03 00 00 40 00 5d "ASUS Tech.Inc."   (real 0x5d handshake)
09 5d 03 00 00 40 00 5d 05 20 31 00 20 00...   (status)
09 5d 03 00 00 40 00 5d c0 00 01 00...   (NEW subcommand 0xc0, never seen before)
09 5d 03 00 00 40 00 5d d1 01 00 02 00...   (NEW subcommand 0xd1, sent TWICE)
09 5a 03 00 00 40 00 5a d0 4e 01...   (the 0x5a query, matches earlier captures)
09 5d 03 00 00 40 00 5d c0 00 01 00...   (0xc0 again)
09 5d 03 00 00 40 00 5d 9e 01 20 00...   (NEW subcommand 0x9e, never seen before)
09 5d 02 00 00 40 00 5d b3 00 02 00 00 00 eb 00...   (the familiar priming triplet)
09 5d 02 00 00 40 00 5d b4 00...
09 5d 02 00 00 40 00 5d b5 00...
```
This entire block (from the `0x5d` handshake through the priming triplet)
repeats a second time about 1.5s later in the same capture, byte-for-byte
identical in structure. Confirmed via `usb.data_len` distinct-value check
across the whole capture: only sizes 8/10/18/72 appear -- **no `0x04`
(59-byte) lightbar write anywhere**, and no `0x00`/9-byte report-0 write
this time either (that one, from Windows session 8, may be specific to
whatever triggered it there, not a universal resume step).

**Two genuinely new things here, neither ever seen in this entire
investigation across either OS**:
1. **Three undocumented `0x5d` subcommands**: `0xc0` (data `00 01`),
   `0xd1` (data `01 00 02`, sent twice), `0x9e` (data `01 20`). All real,
   all sent by Armoury Crate's own driver stack on genuine resume, none
   previously captured in any Device Manager disable/enable, kernel
   reprobe, or service restart this whole investigation.
2. **The `ReportID=1` Output write's second data byte is NOT constant.**
   Every prior sighting (Windows session 3's original priming script,
   `g615lr-alpha-ramp.rs`, Windows session 8's diff of the `25/` Aura
   Creator captures) showed `01 01`. Here, on resume specifically, it's
   `01 03`. This strongly suggests that second byte is a real state/mode
   value (e.g. "resuming" vs some other context), not a fixed wake
   signal -- worth treating as a variable, not a constant, in any future
   test.

**Not yet tested against hardware, real candidate for whoever picks this
up**: none of `0x5d c0`/`0x5d d1`/`0x5d 9e` have ever been sent
deliberately in isolation before a `0x04` write. Given the `0x5d bc`
custom-mode family turned out to be a real, working, previously-
undiscovered protocol this same investigation (Windows/Linux session 6's
BREAKTHROUGH), these three are genuine, evidence-backed candidates worth
the same treatment -- try each one (and the `01 03` variant of the
ReportID=1 write) immediately before a `0x04` zone write, on a clean
baseline. Unlike the report-6 guess or the earlier report-1 timing lead
(both already tried and failed), these three bytes have literally never
been sent by anyone testing this protocol before tonight.

Capture: `usb_capture_session7/sleep_resume_capture.pcapng`. Answers
Linux's `QUESTIONS.md` ask directly -- see that file for the relayed
answer.

## Windows session 9, continued -- Windows' own "Dynamic Lighting" checked, real negative

User's own idea, tested immediately after the sleep/resume capture: does
Windows 11's built-in "Dynamic Lighting" feature (Settings > Personalization
> Dynamic Lighting) use Microsoft's own standardized, PUBLICLY-documented
HID LampArray protocol to drive this device, bypassing ASUS's proprietary
`0x04`/`0x0305` protocol entirely? If so, that would have been a
completely different and far better-documented path forward than
reverse-engineering ASUS's own protocol. Genuinely worth checking --
interface 1's vendor collection uses HID Usage Page `0x59`, which
happens to be the SAME Usage Page number the real USB HID spec assigns
to "Lighting And Illumination" (the LampArray page), a detail Linux's
report-descriptor parse in session 6 had flagged as "non-standard/
vendor" without checking whether it might actually be the real assigned
page.

**Test**: captured live (`usb_capture_session7/dynamic_lighting_capture.pcapng`,
1208 packets) while manually: starting the capture with Armoury Crate's
RainbowCycle already running -> switching Windows Settings' device
priority so "Dynamic Lighting" took control instead of Armoury Crate
(chassis went static) -> changing the Dynamic Lighting mode to Breathing
-> changing brightness 2-4 times -> switching priority back to Armoury
Crate.

**Result: real negative on the interesting hypothesis.** Every single
write in the entire capture is still the exact same already-known
`SET_REPORT Feature ReportID=5` (`0x0305`) structure
(`05 01 00 00 0f 00 [alpha] [byte] [byte]`) -- confirmed by checking
`usb.data_len` across the whole capture: only sizes 8/18/72 appear, same
as every other capture this investigation, no new report ID, no
distinctive LampArray-style report structure (real LampArray devices use
dedicated `LampArrayAttributesReport`/`LampMultiUpdateReport`-style
reports, nothing like this). Extracted the byte7 (alpha/phase) values
across the whole capture and the pattern tells the real story:
- t=0-4.2s: smooth continuous hue rotation (RainbowCycle, matches the
  pre-switch baseline).
- t=13.2-13.3s and t=21.3-26.3s: several consecutive `00 00 00` frames --
  real black/off moments, matching the two priority-switch instants the
  user described ("it was static when dynamic took control").
- t=13.7-21s: values hovering near a fixed hue with the middle byte
  oscillating (`ff` down to `80` and back) -- consistent with a real
  static colour, then Breathing's brightness pulse on that fixed hue,
  matching exactly what the user did in this window.
- t=26.7s onward: smooth hue rotation resumes -- matches switching
  priority back to Armoury Crate/RainbowCycle at the end.
- Only two `0x5d` handshake-sized (72-byte) writes in the ENTIRE capture,
  at t=5.5s and t=27.6s -- lining up with the two priority-switch
  moments, each triggering the same already-known mode-restore handshake
  before `0x0305` streaming resumes. Nothing new in their structure.
- **No `0x04` write anywhere in this capture either**, even while
  actively switching between Windows' native lighting control and
  Armoury Crate.

**Conclusion**: Windows' Dynamic Lighting feature does NOT talk to this
device over a separate, standardized LampArray wire protocol. Whatever
"Dynamic Lighting support" ASUS advertises for this laptop is implemented
as a translation layer inside `LightingService` itself -- Windows' native
lighting API calls get converted into the exact same proprietary
`0x0305` stream Armoury Crate already uses, not a parallel Microsoft-
documented protocol. This closes out a real, worth-checking alternative
path (better to know for certain than to wonder), but it doesn't open a
new one -- `LightingService` remains the single point of control
regardless of which UI nominally has "priority," and the underlying
mystery (`0x04`'s missing prerequisite) is unaffected by any of this.

Capture: `usb_capture_session7/dynamic_lighting_capture.pcapng`.

## Windows session 9, continued again -- a real, wire-verified 0x04 write, first one in any capture tonight

User's third self-directed capture: static blue in Armoury Crate, then
switched device control to "Aura" (the standalone lighting app/service,
not Armoury Crate), which was configured to drive lightbar zones only
(12 regions), no keyboard, as a colour cycle. Saved as
`usb_capture_session7/static_armory_to_aura_lightbar_only.pcapng`
(344 packets).

**Real finding**: exactly ONE `SET_REPORT Feature ReportID=4` (`0x04`)
write in the entire capture, at frame 85 (t=9.593s), landing right in
the middle of a burst of `0x5d` handshake traffic (three 72-byte writes
just before it) and immediately followed by continuous `0x0305`
(`ReportID=5`) streaming for the rest of the capture (t=9.66s onward,
every ~60ms, matching the "colour cycle" the user described).

Full 51-byte payload, byte-accurate (re-verified twice after two manual
transcription slips -- see below):
```
04 05 01 00 00 01 00 02 00 03 00 04 00  00 00 00 00 00 00 00 00 00
   ^id ^cnt=5  ^^^^^ zone ID list (5 x u16 LE) ^^^^^^  ^^ 9-byte pad ^^
01 00 00 00  01 00 00 00  01 00 00 00  01 61 ff 00  ff 00 00 00 00 00 00 00 00 00
^^^^ 4-byte block, zone0 (kbd1) ^^^^  ^^^^ zone1 (kbd2) ^^^^  ^^^^ zone2 (kbd3) ^^^^  ^^^^ zone3 (kbd4) ^^^^  ^^^^ zone4 (back_right, lightbar) ^^^^
```
Zone ID list confirms the 5 zones addressed: `kbd1, kbd2, kbd3, kbd4,
back_right` (wire IDs `0x00,0x01,0x02,0x03,0x04` -- `back_right` is a
real lightbar zone per the corrected zone map). Then a 20-byte block
(5 x 4 bytes, same zone order) with the colour value `61 ff 00`
appearing inside the 4th block.

**Honest, explicit uncertainty -- do not treat this as a confirmed
decode**: every existing reference table in this repo
(`multizone_12x_confirmed.pcapng`'s byte table, the `matches_human_
confirmed_capture` unit test) is for `count=1` (one zone per packet)
writes, where the RGBA fields sit at a fixed offset relative to packet
start. This is a `count=5` packet, and the per-zone field order within
each 4-byte block (is it `[alpha,R,G,B]`? `[R,G,B,alpha]`? something
else?) does NOT resolve cleanly against either interpretation tried by
hand this session -- neither made "keyboard zones off, lightbar zone lit
with colour cycling" fall out consistently. **Do not guess further by
eye** -- this needs either a script that walks the 20-byte block
systematically against several more real multi-zone examples (the
`25/test123.pcapng`/`25/123.pcapng` captures analyzed earlier this
session have 309 and 241 real `0x04` writes respectively, almost
certainly including more `count>1` examples worth cross-referencing),
or asking Linux/a fresh session to write a small script rather than
hand-transcribing hex again -- manual transcription from raw hex dumps
failed twice in a row this session (dropped 2 bytes both times) before
being caught by cross-checking against the `wLength` field.

**What's solid regardless of the byte-order ambiguity**: this confirms,
with a real wire capture, that a genuine "lightbar-only, colour-cycling,
keyboard off" working state on this hardware is achieved via ONE `0x04`
write (establishing which zones are addressable/active) followed by
continuous `0x0305` streaming (the already-fully-characterized
host-rendered animation mechanism) -- not a new protocol, not a
different mechanism than already understood, just the first time either
side has captured the `0x04`-then-`0x0305` handoff moment itself in a
real working session with a live human-confirmed visual result attached.

Capture: `usb_capture_session7/static_armory_to_aura_lightbar_only.pcapng`.

## Windows session 9, continued a third time -- multi-zone `0x04` byte layout, empirically decoded and confirmed

Per direct instruction, replaced hand-transcribing hex (which had already
failed twice this session) with an actual script. Method: real Aura
Creator captures stream each active "layer" repeatedly while animating
it (already established -- alpha/colour ramps smoothly frame to frame),
so grouping consecutive real `0x04` writes by their exact zone-ID list
and measuring per-byte-position smoothness (small frame-to-frame deltas
= real animated channel; zero variance = structural/constant; large
random deltas = misread byte) turns "guess the layout" into "measure
it directly from hundreds of real examples."

Pulled every real `0x04` write from `25/test123.pcapng` (277 writes),
`25/123.pcapng` (188 writes), and this session's
`static_armory_to_aura_lightbar_only.pcapng` (1 write) -- 466 total after
filtering to writes with a parseable zone list. Zone counts seen:
2, 4, 5, 6, 8 (never above 8). Found 11 batches of >=8 consecutive
same-zone-list writes; ran the smoothness analysis on the 3 largest.

**Confirmed structure** (byte-accurate, `data[]` relative to the report
ID byte, i.e. `data[0] = 0x04`):
```
data[0]        report ID (0x04)
data[1]        zone count N (observed range 1-8)
data[2]        flag, always 0x01 in every sample
data[3:19]     zone-ID list, N x u16 LE, zero-padded -- a FIXED 16-byte
               region (room for exactly 8 zones) regardless of actual N
data[19:19+4N] N x RGBA (R, G, B, Alpha -- 4 bytes each), SAME ORDER as
               the zone-ID list, ALWAYS starting at offset 19 no matter
               how small N is (confirmed identical start offset in
               count=4 AND count=8 batches -- the zone-ID region isn't
               resized, just under-filled and zero-padded)
data[19+4N:]   unused, zero
```
This is a real generalization of the already-confirmed `count=1` layout
(zone ID at `data[3:5]`, RGBA at `data[19:23]`) -- turns out `count=1` was
just the `N=1` case of this same fixed-offset scheme all along, not a
special case.

**Applied to `static_armory_to_aura_lightbar_only.pcapng`'s one real
`0x04` write** (the "just lightbar, no keyboard" capture): zone list
`kbd1, kbd2, kbd3, kbd4, back_right`. Decoded RGBA blocks:
- kbd1: `00 00 00 01` -- R,G,B=0, alpha=1 (~0, effectively invisible)
- kbd2: `00 00 00 01` -- same
- kbd3: `00 00 00 01` -- same
- kbd4: `00 00 00 01` -- same
- back_right (lightbar): `61 ff 00 ff` -- R=0x61,G=0xff,B=0x00 (a real
  bright yellow-green), alpha=**0xff (full)**

This lines up exactly with what the user watched happen live: all four
keyboard zones effectively off (near-zero alpha), the lightbar zone lit
with a real colour at full alpha. **Alpha is a real per-zone visibility
gate in this protocol, not just a brightness dimmer** -- confirms multiple
zones can be legitimately "addressed but invisible" in the same packet
that lights another zone, which is exactly the "12 lightbar zones lit,
keyboard untouched" scenario the user described.

**Real caveat, stated plainly**: this decodes the packet FORMAT
correctly and confirms it's internally consistent across 4 independent
real sources -- it does NOT explain why sending byte-identical packets
from Linux/our own Windows raw tests produces zero visible effect. The
core mystery is unchanged.

**Audit done immediately, same session**: checked whether any existing
`count=1` reproduction script had a latent alpha-byte bug (sending
alpha=0 by mistake for a zone it means to light would produce exactly
the "zero visible effect" symptom this whole investigation keeps
hitting, for a completely mundane reason). Checked
`g615lr-alpha-ramp.rs`, `g615lr-kbd1-static-no-priming.rs`, and
`g615lr-corner-no-priming.rs` directly -- **clean, no bug found**. All
three place R/G/B/A at the confirmed-correct offsets 19-22;
`kbd1-static-no-priming.rs` uses a constant `alpha=0xff`, the other two
ramp alpha but both peak at `0xff`. So this newly-confirmed layout
doesn't reveal a bug in prior reproduction attempts, it just confirms
packet construction was already right in every test that mattered --
consistent with (not contradicting) the wire-verification already done
in Linux session 6.

What this DOES give, genuinely new: a corrected, fully empirically-
verified reference for `count>1` packets (previously only `count=1` was
confirmed), useful if either side wants to construct a real multi-zone
reproduction test (nobody has tried a `count>1` packet from Linux or a
raw Windows test yet -- every test all investigation has been
`count=1`, one zone at a time). Given the one real working example we
have (`static_armory_to_aura_lightbar_only.pcapng`) used `count=5`, a
genuine multi-zone test -- replicating that exact packet byte-for-byte,
rather than another single-zone attempt -- is a real, untried variable
worth adding to the list.

Analysis script: `C:\Users\Krushna\re\decode_multizone.py` (local only,
not committed -- pure analysis tool, references temp hex dumps outside
the repo; rewrite against `25/`'s files directly if this needs to be
reproduced in a future session).

## BREAKTHROUGH (2026-07-26, Windows session 9, immediately after the decode above): first-ever successful `0x04` reproduction from our own raw HID code

**This is the actual answer.** Built
`usb_capture_session7/test_count5_multizone.ps1`: real `0x5d b3/b4/b5`
priming (matching what real Aura sent before its own working write),
then the EXACT byte-for-byte `count=5` packet decoded above (zone list
`kbd1, kbd2, kbd3, kbd4, back_right`; keyboard zones at `R,G,B,A =
0,0,0,1`; `back_right` at `R,G,B,A = 0x61,0xff,0x00,0xff`), sent via
`HidD_SetFeature` through `HidSend.cs` -- the same raw HID mechanism
every other test all investigation has used -- streamed continuously
(since we don't have Aura's own `0x0305` stream to hold the state).

**Result, live human-confirmed, twice**: **the back-right lightbar zone
lit up yellow-green, keyboard stayed off/unchanged.** First run the user
asked for a repeat (didn't catch it in time); second run, 20s, explicit
live confirmation: "Lightbar zone lit yellow-green, keyboard stayed
off." Wire-verified via a parallel `USBPcap3` capture
(`usb_capture_session7/pcap3_count5_multizone_test.pcapng`) -- the
captured packet on the wire is **byte-for-byte identical** to both the
intended packet and to Aura's own real working write from
`static_armory_to_aura_lightbar_only.pcapng`:
```
09 04 03 01 00 33 00 04 05 01 00 00 01 00 02 00 03 00 04 00 00 00 00 00
00 00 00 00 00 01 00 00 00 01 00 00 00 01 00 00 00 01 61 ff 00 ff 00 00
00 00 00 00 00 00 00 00
```
This is, after dozens of failed hypotheses across every session this
entire investigation on both Windows and Linux, the first time `0x04`
has done ANYTHING visible when sent from code we wrote ourselves,
rather than by Aura's own real software.

**What actually changed, and genuine uncertainty about which part of it
mattered**: every single `0x04` test before this one -- across this
entire investigation, both OSes -- used `count=1` (one zone per
packet). This is the first `count>1` test ever run. It succeeded. But
it's ALSO the first time zone `0x04` (`back_right`) specifically was
targeted at all -- every prior single-zone test used a different zone
(`0x00` kbd1, `0x02` kbd3, `0x06` back_corner_right, `0x0D`
CornerFrontLeft). So there are two live hypotheses, not cleanly
separated by this one test:
1. **`count>1` (a real multi-zone batch) is the missing prerequisite** --
   maybe the firmware only renders `0x04` writes that address multiple
   zones together, or specifically requires keyboard zones to be
   present (even at alpha~0) alongside a lightbar zone in the same
   packet.
2. **Zone `0x04`/`back_right` specifically behaves differently** from
   every other zone tried so far, for reasons unrelated to `count`.

**The immediate, obvious, single-variable follow-up test, not yet run**:
send a `count=1` packet targeting ONLY zone `0x04` (`back_right`) alone,
same priming, same colour, same alpha=0xff. If that also lights the
lightbar, hypothesis 2 is confirmed and `count>1` never mattered --
every prior test just happened to pick zones that don't work for some
other reason. If it produces the usual zero effect, hypothesis 1 is
confirmed -- `count>1` (or specifically "keyboard zones present in the
same packet") is the real missing piece, and this reframes the entire
remaining investigation on both OSes: every existing Linux test
(`g615lr-alpha-ramp.rs`, `g615lr-corner-no-priming.rs`, etc.) would need
to be rerun as genuine multi-zone batches instead of single-zone writes.

This is the single most important thing for whoever picks this up next
to do first, on whichever OS is more convenient.

## NOT resolved -- isolation test contradicted itself on rerun, real open question

First isolation run (`usb_capture_session7/
test_count1_backright_isolated.ps1`, `count=1`, zone `back_right` alone,
identical priming/colour to the working `count=5` test, 15s/341 packets,
all `HidD_SetFeature` calls succeeded -- clean transport): **zero visible
effect**, live-confirmed. Looked like a clean, definitive answer
(`count=1` never works, `count>1` is the real prerequisite) and got
written up as "RESOLVED" in this file for a few minutes.

**Then the user asked for a rerun before accepting that conclusion --
good call.** Same script, same packet, run again immediately after (18s,
`usb_capture_session7/pcap3_count1_backright_isolated_run2.pcapng`):
**lit up, same as the working multi-zone test.** Flatly contradicts the
first run. Nothing about the script or packet changed between runs.

**Real, live, unresolved question**: does `count=1` genuinely never work
(and the second run's success was caused by some carried-over device/EC
state from the earlier successful `count=5` write and/or the first
`count=1` run's own 341 packets -- i.e. zone `0x04` got "activated" once
by something in this boot session and stayed that way), or does
`count=1` actually work sometimes and the FIRST run's negative result
was the anomaly (e.g. needed more repetitions/time to register than 15s
gave it)? These two explanations make opposite predictions and can only
be told apart by a genuinely clean test.

**The test that would actually resolve this, not yet run**: full
reboot (clean EC/session state, no prior exposure to zone `0x04` at all
this boot), then send a `count=1` write to `back_right` FIRST, before
anything else touches that zone -- no prior `count=5` write, no prior
`count=1` attempt in the same session. If it lights on the very first
try: `count>1` was never the real variable, something else explains
every previous negative result. If it stays dark: strengthens the
`count>1`-requirement theory, though still wouldn't fully rule out a
"needs N repetitions to register" explanation without more controlled
runs.

**What's still solid regardless of this open question**: the `count=5`
multi-zone packet structure decoded earlier this session is real and
wire-verified correct (byte-for-byte identical to Aura's own working
capture), and at least one real, live-confirmed lighting of the lightbar
DID happen from our own code tonight -- that part isn't in question, only
WHY it happened (count, carried-over state, or something else) is still
open. Whoever picks this up next: run the clean-reboot test above
before trusting either the "count>1 required" or "count doesn't matter"
framing -- neither is confirmed yet.

Captures: `usb_capture_session7/pcap3_count1_backright_isolated.pcapng`
(run 1, negative), `usb_capture_session7/
pcap3_count1_backright_isolated_run2.pcapng` (run 2, positive -- same
script). Working capture from the breakthrough:
`usb_capture_session7/pcap3_count5_multizone_test.pcapng`.
