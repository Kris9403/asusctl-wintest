use std::error::Error;
use std::time::Duration;

/// Extends `g615lr-bruteforce-offset.rs` past its hard limit (offset 61
/// is the last 3-byte-aligned position in a single 64-byte packet -- it
/// physically can't go further within one packet).
///
/// Real, code-grounded lead instead of a blind guess: `LedUsbPackets::
/// new_per_key()` (`rog-aura/src/keyboard/advanced.rs`) only allocates 11
/// packet rows (`vec![vec![0u8; 64]; 11]`, valid indices 0..=10), but its
/// OWN `rgb_for_led_code` match arms reference ROW 11 for every lightbar
/// code and lid code in non-zoned (per-key) mode:
/// ```ignore
/// LedCode::LightbarRight => if zoned {(0, 27)} else { (11, 15)},
/// LedCode::LightbarRightCorner => if zoned {(0, 30)} else {(11, 18)},
/// LedCode::LightbarRightBottom => if zoned {(0, 33)} else{(11, 21)},
/// LedCode::LightbarLeftBottom => if zoned {(0, 36)} else{(11, 24)},
/// LedCode::LightbarLeftCorner => if zoned {(0, 39)} else{(11, 27)},
/// LedCode::LightbarLeft => if zoned {(0, 42)} else{(11, 30)},
/// LedCode::LidLogo => (11, 9),
/// LedCode::LidLeft => (11, 36),
/// LedCode::LidRight => (11, 39),
/// ```
/// Row 11 is out of bounds for `new_per_key()`'s own 11-row Vec -- this
/// path was never actually finished/reachable in the existing code. But
/// the row/column INTENT is real, documented, and never tested: whoever
/// wrote this expected lightbar/lid data to live in an 12th packet
/// (group index 11) that nothing currently constructs.
///
/// Per `new_per_key()`'s own header-building loop, each row's group byte
/// is `(count as u8) << 4` at offset 6, with offset 7 = 0x10 normally, or
/// 0x08 specifically for the LAST row (index 10) -- meaning row 10 is
/// already known to be special/terminal in the real protocol. This test:
/// (a) sweeps row 10 (the one legitimately-allocated row we haven't
/// tried yet) at its own per-key column positions, then (b) manually
/// constructs the referenced-but-never-built "group 11" packet (group
/// byte 0xb0, matching the same pattern) and tests the EXACT column
/// positions (9, 15, 18, 21, 24, 27, 30, 36, 39) the existing code
/// already points at for lightbar/lid.
///
/// SAFETY: report ID (0x5d) and mode byte (0xbc) held constant
/// throughout, exactly like the offset-61 sweep -- only colour-byte
/// position and group index vary. Same safe "set LED colour" command
/// space, never a different report ID or subcommand.
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

    // Real per-key custom-mode init: report=5d, mode=bc, all else zero.
    let mut init_msg = [0u8; 64];
    init_msg[0] = 0x5d;
    init_msg[1] = 0xbc;
    send!("0x025d bc (CUSTOM MODE INIT, per-key) iface0", 0x09, 0x025du16, 0u16, &init_msg);
    println!("Custom-mode init sent. Waiting 1s...");
    std::thread::sleep(Duration::from_secs(1));

    // Per-key row header, matching new_per_key()'s own construction
    // exactly: row[0]=5d, row[1]=bc, row[2]=0, row[3]=1, row[4]=1,
    // row[5]=1, row[6]=group<<4, row[7]=0x08 if group==10 else 0x10,
    // row[8]=0.
    fn row_header(group: u8) -> [u8; 64] {
        let mut row = [0u8; 64];
        row[0] = 0x5d;
        row[1] = 0xbc;
        row[2] = 0x00;
        row[3] = 0x01;
        row[4] = 0x01;
        row[5] = 0x01;
        row[6] = group << 4;
        row[7] = if group == 10 { 0x08 } else { 0x10 };
        row[8] = 0x00;
        row
    }

    fn test_offset(
        handle: &rusb::DeviceHandle<impl rusb::UsbContext>,
        group: u8,
        offset: usize,
        label: &str,
    ) {
        let mut pkt = row_header(group);
        if offset + 2 < 64 {
            pkt[offset] = 0xff;
            pkt[offset + 1] = 0xff;
            pkt[offset + 2] = 0xff;
        }
        println!("--- group {group} offset {offset} ({label}): writing white ---");
        let r = handle.write_control(0x21, 0x09, 0x025du16, 0u16, &pkt, Duration::from_secs(2));
        if r.is_err() {
            println!("  write failed: {r:?}");
        }
        std::thread::sleep(Duration::from_secs(3));
        let dark = row_header(group);
        let _ = handle.write_control(0x21, 0x09, 0x025du16, 0u16, &dark, Duration::from_secs(2));
        std::thread::sleep(Duration::from_millis(500));
    }

    println!("=== Part 1: sweeping row (group) 10 -- the one legitimately-allocated per-key row never tried yet ===");
    for offset in (9..=57).step_by(3) {
        test_offset(&handle, 10, offset, "row10 sweep");
    }

    println!("=== Part 2: manually-constructed 'group 11' -- referenced by rgb_for_led_code but never built by new_per_key() ===");
    let group11_targets: [(usize, &str); 9] = [
        (9, "LidLogo"),
        (15, "LightbarRight"),
        (18, "LightbarRightCorner"),
        (21, "LightbarRightBottom"),
        (24, "LightbarLeftBottom"),
        (27, "LightbarLeftCorner"),
        (30, "LightbarLeft"),
        (36, "LidLeft"),
        (39, "LidRight"),
    ];
    for (offset, label) in group11_targets {
        test_offset(&handle, 11, offset, label);
    }

    println!("Sweep complete. Note which group/offset (if any) showed ANYTHING at all.");
    Ok(())
}
