use std::error::Error;
use std::time::Duration;

use rog_aura::keyboard::{LedCode, LedUsbPackets};

/// Follow-up to `g615lr-perkey-zoned-protocol.rs`, which just confirmed
/// LIVE on real hardware: the "custom mode" (`0x5d bc`) protocol's
/// 4-zone `new_zoned()` addressing genuinely works for the keyboard on
/// this G615LR (first real independent per-zone success all session --
/// lightbar codes didn't respond, keyboard zones did). This test goes
/// further: TRUE per-key mode (`LedUsbPackets::new_per_key()`), genuine
/// individual-key RGB addressing (11 packets, ~90+ keys), using the real
/// library code directly (`rog_aura` is already a dev-dependency of this
/// crate) instead of hand-transcribed bytes -- removes any possibility
/// of a transcription error mattering here.
///
/// If 4-zone addressing works, true per-key very plausibly does too --
/// this would be a full, real, per-key RGB win independent of the
/// still-unresolved lightbar `0x04` mystery.
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

    let had0 = handle.kernel_driver_active(0).unwrap_or(false);
    if had0 {
        handle.detach_kernel_driver(0)?;
    }
    let had1 = handle.kernel_driver_active(1).unwrap_or(false);
    if had1 {
        handle.detach_kernel_driver(1)?;
    }
    handle.claim_interface(0)?;
    handle.claim_interface(1)?;

    struct RestoreGuard<'a, T: rusb::UsbContext> {
        handle: &'a rusb::DeviceHandle<T>,
        had0: bool,
        had1: bool,
    }
    impl<'a, T: rusb::UsbContext> Drop for RestoreGuard<'a, T> {
        fn drop(&mut self) {
            let _ = self.handle.release_interface(0);
            let _ = self.handle.release_interface(1);
            if self.had0 {
                let _ = self.handle.attach_kernel_driver(0);
            }
            if self.had1 {
                let _ = self.handle.attach_kernel_driver(1);
            }
        }
    }
    let _restore_guard = RestoreGuard { handle: &handle, had0, had1 };

    macro_rules! send {
        ($label:expr, $req:expr, $val:expr, $idx:expr, $data:expr) => {
            let r = handle.write_control(0x21, $req, $val, $idx, $data, Duration::from_secs(2));
            println!("{}: {:?}", $label, r);
        };
    }

    send!("SET_IDLE iface1", 0x0a, 0x0000u16, 1u16, &[]);
    send!("SET_IDLE iface0", 0x0a, 0x0000u16, 0u16, &[]);
    send!("0x0201 (01 01) iface0", 0x09, 0x0201u16, 0u16, &[0x01, 0x01]);

    // Dark reset first.
    #[rustfmt::skip]
    let dark_b3: [u8; 64] = [
        0x5d, 0xb3, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
    ];
    let mut dark_b5 = [0u8; 64];
    dark_b5[0] = 0x5d;
    dark_b5[1] = 0xb5;
    let mut dark_b4 = [0u8; 64];
    dark_b4[0] = 0x5d;
    dark_b4[1] = 0xb4;
    send!("0x025d b3 (dark reset) iface0", 0x09, 0x025du16, 0u16, &dark_b3);
    send!("0x025d b5 (set) iface0", 0x09, 0x025du16, 0u16, &dark_b5);
    send!("0x025d b4 (apply) iface0", 0x09, 0x025du16, 0u16, &dark_b4);
    println!("Dark reset sent. Waiting 2s...");
    std::thread::sleep(Duration::from_secs(2));

    // Real init message from the library.
    let init_msg = LedUsbPackets::get_init_msg();
    send!("0x025d bc (CUSTOM MODE INIT) iface0", 0x09, 0x025du16, 0u16, &init_msg);
    println!("Custom-mode init sent. Waiting 1s...");
    std::thread::sleep(Duration::from_secs(1));

    // Real per-key packets from the library -- set a spread of keys
    // across the whole board to distinct bright colours so any lit key
    // is easy to spot.
    let mut per_key = LedUsbPackets::new_per_key();
    let keys_colours: [(LedCode, (u8, u8, u8)); 12] = [
        (LedCode::Esc, (255, 0, 0)),
        (LedCode::F7, (0, 255, 0)),
        (LedCode::Tilde, (0, 0, 255)),
        (LedCode::N5, (255, 255, 0)),
        (LedCode::Tab, (255, 0, 255)),
        (LedCode::G, (0, 255, 255)),
        (LedCode::Caps, (255, 128, 0)),
        (LedCode::Return, (128, 0, 255)),
        (LedCode::Z, (255, 255, 255)),
        (LedCode::B, (255, 0, 128)),
        (LedCode::Spacebar5_3, (0, 128, 255)),
        (LedCode::Right, (128, 255, 0)),
    ];
    for (key, (r, g, b)) in keys_colours {
        per_key.set(key, r, g, b);
        println!("Set {key:?} -> ({r},{g},{b})");
    }

    let packets = per_key.get();
    println!("Sending {} per-key packets...", packets.len());
    for (i, row) in packets.iter().enumerate() {
        let r = handle.write_control(0x21, 0x09, 0x025du16, 0u16, row, Duration::from_secs(2));
        println!("  row {i}: {r:?}");
    }

    println!("Sent. Watch the WHOLE keyboard for any of these 12 keys lighting up in their assigned colour. Holding for 12s...");
    std::thread::sleep(Duration::from_secs(12));

    println!("Done.");
    Ok(())
}
