# ROG Strix G16 (2025) / G615LR Aura Lighting — Reverse-Engineered Protocol

Full session findings, consolidated before context compaction. This is the
authoritative summary — if anything below conflicts with an earlier message
in the conversation, trust this file.

## Device identity

- USB VID:PID = `0B05:19B6` (ASUS "N-KEY Device" — shared across most recent
  ROG laptops, so PID alone does not identify this protocol).
- Two separate top-level USB interfaces (`MI_00` and `MI_01`), each with
  multiple HID collections. Which one matters depends on the report ID:
  - Report `0x04` (the real color protocol) → found on `MI_01` on the test
    machine.
  - Report `0x5d` (legacy, confirmed non-functional here) → found on
    `MI_00`'s `COL04` collection.
  - Enumerate by VID/PID and try each path; exact path suffixes are not
    guaranteed stable across reboots.

## The real protocol: per-zone static color, Report ID `0x04`

This is what Armoury Crate's zone painter, Aura Creator, and (as of this
session's later findings) literally every single one of its 12 "hardware"
effect modes actually use — see "No firmware effects exist" below.

- **Transport**: HID **Feature** report. `HidD_SetFeature` on Windows;
  `HIDIOCSFEATURE` ioctl on Linux `hidraw` (NOT a plain `write()` —
  `write()` sends an Output report, the wrong type for this one).
- **Setup packet**: `bmRequestType=0x21, bRequest=0x09 (SET_REPORT),
  wValue=0x0304 (Feature, ReportID 4), wIndex=<interface>, wLength=0x0033
  (51 bytes)`.
- **Payload (51 bytes)**:

| Offset | Meaning |
|---|---|
| 0 | Report ID echo (`0x04`) |
| 1 | Zone count N (1-8 zones per packet) |
| 2 | Flag byte — `0x01` used throughout, exact meaning unconfirmed |
| 3-18 | Up to 8 zone-ID slots, 2 bytes each (little-endian). Unused slots zero. |
| 19-50 | Up to 8 color slots, 4 bytes each: `[byte0, byte1, byte2, 0xFF]`. Unused slots zero. |

A packet only touches the zones it lists — others keep their prior state
(hardware behavior; our scripts always resend every zone explicitly so "off"
stays deterministic).

**Sending**: one zone per packet was adopted as the safe default after
seeing inconsistent results when batching multiple zones in one packet
during testing — whether that was genuine hardware cross-talk or an
artifact of the swap-table bugs active at the time was never conclusively
separated. One-zone-per-packet is confirmed reliable; batching was not
re-tested after the swap table was corrected, so it may actually be fine —
untested, not disproven.

## Zone ID table (16 zones: 4 keyboard + 12 lightbar)

Physical positions confirmed by single-zone isolation testing (only the
named zone lit, everything else forced off, non-invariant color used).

| ID | Zone | ID | Zone |
|---|---|---|---|
| 0x00-0x03 | Keyboard 1-4 (left→right) | 0x08 | Left side, back half |
| 0x04 | Back bar, left | 0x09 | Left side, front half |
| 0x05 | Back bar, right | 0x0A | Right side, front half |
| 0x06 | Corner, back-left | 0x0B | Right side, back half |
| 0x07 | Corner, back-right | 0x0C | Corner, front-right |
| | | 0x0D | Corner, front-left |
| | | 0x0E | Front bar, right |
| | | 0x0F | Front bar, left |

Note the wire IDs do not follow any obvious spatial pattern (0x08/0x09 are
"left", 0x0A/0x0B are "right", but 0x0E/0x0F are reversed vs 0x06/0x07) —
this hardware is genuinely cross-wired, not a labeling convention issue.

## Color channel order — the messy part

Each zone's 4-byte color slot is either plain `[R, G, B, 0xFF]` or a
`[G, R, B, 0xFF]` swap (alpha/enable byte always `0xFF`). **Current
best-known list of zones that do NOT swap** (i.e. use plain RGB):

```
0x00, 0x01, 0x02, 0x03,   # keyboard, all 4
0x04, 0x05, 0x06, 0x07,   # back bar + back corners
0x08,                     # left side, back half
0x0A,                     # right side, front half
0x0C, 0x0D,               # front corners
0x0E, 0x0F                # front bar
```

Only `0x09` (left side, front half) and `0x0B` (right side, back half) are
currently believed to need the swap.

**This flip-flopped during the session** — `0x04`-`0x07` tested as
needing the swap earlier (confirmed with pure Red/Green, a non-invariant
test), then later tested as NOT needing it (confirmed by getting correct
Saffron `#FF9933`, also non-invariant). Both results used real isolated
tests with revealing colors; they can't both be right for genuinely
unchanging hardware. **Leading suspect: Armoury Crate's background services
were running the entire session and were never successfully killed** —
`ArmouryCrate.Service`, `ArmourySwAgent`, `LightingService`, `ROGLiveService`,
`ArmouryCrateControlInterface`, `AsusSoftwareManager`, `GameSDK`, and others
were all still alive even after an elevated `Stop-Service -Force` attempt
(same PIDs before/after — the stop calls silently no-op'd). If these
periodically reassert their own state on the same USB interface, that would
explain inconsistent results without any hardware instability at all. A
faint residual symptom of this was still visible at the very end of the
session: `kbd1` showed a minor intermittent flicker even with a constant
color being resent every frame, while `kbd4` (same static color, same
packet logic) did not.

**Before trusting this table for anything real**: get these services fully
stopped (may need `sc.exe config <name> start= disabled` + reboot, since
`Stop-Service -Force` alone did not work even elevated), then redo full
single-zone isolation testing zone-by-zone with Red and Green specifically.
Do not use Blue/Yellow/White for this — they're mathematically invariant
under the swap and prove nothing.

## No firmware effects exist on this laptop

Tested exhaustively — Static and RainbowCycle via the legacy `0x5d`
protocol, both as HID Feature reports and as Output reports (the call
matching what `asusctl`'s real Linux implementation does via a plain
`write()`), all returned success at the Windows API level with **zero
visible effect** (confirmed with bright magenta Static — impossible to
miss if it had worked).

A genuine Wireshark capture of Armoury Crate itself switching through all
12 of its effect modes (Static, Breathing, Strobing, Color Cycle, Rainbow,
Starry Night, Music, Smart, Adaptive Color, Dark/Off, AI Aura Lighting, and
a custom "INDIA" profile) confirmed why: **every single `0x5d` packet in
that entire capture was byte-for-byte identical** (`5d b3 00 02 00 00 00
eb...` — zone=all, mode=RainbowCycle, color=black), completely independent
of which mode was actually selected. That's dead/vestigial traffic from a
shared multi-model Windows driver codebase, not real mode-switching.

What actually drives every mode is Armoury Crate continuously streaming
`0x04` packets from the PC, computing a new color per zone per frame:
- A long stretch of packets showed smoothly-shifting, continuously-varying
  colors across keyboard+backbar+corners — a moving color sweep (Rainbow
  or Color Cycle).
- A later stretch showed mostly-zero bytes with sparse, isolated bright
  flashes on random zones each frame — a twinkle/sparkle pattern (Starry
  Night).

**Conclusion**: this laptop generation has no onboard animation engine at
all. Armoury Crate's entire "hardware effects" menu — even the ones that
are firmware-native on other ROG laptops — is host-computed and streamed.
Likely an EC firmware cost-cut for this generation.

## Why `asusctl` couldn't have caught this

`asusctl`'s `AuraModeNum` (`Static, Breathe, RainbowCycle, RainbowWave,
Star, Rain, Highlight, Laser, Ripple, Pulse, Comet, Flash`) reflects
**firmware-native** animations on the laptops it was built against — send
one `0x5d` command, the embedded controller animates it autonomously
forever. That list doesn't include Music/Smart/Adaptive Color/AI Aura
Lighting on *any* laptop, ever, because those were always host-computed
(audio capture, sensor polling, wallpaper analysis). On this laptop, even
the "old" modes joined that category, since the firmware engine doesn't
exist here.

There IS a precedent in `asusctl`'s architecture for "no firmware effects,
drive it differently": `AuraDeviceType::LaptopKeyboardTuf` — TUF-series
laptops lack the N-Key hardware effects chip and get a separate code path
through Linux's `asus-wmi` sysfs interface instead of raw USB HID. Our
situation is the same *shape* of problem, just a different transport (raw
USB Feature reports instead of WMI/sysfs).

`asusctl` also has a small **software** effects engine
(`rog-aura::effects` — `Static`, `Breathe`, `DoomFlicker`,
`DoomLightFlash`, driven by an `EffectState` trait with
`next_colour_state()`/`get_colour()`, ticked and re-streamed by the daemon)
used for per-key laptops with no hardware mode for a given effect. It's a
much smaller palette than Armoury Crate's (no Rainbow, no Starry Night,
nothing audio-reactive) but it's the right architectural home for new
effects — see "Path to a real `asusctl` patch" below.

## Does this only help this one laptop?

Mostly scoped to `G615LR` (`aura_support.ron`'s `device_name` matching), but
two things extend it:
1. A sibling board, `G614FR`, was committed separately with the same
   "(ROG Strix G16 2025)" label — plausibly the same EC firmware generation,
   possibly the same `0x04` protocol. Worth testing if anyone has one.
2. That's why the protocol itself is documented here in full rather than
   just hardcoded into a config entry — a future contributor with a
   different 2025-gen board can test whether theirs also speaks report
   `0x04` and just add one `aura_support.ron` line if so, without redoing
   any of this reverse-engineering.

Patches are safe for other laptops by construction — `asusctl` dispatches
per-device by exact `device_name`/`AuraDeviceType` match, so code added for
`G615LR` never executes for any other board.

## Files in this session

- `HidSend.cs` — P/Invoke wrapper (`hid.dll`/`setupapi.dll`):
  `HidD_SetFeature`, `HidD_SetOutputReport`, VID/PID path enumeration,
  persistent-handle helpers (`OpenPersistent`/`SetFeatureOnHandle`) for
  smooth animation without reopening a handle every frame.
- `aura_core.ps1` — shared module, dot-sourced by all three scripts below:
  zone maps (`$INTERNAL_ZONES`, `$PHYSICAL_MAP`), the swap table
  (`$NO_SWAP_ZONES`), `Build-AuraPacket`, `Get-AuraDevicePath`,
  `ConvertFrom-HexColor`/`ConvertFrom-Hsv`, `Resolve-PhysicalZone`. Single
  source of truth — previously these were duplicated across all three
  scripts and had drifted out of sync (`aura_animate.ps1` was still running
  a stale swap table missing the back-bar correction until this refactor).
  Loads `HidSend.cs` relative to its own location, so the whole folder can
  be moved out of the temp scratchpad without editing any paths.
- `aura_control.ps1` — static per-zone control by physical name (`-List` to
  see zone names, `-Zone <name>[,...] -Color <hex>[,...]` to set).
- `aura_animate.ps1` — Rainbow / StarryNight / Breathe animation loops,
  confirmed working live on hardware.
- `aura_india.ps1` — India tricolor static layout + breathing blue
  "chakra" on `kbd2`/`kbd3`, confirmed working live (saffron back / white
  sidebars+outer keys / green front). Static zones are sent once at startup
  rather than every frame (fixes the `kbd1` flicker noted earlier).
- `test_5d_full.ps1` / `test_5d_output_full.ps1` — legacy-protocol test
  harnesses (both confirmed non-functional on this hardware).

## Path to a real `asusctl` patch (not yet done)

A clone with a starting point exists at
`...\scratchpad\asusctl` (committed as `9796e543`, "Document G615LR (ROG
Strix G16 2025) second Aura protocol"):
- `rog-aura/src/lightbar_2025.rs` — `Lightbar2025Zone` enum + packet
  builder, kept in sync with the swap table above (`needs_grb_swap()` is
  the single source of truth, matching `aura_core.ps1`'s `$NO_SWAP_ZONES`).
- `rog-platform/src/hid_raw.rs` — new `HidRaw::set_feature_report`, using
  the `HIDIOCSFEATURE` ioctl (via the `nix` crate) instead of the existing
  `write_bytes` (which sends an Output report — the wrong type for this
  protocol, exactly like the Windows `HidD_SetFeature` vs `HidD_SetOutputReport`
  distinction documented above). Both `HidRaw::new`/`from_device` now open
  the device `read(true).write(true)` instead of write-only, which the
  ioctl's readwrite direction requires.
- `asusd/src/aura_laptop/mod.rs` — new `Aura::write_lightbar_2025` method
  wiring `build_lightbar_2025_packet` to `HidRaw::set_feature_report`.
  **Callable but not called from anywhere yet** — `write_current_config_mode`
  has no G615LR-aware branch, there's no D-Bus method for it, and the
  existing `AuraEffect` config type only carries 1-2 colours, not the 16
  independent per-zone colours this protocol needs. Real dispatch wiring
  needs a config/D-Bus shape change, not just a branch.
- `docs/g615lr-aura-protocol.md` — protocol writeup for the repo itself.
- `aura_support.ron` — comment added flagging the existing `G615LR` entry
  as non-functional for real chassis lighting.
- Not yet done: the dispatch/D-Bus wiring described above, and porting the
  Rainbow/StarryNight color math into `rog-aura::effects` as real
  `EffectState` implementations (currently only exists as the PowerShell
  scripts above). Right shape to follow: same
  `next_colour_state()`/`get_colour()` pattern as the existing `Breathe`
  effect, targeting `Lightbar2025Zone` instead of per-key `LedCode`.
- **None of the new Rust code (ioctl wrapper, `HidRaw` changes,
  `write_lightbar_2025`) has been compile-checked.** `cargo` wasn't
  reachable in this session's shell to even attempt it, and the workspace's
  Linux-only `udev` dependency means it can only be genuinely verified via
  WSL or a real Linux boot, not on Windows. Treat this as
  reviewed-by-inspection only until `cargo check -p rog_platform -p asusd`
  actually runs.
