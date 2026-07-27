use std::error::Error;
use std::time::Duration;

/// One final combination, per direct request: dark -> static blue ->
/// front-lightbar-only, all in one sequence. Echoes something flagged
/// much earlier this session (a pasted Gemini conversation specifically
/// called out "Dark -> Static Blue -> Custom Aura Creator" as producing
/// "the exact right traffic"). Every prior test tonight either primed
/// into RainbowCycle or did a plain dark reset before `0x04` -- never a
/// distinct STATIC BLUE transition step, and never targeting the whole
/// front lightbar strip as its own multi-zone batch (only ever
/// individual zones, or the back_right zone from the Windows breakthrough
/// replication).
///
/// Sequence: dark reset (Static black) -> static blue (whole chassis,
/// same classic protocol, a real, distinct colour transition) -> wait ->
/// 0x0305 handshake -> count=4 packet targeting ONLY the front lightbar
/// zones (front_corner_right 0x0C, front_corner_left 0x0D, front_bar_right
/// 0x0E, front_bar_left 0x0F -- keyboard zones not addressed at all this
/// time, not even at near-zero alpha) -- bright cyan, full alpha.
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

    // Step 1: dark reset (Static black, zone=None, real b3,b5,b4 order).
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

    // Step 2: static BLUE, whole chassis, zone=None, real b3,b5,b4 order.
    #[rustfmt::skip]
    let blue_b3: [u8; 64] = [
        0x5d, 0xb3, 0x00, 0x00, 0x00, 0x00, 0xff, 0xeb, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
    ];
    let mut blue_b5 = [0u8; 64];
    blue_b5[0] = 0x5d;
    blue_b5[1] = 0xb5;
    let mut blue_b4 = [0u8; 64];
    blue_b4[0] = 0x5d;
    blue_b4[1] = 0xb4;
    send!("0x025d b3 (static BLUE) iface0", 0x09, 0x025du16, 0u16, &blue_b3);
    send!("0x025d b5 (set) iface0", 0x09, 0x025du16, 0u16, &blue_b5);
    send!("0x025d b4 (apply) iface0", 0x09, 0x025du16, 0u16, &blue_b4);
    println!("Static blue sent -- whole chassis should be blue now. Waiting 3s...");
    std::thread::sleep(Duration::from_secs(3));

    let handshake05: [u8; 10] = [0x05, 0x00, 0x08, 0x00, 0x0f, 0x00, 0x00, 0x00, 0x00, 0x01];
    send!("0x0305 (handshake) iface1", 0x09, 0x0305u16, 1u16, &handshake05);

    // Step 3: count=4, ONLY the front lightbar zones, bright cyan, full alpha.
    let zone_ids: [u8; 4] = [0x0C, 0x0D, 0x0E, 0x0F];
    let mut pkt = [0u8; 51];
    pkt[0] = 0x04;
    pkt[1] = 4; // count
    pkt[2] = 0x01;
    for (i, &zid) in zone_ids.iter().enumerate() {
        pkt[3 + i * 2] = zid;
        pkt[3 + i * 2 + 1] = 0x00;
    }
    for i in 0..4 {
        let off = 19 + i * 4;
        pkt[off] = 0x00; // R
        pkt[off + 1] = 0xff; // G
        pkt[off + 2] = 0xff; // B  (cyan)
        pkt[off + 3] = 0xff; // A full
    }

    println!("Packet bytes: {:02x?}", pkt);
    println!("Streaming front-lightbar-only (corners + bars, cyan) for 20s...");

    let start = std::time::Instant::now();
    let mut cycles = 0u32;
    while start.elapsed() < Duration::from_secs(20) {
        let r = handle.write_control(0x21, 0x09, 0x0304u16, 1u16, &pkt, Duration::from_millis(500));
        if r.is_err() {
            println!("  write: {r:?}");
        }
        cycles += 1;
        std::thread::sleep(Duration::from_millis(100));
    }
    println!("Done streaming: {cycles} writes over 20s.");

    println!("Done. Watch the FRONT lightbar (both corners + both bars) for cyan, distinct from the blue everywhere else.");
    Ok(())
}
