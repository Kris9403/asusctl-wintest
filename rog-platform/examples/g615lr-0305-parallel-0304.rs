use std::error::Error;
use std::time::Duration;

/// Tests Windows session 4's question 2 (QUESTIONS.md): does keeping
/// 0x0305 continuously streaming IN PARALLEL with 0x04 zone writes (not a
/// one-shot handshake) change whether 0x04 finally sticks?
///
/// Worth noting going in: real Windows captures never actually show this
/// combination. `aura.pcap` (successful 0x04 zone-painting) sends 0x0305
/// exactly once, then only 0x0304 traffic. `breathing_mode_capture.pcapng`
/// (animated effects) sends ONLY 0x0305, zero 0x0304, for the whole
/// session. The two mechanisms appear mutually exclusive on Windows, which
/// argues against this theory -- but it's cheap to test and was explicitly
/// requested, so tried anyway before ruling it out.
///
/// Real priming (SET_IDLE x2, 0x0201, b3/b4/b5 triplet), then interleaves
/// 0x0305 (steady handshake-style bytes, not the breathing ramp -- this
/// isn't about animation, it's about whether keeping 0x0305 "alive" is a
/// prerequisite the EC checks for) with continuous 0x04 zone-0x06 red
/// writes, for 10 seconds.
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

    println!("Priming complete. Interleaving 0x0305 + 0x04 zone writes for 10 seconds...");

    let handshake05: [u8; 10] = [0x05, 0x00, 0x08, 0x00, 0x0f, 0x00, 0x00, 0x00, 0x00, 0x01];
    #[rustfmt::skip]
    let zone_packet: [u8; 51] = [
        0x04, 0x01, 0x01, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0xff, 0x00, 0x00, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    let start = std::time::Instant::now();
    let mut sent_0305 = 0u32;
    let mut sent_0304 = 0u32;
    while start.elapsed() < Duration::from_secs(10) {
        let _ = handle.write_control(0x21, 0x09, 0x0305u16, 1u16, &handshake05, Duration::from_millis(500));
        sent_0305 += 1;
        std::thread::sleep(Duration::from_millis(30));
        let _ = handle.write_control(0x21, 0x09, 0x0304u16, 1u16, &zone_packet, Duration::from_millis(500));
        sent_0304 += 1;
        std::thread::sleep(Duration::from_millis(30));
    }
    println!("Sent {sent_0305} x 0x0305 and {sent_0304} x 0x0304, interleaved.");

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
