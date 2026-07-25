use std::error::Error;
use std::time::Duration;

/// New hypothesis, from FRESH real Aura Creator capture data pushed by
/// Windows session this session (`25/usb_data.txt`, `25/123.xml`): every
/// prior `0x04` test this entire investigation sent byte-for-byte
/// IDENTICAL packets on every repeated write (same zone, same colour,
/// same alpha=0xFF every time). The real capture NEVER does this -- every
/// single frame of a real Aura Creator animation, even for a nominally
/// "solid colour" phase, has a continuously CHANGING 4th colour byte
/// (alpha), e.g. a clean isolated 2-zone segment (wire IDs 0x06/0x0C)
/// with R,G,B constant at ff,00,00 but alpha ramping smoothly
/// 06->18->35->58->80->a7->cb->e7->fb->ff->f5->e0->c2->9d (a triangle
/// wave) across ~14 consecutive packets. This pattern repeats for dozens
/// of different zone groups throughout the capture -- every real
/// animation frame differs from the previous one.
///
/// New hypothesis: this firmware may only redraw on an actual VALUE
/// CHANGE in the Feature report, silently no-op'ing an exact repeat of
/// the last write. That would explain every prior negative result at
/// once -- we always sent identical repeated packets. Real Windows
/// traffic never does.
///
/// Test: continuously stream a SINGLE zone (kbd3, 0x02, chosen since it's
/// already confirmed cleanly isolated with no lightbar bleed on the
/// classic protocol) with alpha genuinely ramping every write (not the
/// same value twice), matching the real capture's cadence and waveform,
/// for 15s. Watch for ANY visible, persistent effect -- even a visible
/// brightness pulse would be a first, since every prior streaming test
/// (including 40s continuous ones) showed literally nothing against a
/// non-animating baseline.
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

    // Real priming triplet (b3,b4,b5 order), matching the real capture's
    // preceding sequence exactly.
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
    send!("0x025d b3 (prime) iface0", 0x09, 0x025du16, 0u16, &b3_prime);
    send!("0x025d b4 (prime) iface0", 0x09, 0x025du16, 0u16, &b4_prime);
    send!("0x025d b5 (prime) iface0", 0x09, 0x025du16, 0u16, &b5_prime);

    let handshake05: [u8; 10] = [0x05, 0x00, 0x08, 0x00, 0x0f, 0x00, 0x00, 0x00, 0x00, 0x01];
    send!("0x0305 (handshake) iface1", 0x09, 0x0305u16, 1u16, &handshake05);

    println!("Streaming kbd3 (zone 0x02) with a genuinely ramping alpha (triangle wave, matching the real capture's exact waveform) for 15s...");

    // Same triangle-wave alpha sequence observed in the real capture
    // (0x06->0x18->0x35->0x58->0x80->0xa7->0xcb->0xe7->0xfb->0xff->0xf5->0xe0->0xc2->0x9d),
    // looped continuously. R,G,B held constant at red (ff,00,00), matching
    // the real capture's own constant-colour-changing-alpha pattern.
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
        let pkt = build_packet(0x02, 0xff, 0x00, 0x00, alpha);
        let r = handle.write_control(0x21, 0x09, 0x0304u16, 1u16, &pkt, Duration::from_millis(500));
        if r.is_err() {
            println!("  write (alpha={alpha:02x}): {r:?}");
        }
        i += 1;
        cycles += 1;
        // ~30ms between frames, roughly matching the real capture's cadence.
        std::thread::sleep(Duration::from_millis(30));
    }
    println!("Done streaming: {cycles} frames over 15s.");

    println!("Done. Watch for: does kbd3's key area show ANY visible effect -- a red pulse/breathing, even a flicker -- distinct from total inertness?");
    Ok(())
}
