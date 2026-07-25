use std::error::Error;
use std::time::Duration;

use rog_aura::keyboard::LedUsbPackets;

/// Direct follow-up to the `g615lr-perkey-zoned-protocol.rs` breakthrough:
/// that test proved `5d bc 00...` (`LedUsbPackets::get_init_msg()`, the
/// real "switch EC from builtin to custom mode" command) is genuine and
/// works on this hardware -- keyboard zones lit up for the first time all
/// session. Never sent once before that, in any `0x04` test.
///
/// New hypothesis: `0x04` may share the same prerequisite -- the EC needs
/// to be switched into custom/host-controlled mode before EITHER the
/// `0x5d bc` zoned protocol OR the `0x04` lightbar protocol will actually
/// take visible effect. Every `0x04` test tonight used either no init at
/// all, or the *builtin*-mode `b3/b4/b5` priming (which triggers
/// RainbowCycle, a builtin effect -- the opposite of custom mode). This
/// specific combination -- real custom-mode init, then `0x04` -- has never
/// been tried.
///
/// Sequence: dark reset -> real custom-mode init (`5d bc`) -> 0x0305
/// handshake -> stream the same wire-verified-correct `0x04` zone packet
/// (front-left corner, ramping alpha) used in every prior test tonight.
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

    // THE NEW STEP: real custom-mode init, proven live minutes ago to
    // actually do something real on this hardware.
    let init_msg = LedUsbPackets::get_init_msg();
    send!("0x025d bc (CUSTOM MODE INIT) iface0", 0x09, 0x025du16, 0u16, &init_msg);
    println!("Custom-mode init sent. Waiting 1s...");
    std::thread::sleep(Duration::from_secs(1));

    let handshake05: [u8; 10] = [0x05, 0x00, 0x08, 0x00, 0x0f, 0x00, 0x00, 0x00, 0x00, 0x01];
    send!("0x0305 (handshake) iface1", 0x09, 0x0305u16, 1u16, &handshake05);

    println!("Streaming front-left corner (zone 0x0D) with ramping alpha for 15s, over a real custom-mode-initialised EC state...");

    let alpha_wave: [u8; 14] = [
        0x06, 0x18, 0x35, 0x58, 0x80, 0xa7, 0xcb, 0xe7, 0xfb, 0xff, 0xf5, 0xe0, 0xc2, 0x9d,
    ];

    fn build_packet(zone_id: u8, r: u8, g: u8, b: u8, a: u8) -> [u8; 51] {
        let mut pkt = [0u8; 51];
        pkt[0] = 0x04;
        pkt[1] = 0x01;
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

    println!("Done. Watch the FRONT-LEFT CORNER for any effect at all.");
    Ok(())
}
