use std::error::Error;
use std::time::Duration;

/// Replicates the EXACT sequence captured immediately before the FIRST
/// successful 0x04 zone write in aura.pcap (the real, visually-confirmed
/// working Windows session) -- extracted directly by chronologically
/// scanning every control transfer before that first write, not
/// reconstructed from theory. See conversation: this is different from
/// every prior handshake attempt in this repo (g615lr-with-handshake.rs,
/// g615lr-iface0-handshake-replay.rs, g615lr-5d-then-04.rs) in three ways:
///
/// 1. Includes SET_IDLE (HID class request, bRequest=0x0a) on BOTH
///    interfaces first -- never sent anywhere else in this repo.
/// 2. The 0x5d priming packet is the "always-identical" one the original
///    investigation called dead/vestigial (`5d b3 00 02 00 00 00 eb...`,
///    mode=RainbowCycle, colour=black) -- turns out it's sent before EVERY
///    lighting operation as routine priming, not something that varies.
/// 3. Uses the real b3,b4,b5 ORDER as captured (not b3,b5,b4, which is
///    what write_effect_and_apply / all our prior tests used).
///
/// Full captured sequence, wire-exact:
///   SET_IDLE iface 1
///   SET_IDLE iface 0
///   SET_REPORT 0x0201 "01 01"                    iface 0
///   SET_REPORT 0x025d "5d b3 00 02 00 00 00 eb"   iface 0 (padded 64B)
///   SET_REPORT 0x025d "5d b4 00"                  iface 0 (padded 64B)
///   SET_REPORT 0x025d "5d b5 00"                  iface 0 (padded 64B)
///   SET_REPORT 0x0305 "05 00 08 00 0f 00 00 00 00 01"  iface 1
///   SET_REPORT 0x0304 <zone data>                 iface 1  <- the real write
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

    send!("SET_IDLE iface1", 0x0a, 0x0000u16, 1u16, &[]);
    send!("SET_IDLE iface0", 0x0a, 0x0000u16, 0u16, &[]);

    send!("0x0201 (01 01) iface0", 0x09, 0x0201u16, 0u16, &[0x01, 0x01]);

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

    // The real write: zone 0x06 (back-left corner), bright red.
    #[rustfmt::skip]
    let zone_packet: [u8; 51] = [
        0x04, 0x01, 0x01, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0xff, 0x00, 0x00, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    send!("0x0304 (ZONE COLOR) iface1", 0x09, 0x0304u16, 1u16, &zone_packet);

    let _ = handle.release_interface(0);
    let _ = handle.release_interface(1);
    if had0 {
        let _ = handle.attach_kernel_driver(0);
    }
    if had1 {
        let _ = handle.attach_kernel_driver(1);
    }

    println!("Done. Check the back-left corner for red.");
    Ok(())
}
