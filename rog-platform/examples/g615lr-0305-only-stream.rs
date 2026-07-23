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
/// VARIANT: g615lr-0305-breathe-stream.rs (priming + stream) produced no
/// visibly different result from priming alone -- chassis just went
/// RainbowCycle, same as every other priming test, no distinguishable
/// breathing/pulsing on top. Since the b3/b4/b5 triplet is now known
/// (Windows session 4) to ALWAYS force mode=0x02 (RainbowCycle) regardless
/// of what's actually being switched to, that dominant animation may be
/// masking anything 0x0305 is doing. This variant skips the b3/b4/b5
/// triplet entirely -- just SET_IDLE + 0x0201, then straight into the
/// 0x0305 stream -- to isolate its effect against a plain static baseline
/// instead of a competing rainbow animation.
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

    // Minimal priming ONLY -- deliberately skipping the b3/b4/b5 triplet
    // this time, since it's known to force RainbowCycle mode regardless of
    // target, which may be masking whatever 0x0305 alone does.
    send!("SET_IDLE iface1", 0x0a, 0x0000u16, 1u16, &[]);
    send!("SET_IDLE iface0", 0x0a, 0x0000u16, 0u16, &[]);
    send!("0x0201 (01 01) iface0", 0x09, 0x0201u16, 0u16, &[0x01, 0x01]);

    println!("Minimal priming complete (no b3/b4/b5). Streaming 0x0305 breathing ramp for 10 seconds...");
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
