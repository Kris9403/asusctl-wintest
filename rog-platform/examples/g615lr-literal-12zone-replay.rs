use std::error::Error;
use std::time::Duration;

/// Replays the LITERAL bytes extracted from
/// `usb_capture_session4/multizone_12x_confirmed.pcapng` via `tshark`
/// (not our own packet builder's re-derived output -- these are the
/// actual captured bytes from the real, human-confirmed-correct session,
/// pulled straight from the file). All 16 single-zone packets are sent in
/// sequence after real priming; since this protocol only touches the
/// zone(s) named in each packet and leaves others alone, sending all 16
/// leaves all 16 simultaneously lit -- 12 real colours + 4 explicit black
/// -- matching exactly what was visually confirmed correct on Windows,
/// twice.
///
/// This is one step more rigorous than g615lr-8zone-batch.rs (which used
/// our own builder to construct equivalent-content packets) -- this test
/// removes even the possibility that our packet construction logic
/// differs subtly from the real thing, since every byte here is copied
/// directly from the real capture file, not regenerated.
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

    // Literal bytes, extracted via tshark from multizone_12x_confirmed.pcapng's
    // "real colour" pass (the second set of 16 writes in that capture).
    // (zone_id, hex bytes, colour description) -- for reference only, the
    // hex is what actually gets sent.
    let packets: [(&str, &str); 16] = [
        ("0x00 kbd1 red",              "04010100000000000000000000000000000000ff0000ff00000000000000000000000000000000000000000000000000000000"),
        ("0x01 kbd2 green",            "0401010100000000000000000000000000000000ff00ff00000000000000000000000000000000000000000000000000000000"),
        ("0x02 kbd3 blue",             "040101020000000000000000000000000000000000ffff00000000000000000000000000000000000000000000000000000000"),
        ("0x03 kbd4 white",            "04010103000000000000000000000000000000ffffffff00000000000000000000000000000000000000000000000000000000"),
        ("0x05 back_left red",         "04010105000000000000000000000000000000ff0000ff00000000000000000000000000000000000000000000000000000000"),
        ("0x04 back_right green",      "0401010400000000000000000000000000000000ff00ff00000000000000000000000000000000000000000000000000000000"),
        ("0x07 back_corner_left blue", "040101070000000000000000000000000000000000ffff00000000000000000000000000000000000000000000000000000000"),
        ("0x06 back_corner_right yel", "04010106000000000000000000000000000000ffff00ff00000000000000000000000000000000000000000000000000000000"),
        ("0x08 right_bar_back black",  "04010108000000000000000000000000000000000000ff00000000000000000000000000000000000000000000000000000000"),
        ("0x09 left_bar_back black",   "04010109000000000000000000000000000000000000ff00000000000000000000000000000000000000000000000000000000"),
        ("0x0a right_bar_front black", "0401010a000000000000000000000000000000000000ff00000000000000000000000000000000000000000000000000000000"),
        ("0x0b left_bar_front black",  "0401010b000000000000000000000000000000000000ff00000000000000000000000000000000000000000000000000000000"),
        ("0x0c front_corner_r orange", "0401010c000000000000000000000000000000ff8000ff00000000000000000000000000000000000000000000000000000000"),
        ("0x0d front_corner_l white",  "0401010d000000000000000000000000000000ffffffff00000000000000000000000000000000000000000000000000000000"),
        ("0x0e front_bar_r cyan",      "0401010e00000000000000000000000000000000ffffff00000000000000000000000000000000000000000000000000000000"),
        ("0x0f front_bar_l magenta",   "0401010f000000000000000000000000000000ff00ffff00000000000000000000000000000000000000000000000000000000"),
    ];

    println!("Sending all 16 literal captured packets...");
    for (label, hex) in packets {
        let bytes: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        assert_eq!(bytes.len(), 51, "packet for {label} should be 51 bytes");
        let r = handle.write_control(0x21, 0x09, 0x0304u16, 1u16, &bytes, Duration::from_millis(500));
        println!("  {label}: {r:?}");
        std::thread::sleep(Duration::from_millis(100));
    }

    println!("All 16 sent. Holding for 15 seconds so all zones can be checked simultaneously...");
    std::thread::sleep(Duration::from_secs(15));

    let _ = handle.release_interface(0);
    let _ = handle.release_interface(1);
    if had0 {
        let _ = handle.attach_kernel_driver(0);
    }
    if had1 {
        let _ = handle.attach_kernel_driver(1);
    }

    println!("Done. Expected: kbd1=red kbd2=green kbd3=blue kbd4=white, back_left=red back_right=green back_corner_left=blue back_corner_right=yellow, all 4 side zones=black, front_corner_right=orange front_corner_left=white front_bar_right=cyan front_bar_left=magenta.");
    Ok(())
}
