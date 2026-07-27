use std::error::Error;
use std::time::Duration;

/// Read-only check: does this device advertise a Microsoft OS 1.0
/// descriptor (the legacy, pre-BOS mechanism -- this device's bcdUSB is
/// 2.00, too old for the modern BOS-based MS OS 2.0 descriptor, which
/// requires bcdUSB >= 2.01)? If it does, Windows automatically queries
/// this during enumeration and may configure vendor-specific driver
/// behaviour based on it that Linux's generic HID stack never touches --
/// a potential explanation for a persistent, byte-content-independent
/// platform difference in how the firmware responds to 0x04, given every
/// other variable (count, priming, clean boot, attached/detached driver)
/// has now been controlled for without changing the outcome.
///
/// Pure GET_DESCRIPTOR(STRING, index=0xEE) query -- standard USB request,
/// completely read-only and safe, no state change possible.
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

    // GET_DESCRIPTOR, bmRequestType=0x80 (device-to-host, standard, device),
    // bRequest=0x06, wValue=(STRING<<8)|0xEE, try both wIndex=0x0000 and
    // 0x0409 (US English) since implementations vary on which they expect.
    for w_index in [0x0000u16, 0x0409u16] {
        let mut buf = [0u8; 255];
        let r = handle.read_control(0x80, 0x06, 0x03EEu16, w_index, &mut buf, Duration::from_secs(2));
        match r {
            Ok(n) => {
                println!("wIndex={w_index:#06x}: got {n} bytes: {:02x?}", &buf[..n]);
                if n >= 16 {
                    let sig = &buf[2..16];
                    let sig_str: String = sig
                        .chunks(2)
                        .filter_map(|c| if c[0] != 0 { Some(c[0] as char) } else { None })
                        .collect();
                    println!("  Decoded signature bytes as text: {sig_str:?}");
                    if sig_str.contains("MSFT100") {
                        println!("  *** REAL MS OS DESCRIPTOR FOUND -- signature matches MSFT100! ***");
                        println!("  bMS_VendorCode = {:#04x}", buf[16]);
                    }
                }
            }
            Err(e) => println!("wIndex={w_index:#06x}: {e:?} (no MS OS descriptor at this index/context)"),
        }
    }

    Ok(())
}
