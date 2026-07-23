use std::error::Error;
use std::time::Duration;

/// Tests the report-0x05 (wValue 0x0305) protocol Windows session 4
/// discovered is a real, separate, continuously-streamed animated-effects
/// mechanism -- not the one-shot "handshake" every prior test in this repo
/// treated it as. Real bytes extracted directly from
/// `usb_capture_session4/all_0305.txt` (a live Breathing-mode capture):
///
///   05 01 00 00 0f 00 [byte6] 00 [byte8] [byte9]
///
/// byte6 stays ~constant (0xff, occasionally 0xfe -- probably jitter),
/// byte8 stays 0x00, byte9 smoothly ramps 0x00->0xff->0x00 roughly every
/// ~2 seconds, sent at ~60ms intervals (~16Hz). This packet carries no
/// R/G/B at all -- whatever color it's modulating must already be set by
/// something else (never identified in the Windows capture: zero 0x0304
/// traffic during Breathing, and the priming triplet's own colour field is
/// black). This test primes (the same real sequence used for 0x04 tests),
/// then streams this exact byte9 ramp continuously, to see whether it's
/// real and observable on Linux the same way priming alone already proved
/// to trigger real hardware reactions (RainbowCycle).
///
/// VARIANT: both g615lr-0305-breathe-stream.rs (with the b3/b4/b5 priming
/// triplet) and g615lr-0305-only-stream.rs (without it) showed no
/// distinguishable effect -- neither the real Windows capture nor either
/// of those tests ever established what colour 0x0305 is actually
/// modulating (zero 0x0304 traffic during Breathing, priming triplet's own
/// colour field is black). This variant sets a REAL colour first via the
/// already-proven-working 0x5d Static sequence (b3,b5,b4 order -- the
/// order that's separately confirmed to visibly set colour, distinct from
/// the priming triplet's b3,b4,b5 order), THEN does minimal priming
/// (SET_IDLE + 0x0201 only, deliberately skipping the RainbowCycle-forcing
/// triplet this time so it can't clobber the colour just set), then the
/// 0x0305 handshake and breathing stream -- on the theory that 0x0305
/// needs an already-established colour to have anything to modulate.
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

    macro_rules! send {
        ($label:expr, $req:expr, $val:expr, $idx:expr, $data:expr) => {
            let r = handle.write_control(0x21, $req, $val, $idx, $data, Duration::from_secs(2));
            println!("{}: {:?}", $label, r);
        };
    }

    // Step 1: set a REAL colour (bright red) via the already-proven-working
    // 0x5d Static sequence, b3,b5,b4 order (matches the real captured
    // traffic from a working `asusctl aura effect static` call).
    #[rustfmt::skip]
    let color_b3: [u8; 64] = [
        0x5d, 0xb3, 0x00, 0x00, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
    ];
    let mut color_b5 = [0u8; 64];
    color_b5[0] = 0x5d;
    color_b5[1] = 0xb5;
    let mut color_b4 = [0u8; 64];
    color_b4[0] = 0x5d;
    color_b4[1] = 0xb4;
    send!("0x025d b3 (SET COLOR: red) iface0", 0x09, 0x025du16, 0u16, &color_b3);
    send!("0x025d b5 (set) iface0", 0x09, 0x025du16, 0u16, &color_b5);
    send!("0x025d b4 (apply) iface0", 0x09, 0x025du16, 0u16, &color_b4);
    std::thread::sleep(Duration::from_millis(500));

    // Step 2: minimal priming only -- SET_IDLE + 0x0201, deliberately
    // skipping the b3/b4/b5 RainbowCycle-forcing triplet so it can't
    // clobber the colour just set.
    send!("SET_IDLE iface1", 0x0a, 0x0000u16, 1u16, &[]);
    send!("SET_IDLE iface0", 0x0a, 0x0000u16, 0u16, &[]);
    send!("0x0201 (01 01) iface0", 0x09, 0x0201u16, 0u16, &[0x01, 0x01]);

    let handshake05: [u8; 10] = [0x05, 0x00, 0x08, 0x00, 0x0f, 0x00, 0x00, 0x00, 0x00, 0x01];
    send!("0x0305 (handshake) iface1", 0x09, 0x0305u16, 1u16, &handshake05);

    println!("Color set (red) + minimal priming complete. Streaming 0x0305 breathing ramp for 10 seconds...");
    println!("(same triangle-wave byte9 pattern extracted from the real Windows Breathing capture)");

    let start = std::time::Instant::now();
    let period = Duration::from_millis(2000); // ~2s full cycle, matching the real capture
    let step = Duration::from_millis(60); // ~16Hz, matching the real capture
    let mut sent = 0u32;
    while start.elapsed() < Duration::from_secs(10) {
        let elapsed_in_period =
            (start.elapsed().as_millis() % period.as_millis() as u128) as f64 / period.as_millis() as f64;
        // Triangle wave 0x00 -> 0xff -> 0x00 over one period.
        let byte9 = if elapsed_in_period < 0.5 {
            (elapsed_in_period * 2.0 * 255.0) as u8
        } else {
            ((1.0 - elapsed_in_period) * 2.0 * 255.0) as u8
        };
        let pkt: [u8; 10] = [0x05, 0x01, 0x00, 0x00, 0x0f, 0x00, 0xff, 0x00, 0x00, byte9];
        let _ = handle.write_control(0x21, 0x09, 0x0305u16, 1u16, &pkt, Duration::from_millis(500));
        sent += 1;
        std::thread::sleep(step);
    }
    println!("Sent {sent} 0x0305 packets.");

    let _ = handle.release_interface(0);
    let _ = handle.release_interface(1);
    if had0 {
        let _ = handle.attach_kernel_driver(0);
    }
    if had1 {
        let _ = handle.attach_kernel_driver(1);
    }

    println!("Done. Did anything visibly pulse/breathe, or change behavior at all?");
    Ok(())
}
