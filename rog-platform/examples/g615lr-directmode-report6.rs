use std::error::Error;
use std::time::Duration;

/// New lead, from a real HID report-descriptor dump of both interfaces
/// (`/sys/bus/hid/devices/0003:0B05:19B6.000{6,7}/report_descriptor`),
/// prompted by asus-linux maintainer "NeroReflex" describing an
/// undiscovered "go to direct mode" command distinct from the `0x5a`/`0x5e`/
/// `0x5d` identification handshake. Their claim of "3 HID devices, only the
/// vendor one accepts 0x04" does NOT hold for this exact laptop/firmware --
/// `lsusb` and this descriptor dump both agree there are only 2 HID
/// devices/interfaces here, matching what we've used all along. BUT the
/// full descriptor of interface 1 (the vendor collection, Usage Page
/// 0x59, the one carrying `0x04`'s zone-colour report) revealed something
/// never tried before: Report ID `0x06`, a single-byte Feature report
/// with LogicalMin=0/LogicalMax=1 (i.e. a boolean), sitting in its own
/// tiny sub-collection right next to `0x04`'s zone data and `0x05`'s
/// smaller 4-zone variant:
///
/// ```text
/// ReportID = 0x6
/// Usage = 0x70
///   Usage = 0x71, LogicalMin=0, LogicalMax=1, ReportSize=8, ReportCount=1, Feature
/// ```
///
/// That shape -- tiny, boolean-ranged, its own report ID, colocated with
/// the zone-write reports -- is exactly what a "direct/manual mode"
/// enable toggle looks like. Never sent in any prior test. This is a
/// single, minimal hypothesis test per systematic-debugging: GET_FEATURE
/// report 6 first (see the real current value/length), SET_FEATURE it to
/// 1, GET_FEATURE again to confirm the write landed, then stream the
/// known-good `0x04` zone packets on a clean dark baseline (deliberately
/// NOT priming RainbowCycle first, to avoid the already-nailed-down
/// animation-overwrite confound from `g615lr-cancel-rainbow-then-04.rs`).
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

    // GET_FEATURE report 6, before touching it: bmRequestType 0xA1
    // (Device-to-Host | Class | Interface), bRequest 0x01 (GET_REPORT),
    // wValue = (reportType<<8)|reportID = 0x0306 (Feature=3, ID=6).
    let mut buf = [0u8; 8];
    let r = handle.read_control(0xa1, 0x01, 0x0306u16, 1u16, &mut buf, Duration::from_secs(2));
    println!("GET_FEATURE report 6 (before): {:?} buf={:02x?}", r, buf);

    // SET_FEATURE report 6 = 1 (hypothesis: enable direct/manual mode).
    send!("SET_FEATURE report 6 = 01 iface1", 0x09, 0x0306u16, 1u16, &[0x06, 0x01]);

    let mut buf2 = [0u8; 8];
    let r2 = handle.read_control(0xa1, 0x01, 0x0306u16, 1u16, &mut buf2, Duration::from_secs(2));
    println!("GET_FEATURE report 6 (after): {:?} buf={:02x?}", r2, buf2);

    println!("Direct-mode toggle attempted. Waiting 1s (deliberately NOT priming RainbowCycle -- clean dark baseline)...");
    std::thread::sleep(Duration::from_secs(1));

    let handshake05: [u8; 10] = [0x05, 0x00, 0x08, 0x00, 0x0f, 0x00, 0x00, 0x00, 0x00, 0x01];
    send!("0x0305 (handshake) iface1", 0x09, 0x0305u16, 1u16, &handshake05);

    // Literal bytes, extracted via tshark from multizone_12x_confirmed.pcapng's
    // "real colour" pass -- same known-good packets every prior 0x04 test used.
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

    let decoded: Vec<(&str, Vec<u8>)> = packets
        .iter()
        .map(|(label, hex)| {
            let bytes: Vec<u8> = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect();
            assert_eq!(bytes.len(), 51, "packet for {label} should be 51 bytes");
            (*label, bytes)
        })
        .collect();

    println!("Streaming all 16 literal packets for 10 seconds over the direct-mode-toggled clean baseline...");
    let start = std::time::Instant::now();
    let mut cycles = 0u32;
    while start.elapsed() < Duration::from_secs(10) {
        for (label, bytes) in &decoded {
            let r = handle.write_control(0x21, 0x09, 0x0304u16, 1u16, bytes, Duration::from_millis(500));
            if r.is_err() {
                println!("  {label}: {r:?}");
            }
        }
        cycles += 1;
    }
    println!("Done streaming: {cycles} full 16-packet cycles.");

    let _ = handle.release_interface(0);
    let _ = handle.release_interface(1);
    if had0 {
        let _ = handle.attach_kernel_driver(0);
    }
    if had1 {
        let _ = handle.attach_kernel_driver(1);
    }

    println!("Done. Expected if hypothesis correct: kbd1=red kbd2=green kbd3=blue kbd4=white, back_left=red back_right=green back_corner_left=blue back_corner_right=yellow, all 4 side zones=black, front_corner_right=orange front_corner_left=white front_bar_right=cyan front_bar_left=magenta.");
    Ok(())
}
