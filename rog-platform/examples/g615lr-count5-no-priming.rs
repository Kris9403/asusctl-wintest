use std::error::Error;
use std::time::Duration;

/// Direct follow-up to `g615lr-count5-multizone.rs`: that test replicated
/// Windows session 9's breakthrough packet (count=5, kbd1-4 near-
/// invisible + back_right full alpha yellow-green) with the same real
/// RainbowCycle priming Windows used -- but got a different visual
/// result: RainbowCycle dominated the WHOLE chassis (keyboard AND
/// lightbar), same confound hit dozens of times all session with
/// count=1 packets, whereas Windows got a clean override (lightbar lit,
/// keyboard off) with the identical bytes.
///
/// Every prior "no priming" test this session used count=1, never
/// count=5 -- this combination has never been tried. Same exact count=5
/// packet, but a real dark reset instead of the RainbowCycle-triggering
/// priming triplet, removing the competing-animation confound entirely.
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

    // Real dark reset (Static black, zone=None, real b3,b5,b4 order) --
    // NOT the RainbowCycle-triggering priming order. Never starts any
    // animation loop.
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
    println!("Dark reset sent -- no priming, no animation. Waiting 2s...");
    std::thread::sleep(Duration::from_secs(2));

    let handshake05: [u8; 10] = [0x05, 0x00, 0x08, 0x00, 0x0f, 0x00, 0x00, 0x00, 0x00, 0x01];
    send!("0x0305 (handshake) iface1", 0x09, 0x0305u16, 1u16, &handshake05);

    // Exact same count=5 packet as g615lr-count5-multizone.rs.
    let zone_ids: [u8; 5] = [0x00, 0x01, 0x02, 0x03, 0x04];
    let colours: [(u8, u8, u8, u8); 5] = [
        (0x00, 0x00, 0x00, 0x01),
        (0x00, 0x00, 0x00, 0x01),
        (0x00, 0x00, 0x00, 0x01),
        (0x00, 0x00, 0x00, 0x01),
        (0x61, 0xff, 0x00, 0xff),
    ];

    let mut pkt = [0u8; 51];
    pkt[0] = 0x04;
    pkt[1] = 5;
    pkt[2] = 0x01;
    for (i, &zid) in zone_ids.iter().enumerate() {
        pkt[3 + i * 2] = zid;
        pkt[3 + i * 2 + 1] = 0x00;
    }
    for (i, &(r, g, b, a)) in colours.iter().enumerate() {
        let off = 19 + i * 4;
        pkt[off] = r;
        pkt[off + 1] = g;
        pkt[off + 2] = b;
        pkt[off + 3] = a;
    }

    println!("Packet bytes: {:02x?}", pkt);
    println!("No priming. Streaming count=5 multizone packet for 20s...");

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

    println!("Done. Watch for: does the BACK-RIGHT lightbar zone light up yellow-green?");
    Ok(())
}
