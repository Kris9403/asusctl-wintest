use std::error::Error;
use std::time::Duration;

/// Maximally isolated test, requested directly: asusd fully stopped
/// (no D-Bus/config-layer involvement at all), NO classic-protocol
/// priming triplet (no `5d b3/b4/b5`, so the animation engine never gets
/// triggered into RainbowCycle or anything else -- completely removes
/// the confound that every other `0x04` test this session has had), and
/// targets a REAL lightbar corner zone (front-left, wire ID `0x0D`,
/// `CornerFrontLeft`) instead of a keyboard zone. Only the bare minimum
/// interface setup (SET_IDLE, the `0x0201` init byte, the `0x0305`
/// handshake) plus the same real, wire-verified-correct alpha-ramping
/// `0x04` stream from `g615lr-alpha-ramp.rs`
/// (`linux_capture_session6/alpha_ramp_0x04_test_wire_verified.pcapng`
/// already proved this exact packet shape reaches the wire correctly).
///
/// This isolates one specific question: does `0x04` do ANYTHING at all
/// -- on a real lightbar zone, not keyboard -- when the classic
/// protocol's animation engine is never touched in the first place,
/// rather than triggered-then-fought-with (every prior test) or
/// triggered-then-cancelled (`g615lr-cancel-rainbow-then-04.rs`).
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

    // Explicit dark reset first (real Static black, zone=None, real
    // b3,b5,b4 SET/APPLY order) -- NOT the RainbowCycle-triggering
    // "priming" order (b3,b4,b5, mode=0x02). Static never starts any
    // animation loop, so this establishes a confirmed-dark baseline
    // without violating the "animation engine never touched" design of
    // this test.
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
    send!("0x025d b3 (dark reset, Static black) iface0", 0x09, 0x025du16, 0u16, &dark_b3);
    send!("0x025d b5 (set) iface0", 0x09, 0x025du16, 0u16, &dark_b5);
    send!("0x025d b4 (apply) iface0", 0x09, 0x025du16, 0u16, &dark_b4);
    println!("Dark reset sent -- ALL zones should now be off. Waiting 2s to confirm dark...");
    std::thread::sleep(Duration::from_secs(2));

    // Deliberately NO 0x5d b3/b4/b5 priming -- the whole point of this
    // test. Straight to the 0x0305 handshake and the 0x04 stream.
    let handshake05: [u8; 10] = [0x05, 0x00, 0x08, 0x00, 0x0f, 0x00, 0x00, 0x00, 0x00, 0x01];
    send!("0x0305 (handshake) iface1", 0x09, 0x0305u16, 1u16, &handshake05);

    println!("No priming sent. Streaming front-left corner (zone 0x0D, CornerFrontLeft) with a genuinely ramping alpha for 15s...");

    let alpha_wave: [u8; 14] = [
        0x06, 0x18, 0x35, 0x58, 0x80, 0xa7, 0xcb, 0xe7, 0xfb, 0xff, 0xf5, 0xe0, 0xc2, 0x9d,
    ];

    fn build_packet(zone_id: u8, r: u8, g: u8, b: u8, a: u8) -> [u8; 51] {
        let mut pkt = [0u8; 51];
        pkt[0] = 0x04;
        pkt[1] = 0x01; // count = 1
        pkt[2] = 0x01;
        pkt[3] = zone_id;
        pkt[4] = 0x00;
        pkt[19] = r;
        pkt[20] = g;
        pkt[21] = b;
        pkt[22] = a;
        pkt
    }

    let start = std::time::Instant::now();
    let mut i = 0usize;
    let mut cycles = 0u32;
    while start.elapsed() < Duration::from_secs(15) {
        let alpha = alpha_wave[i % alpha_wave.len()];
        let pkt = build_packet(0x0D, 0xff, 0x00, 0x00, alpha);
        let r = handle.write_control(0x21, 0x09, 0x0304u16, 1u16, &pkt, Duration::from_millis(500));
        if r.is_err() {
            println!("  write (alpha={alpha:02x}): {r:?}");
        }
        i += 1;
        cycles += 1;
        std::thread::sleep(Duration::from_millis(30));
    }
    println!("Done streaming: {cycles} frames over 15s.");

    println!("Done. Watch for: does the FRONT-LEFT CORNER show ANY visible effect at all -- since nothing was ever primed, the chassis should have stayed completely dark/off the entire time except for whatever this stream itself produces.");
    Ok(())
}
