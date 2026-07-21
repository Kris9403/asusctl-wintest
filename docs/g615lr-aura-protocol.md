# ROG Strix G16 (2025) / G615LR Aura Lighting — Reverse-Engineered Protocol

This laptop's RGB lighting is **not** documented anywhere (not in `asusctl`,
not in the Linux `asus-wmi` driver, not anywhere on asus-linux.org as of this
writing). Armoury Crate talks to it through an undocumented USB HID
interface. This document records what was found by direct USB capture and
live hardware testing (Windows, since no Linux hardware was available during
this investigation — everything here needs a `hidraw` confirmation pass on
Linux before being fully trusted).

## Device identity

- USB VID:PID = `0B05:19B6` (ASUS "N-KEY Device" — shared across most recent
  ROG laptops, so PID alone does not identify this protocol; existing
  `aura_support.ron` entries using this PID for other boards are unaffected).
- The device exposes **two separate top-level USB interfaces** (`MI_00` and
  `MI_01`), each with multiple HID collections. The interface that matters
  depends on which report ID you're using (see below) — on Linux this means
  probing every `/dev/hidrawN` with matching `idProduct` rather than
  assuming the first match is correct (`rog_platform::hid_raw::HidRaw::new`
  currently takes the *first* match unconditionally — see "Impact on
  asusctl" below).

## Two independent lighting protocols exist on this hardware

### 1. Per-zone static color (Report ID `0x04`) — NEW, not in any existing tool

This is the protocol Armoury Crate's simple "Aura Sync" zone painter and
"Aura Creator" custom layouts actually use. It does **not** appear anywhere
in `asusctl`/`rog-aura` prior to this patch — the existing `aura_support.ron`
entry for `G615LR` only wires up protocol #2 below (legacy modes), which is
why community zone/color mappings for this laptop never worked: it's the
right protocol for the wrong feature.

Rust implementation: [`rog-aura::lightbar_2025`](../rog-aura/src/lightbar_2025.rs).

- **Interface**: found via `HidD_GetAttributes`/enumeration on `MI_01` on the
  test machine — confirm per-machine, do not hardcode an interface index.
- **Transport**: HID **Feature** report (`HidD_SetFeature` on Windows;
  `HIDIOCSFEATURE` ioctl on Linux, i.e. **not** a plain `write()` to
  `/dev/hidrawN` — `write()` sends an Output report, which is the wrong
  report type for this one).
- **Report**: `bmRequestType=0x21, bRequest=0x09 (SET_REPORT), wValue=0x0304
  (Feature, ReportID 4), wIndex=<interface>, wLength=0x0033 (51 bytes)`.

**Payload (51 bytes)**:

| Offset | Meaning |
|---|---|
| 0 | Report ID echo (`0x04`) |
| 1 | Zone count N (1-8 zones per packet) |
| 2 | Flag byte — always `0x01` worked; exact meaning (batch type?) unconfirmed |
| 3-18 | Up to 8 zone-ID slots, 2 bytes each (little-endian). Unused slots zero. |
| 19-50 | Up to 8 color slots, 4 bytes each: `[byte0, byte1, byte2, 0xFF]`. Unused slots zero. |

A packet only affects the zones listed in it — zones not mentioned keep
their previous state (this is a hardware capability; the `rog-aura`
implementation always sends every zone explicitly rather than relying on
this, so "off" behaves deterministically).

**Zone IDs** (0x00-0x0F, 16 total — 4 keyboard + 12 lightbar segments
wrapping the chassis). See `Lightbar2025Zone` in the Rust module for the
canonical list; summarized:

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

**Known unresolved issue — color channel order is inconsistent per zone**,
and got contradictory results across separate test sessions on the same
hardware for some zones (specifically the left/back sidebar and back-corner
zones). What's solid:

- Colors within a single zone's 4-byte slot are consistently either plain
  `[R, G, B, 0xFF]` or a `[G, R, B, 0xFF]` swap — never anything more exotic.
- Which zones need the swap was **not reproducible session-to-session** for
  at least two of the zones — isolated single-zone tests gave one answer,
  and later re-tests (sometimes via a different Windows HID API call,
  sometimes after other zones had been toggled) gave the opposite answer for
  the same zone/color/logic.
- Leading hypothesis: Armoury Crate's background service was still running
  during testing and periodically reasserts its own state on this same
  interface, racing with the test writes. **Never conclusively ruled out.**
  Anyone continuing this work should: kill Armoury Crate's services first,
  then retest each zone in full isolation (only that zone lit, pure Red or
  Green — never Blue/Yellow/White, which can't reveal a channel swap) before
  trusting `Lightbar2025Zone::needs_grb_swap()`.
- `needs_grb_swap()` in the Rust module reflects the last self-consistent
  result obtained, **not a confirmed final answer.**

### 2. Legacy built-in effects (Report ID `0x5d`) — the protocol `asusctl` already implements elsewhere

This is the same protocol `rog-aura` already implements for other ROG
laptops (`AURA_LAPTOP_LED_SET`/`_APPLY`, `AuraModeNum`, in
[`rog-aura::usb`](../rog-aura/src/usb.rs) and
[`rog-aura::builtin_modes`](../rog-aura/src/builtin_modes.rs)) — it's what
drives Armoury Crate's hardware Static/Breathing/Rainbow/etc. mode picker on
other boards, and it's what the current `G615LR` entry in
`aura_support.ron` wires up.

- **Interface**: found on `MI_00`'s `COL04` collection on the test machine —
  a *different* interface than protocol #1 above.
- **Transport**: also a HID **Feature** report on this laptop specifically
  (confirmed by trying both) — despite `rog_platform::hid_raw::HidRaw`
  sending it as a plain file `write()` (Output-report semantics) on every
  other laptop this crate supports. This laptop has no interrupt-OUT
  endpoint at all (`HidD_SetOutputReport` returns `ERROR_INVALID_FUNCTION`
  on Windows), so the existing write-based approach may silently no-op here
  even at the transport level, independent of the finding below.
- **Report**: `wValue=0x025D`, 64-byte payload, same structure as
  `AuraEffect`'s existing byte layout in `builtin_modes.rs`.

**Conclusively tested and does NOT produce any visible effect on this
hardware**, tried as both Feature and Output report semantics, both
`RainbowCycle` and `Static` (bright magenta — impossible to miss if it had
worked). A successful `HidD_SetFeature`/`HidD_SetOutputReport`/`write()` call
returning success is **not proof the device acted on it.** The periodic
`0x5d` traffic observed from Armoury Crate in USB captures is most likely a
vestigial code path from a shared multi-model Windows driver codebase that
doesn't actually do anything on this specific board.

**Practical conclusion: all chassis/keyboard lighting control on this laptop
goes through protocol #1 (report `0x04`) only.** No built-in hardware
animation (Breathing/Rainbow/etc.) appears to exist for this laptop at the
protocol level — animated effects need to be driven from software instead,
by repeatedly resending `0x04` static-color packets with a computed color
per frame (the same technique `rog-aura::effects::Breathe` already uses for
per-key software breathing on other laptops — see
[`rog-aura::effects::breathe`](../rog-aura/src/effects/breathe.rs) for the
existing triangle-wave color math this can reuse).

## Impact on `asusctl`

1. `rog_platform::hid_raw::HidRaw::new` (in `rog-platform/src/hid_raw.rs`)
   currently grabs the *first* `/dev/hidrawN` matching `idProduct`,
   unconditionally, and always does a plain `write()` (Output report). For
   this laptop that's the wrong report type for protocol #1 and possibly a
   silent no-op for protocol #2 (no interrupt-OUT endpoint exists on this
   board). Supporting this laptop correctly needs either a new code path
   that opens by report-ID/interface capability rather than first-match, or
   a Feature-report-capable write method alongside the existing
   `write_bytes`.
2. `rog-aura`'s `aura_support.ron` entry for `G615LR` should be updated to
   reflect that it needs protocol #1 for actual color control — the current
   entry (basic zones, `advanced_type: r#None`) implies the classic
   USB-HID zoned protocol works for this board's lightbar, which this
   investigation shows is false.
3. `Lightbar2025Zone`/`build_lightbar_2025_packet` in
   [`rog-aura::lightbar_2025`](../rog-aura/src/lightbar_2025.rs) are ready
   to use as the packet-building layer; they are **not yet wired into**
   `asusd`'s dispatch logic (`asusd/src/aura_laptop/mod.rs`) or exposed over
   D-Bus — that requires a maintainer with real Linux hardware to validate
   the swap table and interface-selection logic (item 1) before it's safe to
   ship, since the biggest open question (per-zone color swap) could not be
   fully resolved in this investigation.
