use std::error::Error;
use std::time::Duration;

/// THE decisive test both Windows and Linux independently converged on:
/// a genuinely clean session (fresh reboot, zero prior `0x04` exposure
/// this boot) with `count=1` targeting ONLY `back_right` (wire ID
/// `0x04`), matching Windows session 9's exact successful colour
/// (`0x61,0xff,0x00,0xff`) and real `b3/b4/b5` RainbowCycle priming.
///
/// This is the ONE test that actually distinguishes the two live
/// hypotheses from Windows' own contradictory count=1 isolation runs:
/// - If this lights up on the very first try: `count>1` was never the
///   real variable. Every prior negative result (both OSes) was
///   something else -- carried-over state, or just needed more time/
///   repetitions to register.
/// - If it stays dark: strengthens the `count>1`-requirement theory,
///   though still wouldn't fully rule out a "needs N repetitions"
///   explanation without further controlled runs.
///
/// Streams continuously for 20s (no Aura-style 0x0305 hold mechanism of
/// our own), matching every prior successful reproduction pattern.
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

    // Real priming triplet, b3,b4,b5 order, mode=0x02 RainbowCycle --
    // matching Windows session 9's exact successful conditions.
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

    // count=1, ONLY zone 0x04 (back_right), same colour Windows used.
    let mut pkt = [0u8; 51];
    pkt[0] = 0x04;
    pkt[1] = 1; // count = 1
    pkt[2] = 0x01;
    pkt[3] = 0x04; // zone id: back_right
    pkt[4] = 0x00;
    pkt[19] = 0x61;
    pkt[20] = 0xff;
    pkt[21] = 0x00;
    pkt[22] = 0xff;

    println!("Packet bytes: {:02x?}", pkt);
    println!("CLEAN BOOT, first-ever touch of zone 0x04 this session. Streaming count=1 back_right for 20s...");

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
