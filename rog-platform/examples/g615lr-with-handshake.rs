use std::error::Error;
use std::time::Duration;

/// Same raw-USB approach as g615lr-raw-usb-test, but first sends a
/// previously-undocumented Feature report ID 0x05 (10 bytes), captured
/// (aura.pcap) immediately preceding the very first 0x04 zone packet of
/// that session -- theory: a one-time "enable custom lighting" handshake
/// Armoury Crate sends once per app launch, which our earlier tests never
/// sent, possibly explaining why 0x04 alone produced no visible effect.
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
    let iface: u8 = 1;

    let had_kernel_driver = handle.kernel_driver_active(iface).unwrap_or(false);
    if had_kernel_driver {
        handle.detach_kernel_driver(iface)?;
    }
    handle.claim_interface(iface)?;

    // Captured handshake packet, report ID 0x05, from aura.pcap.
    let handshake: [u8; 10] = [0x05, 0x00, 0x08, 0x00, 0x0f, 0x00, 0x00, 0x00, 0x00, 0x01];

    #[rustfmt::skip]
    let zone_packet: [u8; 51] = [
        0x04, 0x01, 0x01, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0xff, 0x00, 0x00, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    let r1 = handle.write_control(0x21, 0x09, 0x0305, iface as u16, &handshake, Duration::from_secs(2));
    println!("handshake (report 0x05): {r1:?}");

    std::thread::sleep(Duration::from_millis(50));

    let r2 = handle.write_control(0x21, 0x09, 0x0304, iface as u16, &zone_packet, Duration::from_secs(2));
    println!("zone packet (report 0x04): {r2:?}");

    let _ = handle.release_interface(iface);
    if had_kernel_driver {
        let _ = handle.attach_kernel_driver(iface);
    }

    println!("Sent handshake + zone 0x06 red. Check the hardware now.");
    Ok(())
}
