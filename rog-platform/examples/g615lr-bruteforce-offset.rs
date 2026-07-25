use std::error::Error;
use std::time::Duration;

use rog_aura::keyboard::LedUsbPackets;

/// Scoped, safety-conscious brute force: keyboard zones respond to the
/// real "custom mode" (`0x5d bc`) protocol at documented byte offsets
/// 9/12/15/18. The documented lightbar codes at 27/30/33/36/39/42
/// (designed for G634J/G635L's lightbar hardware) don't respond on this
/// G615LR. Hypothesis: G615LR's real lightbar bytes might live at some
/// OTHER, undocumented offset within this same 64-byte buffer, specific
/// to this model's firmware, that nobody has ever tried.
///
/// SAFETY: report ID (`0x5d`) and mode byte (`0xbc`) are held constant
/// throughout -- only the byte OFFSET where an RGB value is written
/// varies. This stays entirely within the already-proven-safe "set LED
/// colour" command space; it never touches a different report ID or
/// subcommand (e.g. power/sleep controls), which is the only category of
/// risk here. RGB byte values are inherently safe -- there's no such
/// thing as an "invalid" colour byte that could damage an LED driver.
///
/// Sweeps every plausible 3-byte-aligned offset from 5 to 61 (skipping
/// the already-known keyboard-zone offsets), writing bright white at
/// each one in turn, holding for 3s, then resetting to a dark zoned
/// packet before trying the next -- easy to visually isolate any hit.
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

    // Dark reset (real Static black, builtin mode).
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

    // Real custom-mode init -- proven live to work on this hardware.
    let init_msg = LedUsbPackets::get_init_msg();
    send!("0x025d bc (CUSTOM MODE INIT) iface0", 0x09, 0x025du16, 0u16, &init_msg);
    println!("Custom-mode init sent. Waiting 1s...");
    std::thread::sleep(Duration::from_secs(1));

    // Known keyboard-zone offsets (already confirmed working) -- skip
    // these, we already know what they do.
    let known_offsets = [9u8, 12, 15, 18];

    // Candidate offsets to sweep: every 3-byte-aligned position from 5
    // to 61 that isn't already a known keyboard zone.
    let mut candidates: Vec<u8> = (5..=61).step_by(3).collect();
    candidates.retain(|o| !known_offsets.contains(o));

    println!("Sweeping {} candidate byte offsets, report=0x5d mode=0xbc held constant throughout (safe -- only colour-byte position varies)...", candidates.len());

    for &offset in &candidates {
        let mut pkt = [0u8; 64];
        pkt[0] = 0x5d;
        pkt[1] = 0xbc;
        pkt[2] = 0x01;
        pkt[3] = 0x01;
        pkt[4] = 0x04; // multizoned flag, matches new_zoned(true)
        let o = offset as usize;
        if o + 2 < 64 {
            pkt[o] = 0xff;
            pkt[o + 1] = 0xff;
            pkt[o + 2] = 0xff; // bright white
        }
        println!("--- offset {offset} (0x{offset:02x}): writing white ---");
        let r = handle.write_control(0x21, 0x09, 0x025du16, 0u16, &pkt, Duration::from_secs(2));
        if r.is_err() {
            println!("  write failed: {r:?}");
        }
        std::thread::sleep(Duration::from_secs(3));

        // Reset to dark zoned packet before the next candidate.
        let mut dark_zoned = [0u8; 64];
        dark_zoned[0] = 0x5d;
        dark_zoned[1] = 0xbc;
        dark_zoned[2] = 0x01;
        dark_zoned[3] = 0x01;
        dark_zoned[4] = 0x04;
        let _ = handle.write_control(0x21, 0x09, 0x025du16, 0u16, &dark_zoned, Duration::from_secs(2));
        std::thread::sleep(Duration::from_millis(500));
    }

    println!("Sweep complete. If nothing lit up beyond the known keyboard zones, note which offset (if any) showed ANYTHING at all, even briefly.");
    Ok(())
}
