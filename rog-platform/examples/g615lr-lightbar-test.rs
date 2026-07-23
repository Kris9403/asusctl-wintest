use std::error::Error;
use std::path::Path;

use rog_platform::hid_raw::HidRaw;

/// Throwaway hardware test for the G615LR's second Aura protocol (HID
/// Feature report 0x04) -- see HANDOFF.md / docs/g615lr-aura-protocol.md in
/// the repo root. Lights a single zone (Keyboard1) bright red and nothing
/// else, per the "isolate one variable at a time" discipline that produced
/// every other finding in this investigation.
///
/// Usage: sudo target/debug/examples/g615lr-lightbar-test [/dev/hidrawN]
/// Defaults to /dev/hidraw2, confirmed via
/// `udevadm info -a -n /dev/hidraw2 | grep bInterfaceNumber` to be
/// interface 01 (MI_01) -- the interface the Windows investigation found
/// report 0x04 on. If nothing happens, try other /dev/hidrawN nodes with
/// matching idProduct before assuming the Rust code is wrong.
fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let devnode = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/dev/hidraw2".into());
    let hid = HidRaw::from_devnode(Path::new(&devnode), "19b6")?;

    // 51-byte packet, hand-built to match
    // rog_aura::lightbar_2025::build_lightbar_2025_packet's output for a
    // single (Keyboard1, red) pair (see that module's tests) -- inlined
    // here rather than pulling rog_aura in as a dev-dependency.
    let mut packet = [0u8; 51];
    packet[0] = 0x04; // report ID
    packet[1] = 0x01; // zone count
    packet[2] = 0x01; // flag byte, meaning unconfirmed but consistently 0x01
    packet[3] = 0x00; // zone id slot 0, low byte  (Keyboard1 = 0x00)
    packet[4] = 0x00; // zone id slot 0, high byte
    packet[19] = 0xFF; // color slot 0: R (Keyboard1 needs no GRB swap)
    packet[20] = 0x00; // G
    packet[21] = 0x00; // B
    packet[22] = 0xFF; // alpha/enable, always full

    hid.set_feature_report(&packet)?;
    println!("Sent: Keyboard1 = bright red, via {devnode}. Check the hardware now.");
    Ok(())
}
