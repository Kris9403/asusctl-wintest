use std::error::Error;
use std::path::Path;

use rog_platform::hid_raw::HidRaw;

/// Replays the EXACT bytes of a real, visually-confirmed-working packet
/// captured from Armoury Crate on Windows (aura.pcap, 2nd `wValue=0x0304`
/// SET_REPORT hit) -- zone 0x06 (back-left corner), color bytes ff 00 00 ff.
/// Not a re-derivation from the documented format, the literal wire bytes.
/// If this produces no effect either, the problem is not in packet
/// construction -- it's the Linux transport path or something environmental.
fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let devnode = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/dev/hidraw2".into());
    let hid = HidRaw::from_devnode(Path::new(&devnode), "19b6")?;

    #[rustfmt::skip]
    let captured_packet: [u8; 51] = [
        0x04, 0x01, 0x01, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0xff, 0x00, 0x00, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    hid.set_feature_report(&captured_packet)?;
    println!(
        "Replayed captured packet (zone 0x06, back-left corner) via {devnode}. Check the hardware now."
    );
    Ok(())
}
