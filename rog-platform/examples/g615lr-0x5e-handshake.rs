use std::error::Error;
use std::time::Duration;

/// Replicates the EXACT "Asus handshake" the in-kernel `hid-asus.c` driver
/// performs on every probe (`asus_kbd_init()`): SET_REPORT a 16-byte
/// Feature payload (`report_id` + ASCII "ASUS Tech.Inc." + a null byte),
/// then GET_REPORT the same ID and compare -- a match means "valid", a
/// mismatch means "invalid" (source: torvalds/linux drivers/hid/hid-asus.c).
///
/// Found via `dmesg`: the kernel's own attempt at report ID `0x5e`
/// specifically has failed identically -- `ff ff ff...` instead of an
/// echo -- on EVERY single device probe this entire session (7/7,
/// spanning ~2 hours). `0x5e` is not declared in either interface's HID
/// report descriptor (dumped earlier this session), yet the device still
/// answers GET_REPORT for it with real data, just not the expected one.
///
/// This test replicates the same handshake independently of the kernel
/// driver's context/timing, on BOTH interfaces, to (a) confirm the
/// failure reproduces outside `hid_asus.c`'s own probe sequence, and (b)
/// as a base for follow-up variations (different padding, different
/// interface, different position in the init sequence) if the plain
/// replication also fails.
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

    // report_id + "ASUS Tech.Inc." + 0x00, exactly matching hid-asus.c's
    // asus_kbd_init(). 16 bytes total.
    fn handshake_payload(report_id: u8) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[0] = report_id;
        buf[1..15].copy_from_slice(b"ASUS Tech.Inc.");
        buf[15] = 0x00;
        buf
    }

    // `read_len` matters: 0x5a/0x5d's actual Feature reports are 64 bytes
    // (ReportCount 0x3f + report ID byte, per the descriptor dump earlier
    // this session) -- asking for only 16 bytes caused a real `Overflow`
    // (device has more data than the buffer can hold), NOT a meaningful
    // "does it echo" result. 0x5e's report is apparently genuinely 16
    // bytes natively (no overflow at 16), which is itself informative --
    // it doesn't reuse 0x5a/0x5d's bigger multi-purpose report format.
    fn try_handshake(
        handle: &rusb::DeviceHandle<impl rusb::UsbContext>,
        iface: u16,
        report_id: u8,
        read_len: usize,
    ) {
        let payload = handshake_payload(report_id);
        let w_value = 0x0300u16 | report_id as u16; // Feature report type = 3
        let set_r = handle.write_control(
            0x21, 0x09, w_value, iface, &payload, Duration::from_secs(2),
        );
        println!(
            "iface{iface} SET_REPORT {report_id:02x}: {set_r:?}"
        );

        let mut readbuf = vec![0u8; read_len];
        let get_r = handle.read_control(
            0xa1, 0x01, w_value, iface, &mut readbuf, Duration::from_secs(2),
        );
        let matches = readbuf.len() >= payload.len() && readbuf[..payload.len()] == payload;
        println!(
            "iface{iface} GET_REPORT {report_id:02x}: {get_r:?} buf={readbuf:02x?} MATCH(first16)={matches}"
        );
    }

    println!("--- Replicating kernel's failing 0x5e handshake, interface 0 ---");
    try_handshake(&handle, 0, 0x5e, 16);
    println!("--- Same, interface 1 (never tried by the kernel driver, which only touches iface 0) ---");
    try_handshake(&handle, 1, 0x5e, 16);

    println!("--- Control: 0x5a handshake, correct 64-byte buffer this time ---");
    try_handshake(&handle, 0, 0x5a, 64);
    println!("--- Control: 0x5d handshake, correct 64-byte buffer this time ---");
    try_handshake(&handle, 0, 0x5d, 64);

    Ok(())
}
