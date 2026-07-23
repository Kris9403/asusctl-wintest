//! Support for the "second" Aura protocol found on the 2025 ROG Strix G16
//! refresh (board name `G615LR`, USB `0b05:19b6`, same "N-KEY Device" PID
//! used by most modern ROG laptops -- PID alone does not distinguish this
//! protocol).
//!
//! This device does **not** use the classic `0x5d` static/breathe/rainbow
//! protocol (`AuraEffect`/`AURA_LAPTOP_LED_SET`/`_APPLY` in [`crate::usb`])
//! for its per-zone lightbar colors, even though it still periodically
//! echoes that protocol's traffic in the background (observed via USB
//! capture; this appears to be dead/vestigial code in Armoury Crate's
//! Windows driver stack -- sending it produces no visible effect on this
//! hardware, tested exhaustively as both a Feature report and an Output
//! report, with both `Static` and `RainbowCycle` modes).
//!
//! Instead, all 16 zones (4 keyboard segments + 12 lightbar segments
//! wrapping the chassis) are set through a single undocumented **HID
//! Feature report, Report ID `0x04`**, discovered and verified entirely by
//! USB traffic capture plus live hardware testing (no vendor documentation
//! exists for this). See `docs/g615lr-aura-protocol.md` in this repo for
//! the full writeup, or the original investigation notes.
//!
//! ```ignore
//! bmRequestType = 0x21 (host->device, class, interface)
//! bRequest      = 0x09 (SET_REPORT)
//! wValue        = 0x0304 (ReportType=Feature(3), ReportID=4)
//! wIndex        = interface number carrying this report (varies by OS
//!                 enumeration order -- must be located by probing, not
//!                 assumed to be a fixed interface index)
//! wLength       = 0x0033 (51 bytes)
//! ```
//!
//! On Linux this is sent via the `HIDIOCSFEATURE` ioctl, NOT a plain
//! `write()` (which sends an Output report, a different report type this
//! device does not act on for this report ID). See
//! `rog_platform::hid_raw::HidRaw::set_feature_report` -- the packet built
//! by [`build_lightbar_2025_packet`] below is exactly what that method
//! expects. This ioctl wrapper is implemented but not yet compile- or
//! hardware-verified on a live Linux boot -- see `docs/g615lr-aura-protocol.md`.
//!
//! Payload (51 bytes total):
//!
//! | Offset | Meaning |
//! |---|---|
//! | 0 | Report ID echo (`0x04`) |
//! | 1 | Zone count N, 1-8 |
//! | 2 | Flag byte -- `0x01` used throughout testing, exact meaning unconfirmed |
//! | 3..=18 | Up to 8 zone-ID slots, 2 bytes each (little-endian). Unused slots zero. |
//! | 19..=50 | Up to 8 color slots, 4 bytes each. Unused slots zero. |
//!
//! **Open problem, do not treat as solved:** the 4-byte color slot is
//! *usually* `[G, R, B, 0xFF]` (green/red channels swapped from what you'd
//! expect, alpha/enable always `0xFF`), but which zones need this swap
//! produced contradictory results across separate test sessions on the same
//! hardware for the left/back sidebar and back-corner zones specifically.
//! The leading theory is interference from Armoury Crate's background
//! service still running and racing writes to the same interface during
//! testing -- this was never conclusively ruled out. **Re-verify swap
//! behavior per zone, in full isolation, with a channel-revealing color
//! (Red or Green -- never Blue/Yellow/White) before trusting any swap table,
//! including the one below.**

use serde::{Deserialize, Serialize};
#[cfg(feature = "dbus")]
use zbus::zvariant::{OwnedValue, Type, Value};

use crate::Colour;

pub const LIGHTBAR_2025_REPORT_ID: u8 = 0x04;
/// Total payload length including the report ID byte.
pub const LIGHTBAR_2025_PACKET_LEN: usize = 51;
/// Max zones settable in a single packet.
pub const LIGHTBAR_2025_MAX_ZONES_PER_PACKET: usize = 8;

/// All 16 addressable zones on this device: 4 keyboard segments plus 12
/// lightbar segments wrapping the chassis (back bar, both corners at each
/// end, both side strips split front/back, front bar).
///
/// Numeric values are the raw wire zone IDs, verified by direct
/// single-zone-isolation testing (only the named zone lit, everything else
/// forced off, before recording the result).
///
/// Variant NAMES (not values) were corrected 2026-07-23 against ASUS's own
/// official Aura Creator device profile for this laptop
/// (`usb_capture_session3/ground_truth/WDL_G615LR.csv`), cross-validated by
/// a human-confirmed live 12-zone capture
/// (`usb_capture_session4/multizone_12x_confirmed.pcapng`) and the labeled
/// diagram at `usb_capture_session3/g615lr_zone_map.png`. Six of sixteen
/// were wrong (back edge `0x04-0x07`, side-back/front split `0x08/0x09`
/// and `0x0A/0x0B`) -- the wire IDs themselves (hex values below) were
/// never wrong and no previously-sent packet bytes change as a result of
/// this fix; only what a human calls each zone was incorrect. See
/// `HANDOFF.md` "Windows session 3" for the full derivation.
#[cfg_attr(
    feature = "dbus",
    derive(Type, Value, OwnedValue),
    zvariant(signature = "u")
)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum Lightbar2025Zone {
    #[default]
    Keyboard1 = 0x00,
    Keyboard2 = 0x01,
    Keyboard3 = 0x02,
    Keyboard4 = 0x03,
    BackBarRight = 0x04,
    BackBarLeft = 0x05,
    CornerBackRight = 0x06,
    CornerBackLeft = 0x07,
    SideRightBack = 0x08,
    SideLeftBack = 0x09,
    SideRightFront = 0x0A,
    SideLeftFront = 0x0B,
    CornerFrontRight = 0x0C,
    CornerFrontLeft = 0x0D,
    FrontBarRight = 0x0E,
    FrontBarLeft = 0x0F,
}

impl Lightbar2025Zone {
    pub const ALL: [Lightbar2025Zone; 16] = [
        Self::Keyboard1,
        Self::Keyboard2,
        Self::Keyboard3,
        Self::Keyboard4,
        Self::BackBarRight,
        Self::BackBarLeft,
        Self::CornerBackRight,
        Self::CornerBackLeft,
        Self::SideRightBack,
        Self::SideLeftBack,
        Self::SideRightFront,
        Self::SideLeftFront,
        Self::CornerFrontRight,
        Self::CornerFrontLeft,
        Self::FrontBarRight,
        Self::FrontBarLeft,
    ];

    /// Whether this zone's color slot needs the G/R channel swap.
    ///
    /// Targets wire IDs `0x09`/`0x0B` specifically (now named
    /// `SideLeftBack`/`SideLeftFront` after the 2026-07-23 zone-name
    /// correction above -- this match arm was updated to the new names but
    /// still targets the exact same two wire IDs the original empirical
    /// testing covered, so the underlying hardware behavior this encodes is
    /// unchanged by the rename).
    ///
    /// UNVERIFIED / INCONSISTENT -- see module docs. This is the LATEST
    /// result (BackBarLeft/Right and CornerBackLeft/Right flipped to
    /// no-swap after re-testing with a non-invariant color, Saffron
    /// #FF9933) but an EARLIER isolated test with pure Red/Green found the
    /// opposite for those same four zones. Both tests used non-invariant
    /// colors and were self-consistent at the time. Leading suspect:
    /// Armoury Crate's background services (ArmourySwAgent,
    /// LightingService, ROGLiveService, etc.) were never successfully
    /// stopped during testing and may race writes to this interface. Kill
    /// those services fully before re-testing this table. Also worth
    /// re-verifying now given the zone-name correction above -- the
    /// original swap testing may have been done under the wrong zone-ID
    /// assumptions (see `HANDOFF.md` "Windows session 3").
    pub fn needs_grb_swap(&self) -> bool {
        matches!(self, Self::SideLeftBack | Self::SideLeftFront)
    }
}

/// One (zone, color) pair to set in a single packet.
#[derive(Debug, Clone, Copy)]
pub struct Lightbar2025ZoneColour {
    pub zone: Lightbar2025Zone,
    pub colour: Colour,
}

/// Builds the raw 51-byte Feature report payload for up to 8 zone/color
/// pairs. Zones not included are left untouched by the hardware (this
/// protocol only updates zones explicitly listed) -- callers wanting
/// deterministic "everything else off" behavior must explicitly include
/// every zone they want turned off with a black colour, split across
/// multiple packets if more than 8 zones total need updating.
pub fn build_lightbar_2025_packet(zones: &[Lightbar2025ZoneColour]) -> [u8; LIGHTBAR_2025_PACKET_LEN] {
    assert!(
        !zones.is_empty() && zones.len() <= LIGHTBAR_2025_MAX_ZONES_PER_PACKET,
        "must set between 1 and {LIGHTBAR_2025_MAX_ZONES_PER_PACKET} zones per packet"
    );

    let mut pkt = [0u8; LIGHTBAR_2025_PACKET_LEN];
    pkt[0] = LIGHTBAR_2025_REPORT_ID;
    pkt[1] = zones.len() as u8;
    pkt[2] = 0x01;

    for (i, zc) in zones.iter().enumerate() {
        let zone_id = zc.zone as u16;
        let zoff = 3 + i * 2;
        pkt[zoff] = (zone_id & 0xFF) as u8;
        pkt[zoff + 1] = ((zone_id >> 8) & 0xFF) as u8;

        let coff = 19 + i * 4;
        if zc.zone.needs_grb_swap() {
            pkt[coff] = zc.colour.g;
            pkt[coff + 1] = zc.colour.r;
        } else {
            pkt[coff] = zc.colour.r;
            pkt[coff + 1] = zc.colour.g;
        }
        pkt[coff + 2] = zc.colour.b;
        pkt[coff + 3] = 0xFF; // alpha/enable, always full in every capture seen
    }

    pkt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_zone_packet_structure() {
        let pkt = build_lightbar_2025_packet(&[Lightbar2025ZoneColour {
            zone: Lightbar2025Zone::Keyboard1,
            colour: Colour { r: 255, g: 0, b: 0 },
        }]);
        assert_eq!(pkt[0], 0x04);
        assert_eq!(pkt[1], 1); // count
        assert_eq!(pkt[3], 0x00); // zone id low byte
        assert_eq!(pkt[4], 0x00); // zone id high byte
        // Keyboard1 does not swap -> plain R,G,B
        assert_eq!(pkt[19], 255);
        assert_eq!(pkt[20], 0);
        assert_eq!(pkt[21], 0);
        assert_eq!(pkt[22], 0xFF);
        assert_eq!(pkt.len(), 51);
    }

    #[test]
    fn swap_zone_reorders_channels() {
        let pkt = build_lightbar_2025_packet(&[Lightbar2025ZoneColour {
            zone: Lightbar2025Zone::SideLeftFront,
            colour: Colour { r: 255, g: 0, b: 0 },
        }]);
        // SideLeftFront is wire 0x0B -- assert the zone ID explicitly so a
        // future accidental re-numbering (like the one fixed 2026-07-23)
        // fails this test instead of silently passing.
        assert_eq!(pkt[3], 0x0B);
        assert_eq!(pkt[4], 0x00);
        // SideLeftFront needs swap -> G,R,B on the wire
        assert_eq!(pkt[19], 0); // g
        assert_eq!(pkt[20], 255); // r
        assert_eq!(pkt[21], 0); // b
    }

    #[test]
    #[should_panic]
    fn rejects_more_than_eight_zones() {
        let z = Lightbar2025ZoneColour {
            zone: Lightbar2025Zone::Keyboard1,
            colour: Colour::default(),
        };
        build_lightbar_2025_packet(&[z; 9]);
    }

    /// Every zone/colour pair from the human-confirmed-correct live
    /// capture (`usb_capture_session4/multizone_12x_confirmed.pcapng`,
    /// `HANDOFF.md` "Windows session 3", 12 of 16 zones lit distinct
    /// colours simultaneously, visually verified against the physical
    /// hardware twice). Confirms every wire zone ID this crate assigns
    /// matches what real hardware testing confirmed, and that non-swap
    /// zones' colour byte order matches the real captured bytes exactly.
    /// Doesn't exercise `needs_grb_swap` (the two swap zones, 0x09/0x0B,
    /// were sent black in that capture, which is swap-invariant) -- see
    /// `swap_zone_reorders_channels` above for that coverage instead.
    #[test]
    fn matches_human_confirmed_capture() {
        use Lightbar2025Zone::*;
        let cases: &[(Lightbar2025Zone, u8, [u8; 3])] = &[
            (Keyboard1, 0x00, [0xFF, 0x00, 0x00]),
            (Keyboard2, 0x01, [0x00, 0xFF, 0x00]),
            (Keyboard3, 0x02, [0x00, 0x00, 0xFF]),
            (Keyboard4, 0x03, [0xFF, 0xFF, 0xFF]),
            (BackBarLeft, 0x05, [0xFF, 0x00, 0x00]),
            (BackBarRight, 0x04, [0x00, 0xFF, 0x00]),
            (CornerBackLeft, 0x07, [0x00, 0x00, 0xFF]),
            (CornerBackRight, 0x06, [0xFF, 0xFF, 0x00]),
            (CornerFrontRight, 0x0C, [0xFF, 0x80, 0x00]),
            (CornerFrontLeft, 0x0D, [0xFF, 0xFF, 0xFF]),
            (FrontBarRight, 0x0E, [0x00, 0xFF, 0xFF]),
            (FrontBarLeft, 0x0F, [0xFF, 0x00, 0xFF]),
        ];
        for (zone, wire_id, rgb) in cases {
            let pkt = build_lightbar_2025_packet(&[Lightbar2025ZoneColour {
                zone: *zone,
                colour: Colour {
                    r: rgb[0],
                    g: rgb[1],
                    b: rgb[2],
                },
            }]);
            assert_eq!(pkt[3], *wire_id, "{zone:?} wire ID low byte");
            assert_eq!(pkt[4], 0x00, "{zone:?} wire ID high byte");
            assert_eq!(pkt[19], rgb[0], "{zone:?} R");
            assert_eq!(pkt[20], rgb[1], "{zone:?} G");
            assert_eq!(pkt[21], rgb[2], "{zone:?} B");
            assert_eq!(pkt[22], 0xFF, "{zone:?} alpha");
        }
    }
}
