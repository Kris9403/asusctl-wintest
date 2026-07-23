use std::error::Error;
use std::time::Duration;

/// Resends the same static zone-0x06-red packet in a tight loop for ~6
/// seconds, mimicking Armoury Crate's continuous streaming, on the theory
/// that a one-shot packet gets overwritten by the next frame of whatever
/// free-running default animation is currently driving the visible
/// rainbow effect (there's no Armoury Crate on Linux to have put it there,
/// so it's most likely a firmware idle default, not host-driven -- but it
/// still needs to be "outrun" by continuous writes to be visible).
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

    #[rustfmt::skip]
    let zone_packet: [u8; 51] = [
        0x04, 0x01, 0x01, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0xff, 0x00, 0x00, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    println!("Streaming zone 0x06 = red for 6 seconds. Watch the back-left corner now...");
    let start = std::time::Instant::now();
    let mut sent = 0u32;
    let mut errors = 0u32;
    while start.elapsed() < Duration::from_secs(6) {
        match handle.write_control(0x21, 0x09, 0x0304, iface as u16, &zone_packet, Duration::from_millis(500)) {
            Ok(_) => sent += 1,
            Err(_) => errors += 1,
        }
        std::thread::sleep(Duration::from_millis(20)); // ~50fps
    }
    println!("Done: {sent} packets sent, {errors} errors.");

    let _ = handle.release_interface(iface);
    if had_kernel_driver {
        let _ = handle.attach_kernel_driver(iface);
    }
    Ok(())
}
