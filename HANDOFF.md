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
