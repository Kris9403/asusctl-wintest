use std::error::Error;
use std::time::Duration;

/// Further extends the `0x5d bc` byte-position brute force. So far tried:
/// every offset in the single `new_zoned()` packet (0-61), and groups 10
/// + 11 (`g615lr-bruteforce-row11.rs`) at all their offsets -- nothing.
///
/// Untested territory: the group byte at offset 6 is `group << 4`, a
/// 4-bit field, so 15 is the maximum possible value. Groups 12-15 have
/// never been tried at all -- completely unexplored. Groups 0-9 (the
/// other per-key rows, addressing different keyboard keys per
/// `rgb_for_led_code`) have only had a handful of specific documented
/// keys tested, never a full sweep of every offset for UNUSED/undocumented
/// positions that might reach the lightbar the same way group 10/11's
/// candidates were checked.
///
/// Priority order: groups 12-15 first (genuinely novel, most likely to
/// reveal something new), then groups 0-9 (secondary, since their
/// documented positions are already known-safe/known-keyboard).
///
/// SAFETY: identical to every other brute-force test tonight -- report
/// ID (0x5d) and mode byte (0xbc) held constant, only group index and
/// RGB-value byte position vary. Same proven-safe "set LED colour"
/// command space throughout.
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

    let mut init_msg = [0u8; 64];
    init_msg[0] = 0x5d;
    init_msg[1] = 0xbc;
    send!("0x025d bc (CUSTOM MODE INIT) iface0", 0x09, 0x025du16, 0u16, &init_msg);
    println!("Custom-mode init sent. Waiting 1s...");
    std::thread::sleep(Duration::from_secs(1));

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

    fn test_offset(handle: &rusb::DeviceHandle<impl rusb::UsbContext>, group: u8, offset: usize) {
        let mut pkt = row_header(group);
        if offset + 2 < 64 {
            pkt[offset] = 0xff;
            pkt[offset + 1] = 0xff;
            pkt[offset + 2] = 0xff;
        }
        println!("--- group {group} offset {offset}: writing white ---");
        let r = handle.write_control(0x21, 0x09, 0x025du16, 0u16, &pkt, Duration::from_secs(2));
        if r.is_err() {
            println!("  write failed: {r:?}");
        }
        std::thread::sleep(Duration::from_secs(2));
        let dark = row_header(group);
        let _ = handle.write_control(0x21, 0x09, 0x025du16, 0u16, &dark, Duration::from_secs(2));
        std::thread::sleep(Duration::from_millis(300));
    }

    println!("=== Part 1: groups 12-15 (novel, group byte is 4 bits, 15 is the max possible value) ===");
    for group in 12u8..=15 {
        for offset in (9..=57).step_by(3) {
            test_offset(&handle, group, offset);
        }
    }

    println!("=== Part 2: groups 0-9, unused/undocumented offsets only (skipping known-mapped key positions) ===");
    // Known-documented column positions per row (from rgb_for_led_code) --
    // skip these, already understood as real keyboard keys, focus only on
    // gaps.
    let known_by_group: [(u8, &[usize]); 10] = [
        (0, &[15, 18, 21, 24]),
        (1, &[24, 30, 33, 36, 39, 45, 48, 51, 54]),
        (2, &[12, 15, 18, 21, 24, 39, 42, 45, 48, 51, 54]),
        (3, &[9, 12, 15, 18, 21, 24, 27, 30, 33, 36, 39, 54]),
        (4, &[9, 12, 15, 18, 21, 24, 27, 30, 33, 36, 39, 42, 45, 54]),
        (5, &[21, 24, 27, 30, 33, 36, 39, 42, 45, 48, 51, 54]),
        (6, &[9, 12, 15, 18, 21, 36, 42, 45, 48, 51, 54]),
        (7, &[9, 12, 15, 18, 21, 24, 27, 30, 33, 36, 51, 54]),
        (8, &[9, 12, 15, 18, 21, 24, 27, 30, 33, 36, 42, 51]),
        (9, &[54]),
    ];
    for (group, known) in known_by_group {
        for offset in (9..=57).step_by(3) {
            if known.contains(&offset) {
                continue;
            }
            test_offset(&handle, group, offset);
        }
    }

    println!("Sweep complete. Note which group/offset (if any) showed ANYTHING at all.");
    Ok(())
}
