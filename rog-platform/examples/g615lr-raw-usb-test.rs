use std::error::Error;
use std::time::Duration;

/// Bypasses the Linux kernel HID subsystem entirely (and with it, the
/// `hid_asus` driver bound to this device -- see `lsusb`/`/sys/bus/hid`,
/// which shows driver "asus", not generic "hid-generic") by detaching the
/// kernel driver from interface 1 (MI_01) and sending a raw USB control
/// transfer directly via libusb, matching Windows' `HidD_SetFeature`
/// exactly at the wire level: bmRequestType=0x21, bRequest=0x09
/// (SET_REPORT), wValue=0x0304 (Feature, ReportID 4), wIndex=1.
///
/// Rationale: `HIDIOCSFEATURE` via /dev/hidraw2 reports success (51/51
/// bytes) but produces no visible hardware effect, even replaying an exact
/// captured-good packet. If `hid_asus`'s raw_request handler is
/// intercepting/no-op'ing report ID 0x04 (which it has no built-in
/// knowledge of) before it reaches the wire, going around the HID
/// subsystem entirely should still work, since libusb talks straight to
/// the USB core.
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

    let iface: u8 = 1; // MI_01, per docs and udevadm bInterfaceNumber=01

    let had_kernel_driver = handle.kernel_driver_active(iface).unwrap_or(false);
    if had_kernel_driver {
        println!("Detaching kernel driver (hid_asus) from interface {iface}...");
        handle.detach_kernel_driver(iface)?;
    }

    handle.claim_interface(iface)?;

    #[rustfmt::skip]
    let captured_packet: [u8; 51] = [
        0x04, 0x01, 0x01, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0xff, 0x00, 0x00, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    let result = handle.write_control(
        0x21,   // bmRequestType: host->device, class, interface
        0x09,   // bRequest: SET_REPORT
        0x0304, // wValue: Feature(3) << 8 | ReportID(4)
        iface as u16,
        &captured_packet,
        Duration::from_secs(2),
    );

    // Best-effort cleanup regardless of transfer outcome.
    let _ = handle.release_interface(iface);
    if had_kernel_driver {
        let _ = handle.attach_kernel_driver(iface);
    }

    let n = result?;
    println!("Raw USB control transfer: {n} bytes accepted. Check the hardware now.");
    Ok(())
}
