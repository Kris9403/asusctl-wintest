use std::error::Error;
use std::time::Duration;

/// Replicates Windows session 9's BREAKTHROUGH -- the first-ever
/// successful `0x04` lightbar activation from code, anywhere, on either
/// OS, this entire investigation. Every prior `0x04` test all session
/// (both OSes) used `count=1` (one zone per packet). This is the first
/// `count>1` test ever run on Linux.
///
/// Exact packet, per Windows' own empirically-verified (not hand-
/// transcribed -- built from a script measuring smoothness across 466
/// real `0x04` writes from `25/test123.pcapng`+`25/123.pcapng`) byte
/// layout:
/// ```text
/// data[0]        report ID (0x04)
/// data[1]        zone count N
/// data[2]        flag, always 0x01
/// data[3:19]     zone-ID list, N x u16 LE, zero-padded to 16 bytes (8 zone slots)
/// data[19:19+4N] N x RGBA (R,G,B,Alpha), same order as zone-ID list
/// data[19+4N:]   unused, zero
/// ```
/// Zone list: kbd1, kbd2, kbd3, kbd4, back_right (wire IDs 0x00-0x04).
/// Colours: keyboard zones at (0,0,0,1) -- alpha~0, effectively
/// invisible -- back_right at (0x61,0xff,0x00,0xff) -- full alpha,
/// bright yellow-green. This exact packet lit the lightbar zone live on
/// Windows, twice, wire-verified byte-for-byte identical to a real Aura
/// Creator capture (`static_armory_to_aura_lightbar_only.pcapng`).
///
/// Streamed continuously since we don't have Aura's own `0x0305` stream
/// to hold the state, matching what Windows did in its own reproduction.
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

    // Real priming triplet (b3,b4,b5 order, mode=0x02 RainbowCycle) --
    // matching what real Aura sent before its own working write, and
    // what Windows' own successful reproduction used.
    #[rustfmt::skip]
    let b3_prime: [u8; 64] = [
        0x5d, 0xb3, 0x00, 0x02, 0x00, 0x00, 0x00, 0xeb, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
    ];
    let mut b4_prime = [0u8; 64];
    b4_prime[0] = 0x5d;
    b4_prime[1] = 0xb4;
    let mut b5_prime = [0u8; 64];
    b5_prime[0] = 0x5d;
    b5_prime[1] = 0xb5;
    send!("0x025d b3 (prime -> RainbowCycle) iface0", 0x09, 0x025du16, 0u16, &b3_prime);
    send!("0x025d b4 (prime) iface0", 0x09, 0x025du16, 0u16, &b4_prime);
    send!("0x025d b5 (prime) iface0", 0x09, 0x025du16, 0u16, &b5_prime);

    println!("Priming sent. Waiting 2s...");
    std::thread::sleep(Duration::from_secs(2));

    let handshake05: [u8; 10] = [0x05, 0x00, 0x08, 0x00, 0x0f, 0x00, 0x00, 0x00, 0x00, 0x01];
    send!("0x0305 (handshake) iface1", 0x09, 0x0305u16, 1u16, &handshake05);

    // Build the exact count=5 packet using the confirmed formula.
    let zone_ids: [u8; 5] = [0x00, 0x01, 0x02, 0x03, 0x04]; // kbd1,kbd2,kbd3,kbd4,back_right
    let colours: [(u8, u8, u8, u8); 5] = [
        (0x00, 0x00, 0x00, 0x01), // kbd1: effectively invisible
        (0x00, 0x00, 0x00, 0x01), // kbd2
        (0x00, 0x00, 0x00, 0x01), // kbd3
        (0x00, 0x00, 0x00, 0x01), // kbd4
        (0x61, 0xff, 0x00, 0xff), // back_right: bright yellow-green, full alpha
    ];

    let mut pkt = [0u8; 51];
    pkt[0] = 0x04;
    pkt[1] = 5; // count
    pkt[2] = 0x01; // flag
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
    println!("Streaming count=5 multizone packet (kbd1-4 near-invisible, back_right yellow-green full alpha) for 20s...");

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

    println!("Done. Watch for: does the BACK-RIGHT lightbar zone light up yellow-green, with keyboard staying off/unchanged?");
    Ok(())
}
