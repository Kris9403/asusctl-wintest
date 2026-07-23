use std::error::Error;
use std::time::Duration;

use rog_aura::usb::{AURA_LAPTOP_LED_APPLY, AURA_LAPTOP_LED_SET};
use rog_aura::{AuraEffect, AuraModeNum, AuraZone, Colour, Direction, Speed};

/// Sends two AuraEffects back to back -- a CONFIRMED WORKING mode (Pulse)
/// and a CONFIRMED NOT-WORKING mode (Comet), live-tested via
/// `asusctl aura effect pulse/comet` -- using the REAL production
/// `From<&AuraEffect> for [u8; AURA_LAPTOP_LED_MSG_LEN]` conversion
/// (rog-aura/src/builtin_modes.rs), not hand-rolled bytes. Both packets
/// are structurally identical except for byte 3 (the mode number: 10 vs
/// 11). Goal: with a usbmon capture running alongside, check whether the
/// device's own interrupt-IN ACK ("5d ec b3...") comes back for Comet the
/// same way it does for Pulse, to tell apart "firmware silently ignores
/// this mode number" (hardware/firmware limitation) from "our code sends
/// something malformed for this mode" (fixable bug) -- see the user's
/// pushback in conversation: Windows proved individual zone (0x04) control
/// works at the protocol level, so nothing here should be written off as
/// hardware-incapable without direct evidence.
fn send_effect(
    handle: &rusb::DeviceHandle<rusb::GlobalContext>,
    iface: u8,
    label: &str,
    effect: &AuraEffect,
) -> Result<(), Box<dyn Error>> {
    const PADDED_LEN: usize = 64;
    let bytes: [u8; 17] = effect.into();
    let mut effect_padded = [0u8; PADDED_LEN];
    effect_padded[..17].copy_from_slice(&bytes);
    let mut set_padded = [0u8; PADDED_LEN];
    set_padded[..17].copy_from_slice(&AURA_LAPTOP_LED_SET);
    let mut apply_padded = [0u8; PADDED_LEN];
    apply_padded[..17].copy_from_slice(&AURA_LAPTOP_LED_APPLY);

    println!(
        "{label}: mode={:?} bytes={}",
        effect.mode,
        effect_padded[..17]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ")
    );

    for (name, pkt) in [
        ("effect", &effect_padded),
        ("set", &set_padded),
        ("apply", &apply_padded),
    ] {
        let r = handle.write_control(0x21, 0x09, 0x025d, iface as u16, pkt, Duration::from_secs(2));
        println!("  {label} {name}: {r:?}");
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let devices = rusb::devices()?;
    let mut target = None;
    for device in devices.iter() {
        let desc = device.device_descriptor()?;
        if desc.vendor_id() == 0x0B05 && desc.product_id() == 0x19B6 {
            target = Some(device);
            break;
        }
    }
    let device = target.ok_or("device 0B05:19B6 not found")?;
    let handle = device.open()?;
    let iface: u8 = 0;

    let had_driver = handle.kernel_driver_active(iface).unwrap_or(false);
    if had_driver {
        println!("Detaching kernel driver from interface 0...");
        handle.detach_kernel_driver(iface)?;
    }
    handle.claim_interface(iface)?;

    let pulse = AuraEffect {
        mode: AuraModeNum::Pulse,
        zone: AuraZone::None,
        colour1: Colour { r: 255, g: 0, b: 0 },
        colour2: Colour::default(),
        speed: Speed::Med,
        direction: Direction::Right,
    };
    send_effect(&handle, iface, "PULSE (works)", &pulse)?;

    std::thread::sleep(Duration::from_millis(3000));

    let comet = AuraEffect {
        mode: AuraModeNum::Comet,
        zone: AuraZone::None,
        colour1: Colour { r: 0, g: 255, b: 0 },
        colour2: Colour::default(),
        speed: Speed::Med,
        direction: Direction::Right,
    };
    send_effect(&handle, iface, "COMET (fails)", &comet)?;

    let _ = handle.release_interface(iface);
    if had_driver {
        let _ = handle.attach_kernel_driver(iface);
    }

    println!("Done. Check the usbmon capture for interrupt-IN ACKs after each.");
    Ok(())
}
