"""
Decode the byte layout of multi-zone (count>1) 0x04 SET_REPORT writes,
using real captured Aura Creator / Aura traffic as ground truth instead
of guessing by eye.

Method: Aura Creator streams each active "layer" (a fixed set of zones)
repeatedly while animating it (alpha/colour ramps smoothly frame to
frame -- already established in HANDOFF.md). So: group consecutive real
0x04 writes by their exact zone-ID list (same batch = same layer), then
for every candidate byte position in the packet, measure how *smoothly*
that byte's value changes across consecutive frames of the same batch.
A byte that's part of the actively-animated channel (alpha, or a colour
component) will show small, smooth frame-to-frame deltas. A byte that's
structural/constant will show zero variance. A byte we're misreading
(wrong stride/offset) will show large, effectively-random deltas.

Self-contained: runs `tshark -x` directly against the real capture files
already in this repo (no external/pre-extracted input needed). Requires
tshark on PATH.

Usage: python decode_multizone.py
(run from anywhere -- paths below are relative to this file's location)

Findings from this exact script, see HANDOFF.md "Windows session 9,
continued a third time": confirmed structure --
  data[0]        report ID (0x04)
  data[1]        zone count N (observed range 1-8)
  data[2]        flag, always 0x01
  data[3:19]     zone-ID list, N x u16 LE, zero-padded to a fixed 16-byte
                 region (room for up to 8 zones) regardless of actual N
  data[19:19+4N] N x RGBA (R,G,B,Alpha), same order as the zone list,
                 always starting at offset 19 regardless of N
This directly explains the `static_armory_to_aura_lightbar_only.pcapng`
capture: keyboard zones get alpha~0 (invisible), the lightbar zone gets
full alpha with a real colour -- alpha is a real per-zone visibility
gate in this protocol, not just a brightness dimmer.
"""
import re
import subprocess
import sys
from pathlib import Path
from statistics import mean, pstdev
from collections import Counter, defaultdict

REPO_ROOT = Path(__file__).resolve().parent.parent

CAPTURE_FILES = {
    "test123": REPO_ROOT / "25" / "test123.pcapng",
    "aura123": REPO_ROOT / "25" / "123.pcapng",
    "lightbaronly": REPO_ROOT / "usb_capture_session7" / "static_armory_to_aura_lightbar_only.pcapng",
}


def extract_04_writes_hex(pcap_path: Path) -> str:
    """Run tshark to dump every 0x21 (SET_REPORT-class) control write as hex text."""
    result = subprocess.run(
        [
            "tshark", "-r", str(pcap_path),
            "-Y", "usb.bmRequestType==0x21 and usb.data_len==59",
            "-x", "-T", "text",
        ],
        capture_output=True, text=True, check=True,
    )
    return result.stdout


def parse_dump(text: str):
    entries = []
    parts = text.split("USB Control (")
    for part in parts[1:]:
        header, _, rest = part.partition("\n")
        nbytes = int(header.split(" bytes")[0])
        hexbytes = []
        for line in rest.splitlines():
            line = line.strip()
            if not line or not re.match(r"^[0-9a-fA-F]{4}\s", line):
                break
            toks = line.split()[1:]
            for t in toks:
                if re.fullmatch(r"[0-9a-fA-F]{2}", t):
                    hexbytes.append(int(t, 16))
        if len(hexbytes) >= nbytes:
            entries.append(hexbytes[:nbytes])
    return entries


def as_setup_and_data(entry):
    # entry = [bRequest, wValueL, wValueH, wIndexL, wIndexH, wLengthL, wLengthH, *data]
    if len(entry) < 7:
        return None
    bRequest, wValueL, wValueH, wIndexL, wIndexH, wLengthL, wLengthH = entry[:7]
    data = entry[7:]
    return {
        "bRequest": bRequest, "reportID": wValueL, "reportType": wValueH,
        "wLength": wLengthL | (wLengthH << 8), "data": data,
    }


def main():
    all_writes = []
    for src, path in CAPTURE_FILES.items():
        if not path.exists():
            print(f"WARNING: {path} not found, skipping")
            continue
        try:
            hexdump = extract_04_writes_hex(path)
        except FileNotFoundError:
            print("FATAL: tshark not found on PATH. Install Wireshark/tshark and retry.")
            sys.exit(1)
        for e in parse_dump(hexdump):
            sd = as_setup_and_data(e)
            if sd and sd["reportID"] == 0x04:
                sd["src"] = src
                all_writes.append(sd)

    print(f"Total 0x04 writes parsed: {len(all_writes)}")
    counts = Counter(w["data"][1] if len(w["data"]) > 1 else -1 for w in all_writes)
    print("Distribution of count field (data[1]):", dict(sorted(counts.items())))

    def zone_ids(data, count):
        return tuple((data[3 + 2 * i], data[4 + 2 * i]) for i in range(count))

    batches = defaultdict(list)
    for w in all_writes:
        data = w["data"]
        if len(data) < 2:
            continue
        count = data[1]
        if count < 2 or count > 16 or len(data) < 3 + 2 * count:
            continue
        key = (w["src"], count, zone_ids(data, count))
        batches[key].append(data)

    real_batches = {k: v for k, v in batches.items() if len(v) >= 8}
    print(f"\nMulti-zone batches with >=8 consecutive samples: {len(real_batches)}")
    for k in list(real_batches.keys())[:10]:
        print(f"  count={k[1]} zones={k[2]} n_samples={len(real_batches[k])}")

    if not real_batches:
        print("No batches with enough repetition found -- can't measure smoothness.")
        return

    top_batches = sorted(real_batches.items(), key=lambda kv: -len(kv[1]))[:3]
    for key, samples in top_batches:
        print(f"\n=== FULL byte table: src={key[0]} count={key[1]} zones={key[2]} n={len(samples)} ===")
        minlen = min(len(s) for s in samples)
        print(f"{'pos':>4} {'mean|delta|':>12} {'stdev':>8} {'min':>4} {'max':>4}  tag")
        for pos in range(minlen):
            vals = [s[pos] for s in samples]
            deltas = [abs(vals[i + 1] - vals[i]) for i in range(len(vals) - 1)]
            var = pstdev(vals) if len(set(vals)) > 1 else 0
            mdelta = mean(deltas)
            tag = "CONST" if var == 0 else ("ANIMATED" if (var > 5 and mdelta < 30) else "noisy/mixed")
            print(f"{pos:>4} {mdelta:>12.2f} {var:>8.2f} {min(vals):>4} {max(vals):>4}  {tag}")


if __name__ == "__main__":
    main()
