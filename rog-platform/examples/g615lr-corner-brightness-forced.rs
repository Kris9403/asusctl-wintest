use std::error::Error;
use std::time::Duration;

/// Direct retest of `g615lr-corner-no-priming.rs` (the wire-verified
/// negative result), after the user raised a real methodological
/// concern: keyboard/lightbar brightness might have been sitting at 0
/// during some of tonight's testing, which would make ANY test show
/// "nothing" regardless of whether the colour data landed correctly.
/// Checked directly: `/sys/class/leds/asus::kbd_backlight/brightness`
/// currently reads `3` (max), not 0 -- but forcing it explicitly here
/// removes all doubt for this specific retest.
///
/// This is the standard Linux LED class device (confirmed in asusd's own
/// boot log: "Found keyboard LED controls at asus::kbd_backlight"),
/// entirely independent of the USB HID protocol -- a plain sysfs write,
/// safe, reversible, doesn't touch the device claim/interfaces at all.
///
/// Otherwise identical to `g615lr-corner-no-priming.rs`: no priming, real
/// dark reset, real custom-mode init skipped (this is the classic-
/// protocol-free path, matching the original wire-verified test exactly),
/// streams the front-left corner (zone 0x0D) with the same real ramping-
/// alpha waveform for 15s.
fn main() -> Result<(), Box<dyn Error>> {
    // Force keyboard backlight brightness to max BEFORE anything else --
    // plain sysfs, independent of USB/asusd.
    let max_path = "/sys/class/leds/asus::kbd_backlight/max_brightness";
    let bright_path = "/sys/class/leds/asus::kbd_backlight/brightness";
    let before = std::fs::read_to_string(bright_path).unwrap_or_default();
    let max_val = std::fs::read_to_string(max_path).unwrap_or_else(|_| "3".to_string());
    let max_val = max_val.trim();
    println!("Brightness before: {}", before.trim());
    std::fs::write(bright_path, max_val)?;
    let after = std::fs::read_to_string(bright_path).unwrap_or_default();
    println!("Forced brightness to max ({max_val}). Now reads: {}", after.trim());

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
    send!("0x025d b3 (dark reset, Static black) iface0", 0x09, 0x025du16, 0u16, &dark_b3);
    send!("0x025d b5 (set) iface0", 0x09, 0x025du16, 0u16, &dark_b5);
    send!("0x025d b4 (apply) iface0", 0x09, 0x025du16, 0u16, &dark_b4);
    println!("Dark reset sent -- ALL zones should now be off. Waiting 2s to confirm dark...");
    std::thread::sleep(Duration::from_secs(2));

    let handshake05: [u8; 10] = [0x05, 0x00, 0x08, 0x00, 0x0f, 0x00, 0x00, 0x00, 0x00, 0x01];
    send!("0x0305 (handshake) iface1", 0x09, 0x0305u16, 1u16, &handshake05);

    println!("Brightness forced to max, no priming. Streaming front-left corner (zone 0x0D) with ramping alpha for 15s...");

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

    println!("Done. Brightness was forced to max for this entire run. Watch for: does the FRONT-LEFT CORNER show ANY visible effect at all?");
    Ok(())
}
