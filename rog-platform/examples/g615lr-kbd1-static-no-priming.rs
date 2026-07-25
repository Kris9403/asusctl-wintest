use std::error::Error;
use std::time::Duration;

/// Companion to `g615lr-corner-no-priming.rs`, run in parallel to test the
/// opposite corner of the same variable space: keyboard zone 1 (wire ID
/// `0x00`, `Keyboard1`) with a PLAIN STATIC colour (constant alpha=0xFF,
/// never changing -- the classic single-shot write every prior test this
/// whole investigation used) instead of the ramping-alpha animated
/// approach, and -- like the corner test -- with NO `0x5d` priming
/// triplet at all, asusd fully stopped, nothing else touching the device.
///
/// Together with `g615lr-corner-no-priming.rs` this covers all four
/// combinations tried today: {kbd zone, lightbar corner} x {ramping
/// alpha, static constant}, all with the confound of an active classic-
/// protocol animation engine fully removed for the first time. If EITHER
/// this or the corner test shows anything, that's the clearest signal
/// yet about which variable (zone type, or alpha animation) actually
/// matters. If BOTH still show nothing, that's strong evidence the
/// remaining gap has nothing to do with priming/animation-engine state
/// at all.
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

    // Explicit dark reset (real Static black, zone=None, real b3,b5,b4
    // SET/APPLY order) -- not the priming order, never triggers
    // animation.
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

    // Deliberately NO 0x5d b3/b4/b5 priming.
    let handshake05: [u8; 10] = [0x05, 0x00, 0x08, 0x00, 0x0f, 0x00, 0x00, 0x00, 0x00, 0x01];
    send!("0x0305 (handshake) iface1", 0x09, 0x0305u16, 1u16, &handshake05);

    println!("No priming sent. Streaming kbd1 (zone 0x00, Keyboard1) with a CONSTANT static green (alpha always 0xFF, never changing) for 15s...");

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

    // Constant green, alpha always 0xFF -- identical packet every write,
    // matching every prior negative test this session, but this time
    // with the animation-engine confound fully removed.
    let pkt = build_packet(0x00, 0x00, 0xff, 0x00, 0xff);

    let start = std::time::Instant::now();
    let mut cycles = 0u32;
    while start.elapsed() < Duration::from_secs(15) {
        let r = handle.write_control(0x21, 0x09, 0x0304u16, 1u16, &pkt, Duration::from_millis(500));
        if r.is_err() {
            println!("  write: {r:?}");
        }
        cycles += 1;
        std::thread::sleep(Duration::from_millis(200));
    }
    println!("Done streaming: {cycles} identical writes over 15s.");

    println!("Done. Watch for: does kbd1 show ANY green at all -- since nothing was ever primed, the chassis should have stayed completely dark the entire time except for whatever this write itself produces.");
    Ok(())
}
