#!/usr/bin/env python3
"""Decode the NMEA2000 sniffer capture stream and check its integrity.

    ./decode_capture.py capture.bin              # recorded file
    ./decode_capture.py capture.bin --csv        # CSV output
    ./decode_capture.py capture.bin --quiet      # report only
    ./decode_capture.py /dev/ttyACM0             # live

Integrity is checked from two independent sources: the target counters (losses
before USB) and the sequence numbers (losses on the USB link, which the target
cannot see).
"""

import argparse
import os
import stat
import struct
import sys
from collections import Counter

RECORD_SIZE = 24
MAGIC = b"KN2KCAP\x01"
RECORD_FRAME = 0x01
RECORD_STATS = 0x02

FLAG_EXTENDED = 0b0001
FLAG_REMOTE = 0b0010

CHANNEL_DEPTH = 1024
BACKLOG_LIMIT = 32
BROADCAST = 0xFF

PGN_NAMES = {
    59392: "ISO Acknowledgement",
    59904: "ISO Request",
    60160: "ISO TP Data Transfer",
    60416: "ISO TP Connection Mgmt",
    60928: "ISO Address Claim",
    65240: "ISO Commanded Address",
    126208: "NMEA Group Function",
    126983: "Alert",
    126984: "Alert Response",
    126985: "Alert Text",
    126992: "System Time",
    126993: "Heartbeat",
    126996: "Product Information",
    126998: "Configuration Information",
    127233: "Man Overboard",
    127237: "Heading/Track Control",
    127245: "Rudder",
    127250: "Vessel Heading",
    127251: "Rate of Turn",
    127257: "Attitude",
    127258: "Magnetic Variation",
    127488: "Engine, Rapid Update",
    127489: "Engine, Dynamic",
    127493: "Transmission, Dynamic",
    127497: "Trip Parameters, Engine",
    127501: "Binary Switch Bank",
    127503: "AC Input Status",
    127504: "AC Output Status",
    127505: "Fluid Level",
    127506: "DC Detailed Status",
    127507: "Charger Status",
    127508: "Battery Status",
    127513: "Battery Configuration",
    128259: "Speed, Water Referenced",
    128267: "Water Depth",
    128275: "Distance Log",
    129025: "Position, Rapid Update",
    129026: "COG & SOG, Rapid Update",
    129029: "GNSS Position Data",
    129033: "Local Time Offset",
    129038: "AIS Class A Position",
    129039: "AIS Class B Position",
    129040: "AIS Class B Extended",
    129041: "AIS Aids to Navigation",
    129044: "Datum",
    129283: "Cross Track Error",
    129284: "Navigation Data",
    129285: "Route/WP Information",
    129291: "Set & Drift, Rapid",
    129539: "GNSS DOPs",
    129540: "GNSS Sats in View",
    130306: "Wind Data",
    130310: "Environmental Parameters",
    130311: "Environmental Parameters",
    130312: "Temperature",
    130313: "Humidity",
    130314: "Actual Pressure",
    130316: "Temperature, Extended",
    130577: "Direction Data",
}


# ----------------------------------------------------------------------- colour

def _colour_enabled(stream):
    return stream.isatty() and os.environ.get("NO_COLOR") is None


OUT_COLOUR = _colour_enabled(sys.stdout)
ERR_COLOUR = _colour_enabled(sys.stderr)

BOLD, DIM, RED, GREEN, YELLOW, CYAN = "1", "2", "31", "32", "33", "36"


def paint(text, code, enabled):
    return f"\033[{code}m{text}\033[0m" if enabled else text


def out(text, code=None):
    return paint(text, code, OUT_COLOUR) if code else text


def err(text, code=None):
    return paint(text, code, ERR_COLOUR) if code else text


# ---------------------------------------------------------------------- format

def pgn_name(pgn):
    return PGN_NAMES.get(pgn, "—")


def hex_bytes(data):
    return " ".join(f"{b:02x}" for b in data)


def decode_n2k_id(can_id):
    """Split a 29-bit identifier into (priority, PGN, source, destination)."""
    priority = (can_id >> 26) & 0x07
    data_page = (can_id >> 24) & 0x03
    pdu_format = (can_id >> 16) & 0xFF
    pdu_specific = (can_id >> 8) & 0xFF
    source = can_id & 0xFF

    if pdu_format < 240:
        # PDU1: addressed message, PS holds the destination.
        return priority, (data_page << 16) | (pdu_format << 8), source, pdu_specific

    # PDU2: broadcast, PS is part of the PGN.
    pgn = (data_page << 16) | (pdu_format << 8) | pdu_specific
    return priority, pgn, source, BROADCAST


# --------------------------------------------------------------------- stream

def open_source(path):
    """Open a file or a serial port, forcing raw mode on a port.

    A port left in canonical mode mangles binary data and eventually blocks the
    target on write.
    """
    stream = open(path, "rb", buffering=0)
    if not stat.S_ISCHR(os.fstat(stream.fileno()).st_mode):
        return stream

    try:
        import termios
        import tty

        tty.setraw(stream.fileno(), termios.TCSANOW)
    except (ImportError, OSError):
        print(err(f"warning: could not set {path} to raw mode", YELLOW),
              file=sys.stderr)

    return stream


def looks_valid(record):
    """Guard against a false sync: MAGIC can appear inside frame data."""
    if record.startswith(MAGIC):
        return True
    if record[0] == RECORD_STATS:
        return True
    if record[0] == RECORD_FRAME:
        return (record[1] & 0x0F) <= 8
    return False


def records(stream, integrity):
    """Split the stream into records, synced on the session header."""
    synced = False
    buffer = b""

    while True:
        chunk = stream.read(RECORD_SIZE)
        if not chunk:
            return
        integrity.bytes_read += len(chunk)
        buffer += chunk

        while len(buffer) >= RECORD_SIZE:
            if not synced:
                index = buffer.find(MAGIC)
                if index < 0:
                    buffer = buffer[-(len(MAGIC) - 1):]
                    break
                buffer = buffer[index:]
                if len(buffer) < RECORD_SIZE:
                    break
                synced = integrity.synced = True
            elif not looks_valid(buffer[:RECORD_SIZE]):
                # Sync lost: search again, shifted by one byte.
                integrity.resyncs += 1
                synced = False
                buffer = buffer[1:]
                continue

            yield buffer[:RECORD_SIZE]
            buffer = buffer[RECORD_SIZE:]


def parse_frame(record):
    length = record[1] & 0x0F
    flags = record[1] >> 4
    return {
        "timestamp_us": struct.unpack_from("<Q", record, 8)[0],
        "id": struct.unpack_from("<I", record, 4)[0],
        "data": record[16:16 + length],
        "extended": bool(flags & FLAG_EXTENDED),
        "remote": bool(flags & FLAG_REMOTE),
    }


def parse_stats(record):
    rx, channel, overrun, soft, sink, depth = struct.unpack_from("<IIIHHH", record, 4)
    return {
        "frames_rx": rx,
        "channel_drops": channel,
        "hw_overruns": overrun,
        "soft_errors": soft,
        "sink_drops": sink,
        "max_channel_depth": depth,
        "max_backlog_run": record[22],
        "bus_off": record[23],
    }


DELTA_FIELDS = ("frames_rx", "channel_drops", "hw_overruns", "soft_errors",
                "sink_drops", "bus_off")

# Counters and peaks the target never decreases.
MONOTONIC_FIELDS = DELTA_FIELDS + ("max_channel_depth", "max_backlog_run")


def plausible(snapshot):
    """Firmware invariants, broken by a corrupted snapshot."""
    return (
        snapshot["max_channel_depth"] <= CHANNEL_DEPTH
        and snapshot["channel_drops"] <= snapshot["frames_rx"]
    )


# ------------------------------------------------------------------ integrity

class Integrity:
    """Loss tracking, as seen by the target and by the link."""

    def __init__(self):
        self.expected_seq = None
        self.link_gaps = 0
        self.link_lost = 0
        self.worst_gap = 0
        self.records = 0
        self.frames = 0
        self.headers = 0
        self.resyncs = 0
        self.bytes_read = 0
        self.synced = False
        self.bad_stats = 0
        self.first_stats = None
        self.last_stats = None
        self.bitrate = None
        self.sources = Counter()
        self.pgns = Counter()
        self.first_us = None
        self.last_us = None

    def check_seq(self, seq):
        self.records += 1
        if self.expected_seq is not None and seq != self.expected_seq:
            missing = (seq - self.expected_seq) & 0xFFFF
            self.link_gaps += 1
            self.link_lost += missing
            self.worst_gap = max(self.worst_gap, missing)
        self.expected_seq = (seq + 1) & 0xFFFF

    def record_frame(self, timestamp_us, pgn, source):
        self.frames += 1
        self.sources[source] += 1
        self.pgns[pgn] += 1
        if self.first_us is None:
            self.first_us = timestamp_us
        self.last_us = timestamp_us

    def record_stats(self, snapshot):
        # Target counters accumulate since its boot, usually well before the
        # capture started. Only the delta describes the captured window.
        #
        # Two filters reject snapshots from a corrupted record: the firmware
        # invariants, then counter monotonicity.
        if not plausible(snapshot):
            self.bad_stats += 1
            return

        if self.last_stats is not None:
            if any(snapshot[k] < self.last_stats[k] for k in MONOTONIC_FIELDS):
                self.bad_stats += 1
                return

        if self.first_stats is None:
            self.first_stats = snapshot
        self.last_stats = snapshot

    @property
    def duration_s(self):
        if self.first_us is None or self.last_us == self.first_us:
            return 0.0
        return (self.last_us - self.first_us) / 1e6

    # --------------------------------------------------------------- report

    def report(self):
        if not self.synced:
            return "\n".join(self._unsynced())
        return "\n".join(self._summary() + [""] + self._integrity())

    def _unsynced(self):
        lines = [rule("Capture"), ""]
        lines.append(f"  {self.bytes_read} bytes read, no session header found.")
        lines.append("")
        if self.bytes_read == 0:
            lines.append(err("  Empty stream.", RED) + " Is the target running, on this port?")
        else:
            lines.append(err("  Data is not in the expected format.", RED) + " Things to check:")
            lines.append("    - port read without raw mode -> use `just capture`")
            lines.append("    - another program writing to this port")
            lines.append("    - wrong port, or outdated firmware")
        return lines

    def _summary(self):
        lines = [rule("Capture"), ""]

        if not self.frames:
            lines.append(field("frames", "none"))
            return lines

        seconds = self.duration_s
        parts = [f"{seconds:.2f} s", f"{self.frames} frames"]
        # Below one second, an extrapolated rate would be meaningless.
        if seconds >= 1.0:
            parts.append(f"{self.frames / seconds:.0f} frames/s")
        if self.bitrate:
            parts.append(f"bus {self.bitrate} bit/s")
        lines.append(field("duration", " | ".join(parts)))

        sources = "  ".join(
            f"{src} {err(f'({count})', DIM)}" for src, count in sorted(self.sources.items())
        )
        lines.append(field("sources", sources))
        lines.append("")

        lines.append(err(PGN_HEADER, DIM))
        for pgn, count in self.pgns.most_common(PGN_ROWS):
            share = 100 * count / self.frames
            lines.append(f"  {pgn:>6}  {pgn_name(pgn):<26}{count:>8}  {share:>5.1f}%")
        if len(self.pgns) > PGN_ROWS:
            lines.append(err(f"  ... and {len(self.pgns) - PGN_ROWS} more PGNs", DIM))

        return lines

    def _integrity(self):
        lines = [rule("Integrity"), ""]

        if self.resyncs:
            lines.append(field("resyncs", err(f"{self.resyncs} - stream truncated or interrupted",
                                              YELLOW)))
        if self.bad_stats:
            lines.append(field("snapshots", err(f"{self.bad_stats} corrupted, discarded", YELLOW)))

        if self.link_gaps:
            lines.append(field("USB link", err(
                f"{self.link_lost} records lost in {self.link_gaps} gaps", RED)))
            if self.worst_gap > 2 * CHANNEL_DEPTH:
                lines.append(field("", err(
                    f"gap of {self.worst_gap}: too large for a real loss,", DIM)))
                lines.append(field("", err(
                    "the stream is likely corrupted rather than truncated", DIM)))
        else:
            lines.append(field("USB link", "intact, no sequence gap"))

        if self.last_stats is None:
            lines.append(field("counters", "no snapshot received"))
            return lines

        last, first = self.last_stats, self.first_stats
        delta = {k: last[k] - first[k] for k in DELTA_FIELDS}

        lines.append(field("window", describe_counters(delta)))
        lines.append(field("since boot", describe_counters(last)))

        earlier = last["sink_drops"] - delta["sink_drops"]
        if earlier:
            lines.append(field("", err(
                f"including {earlier} USB losses before the capture - the target "
                "was writing with no reader, which is expected", DIM)))

        lines.append(field("headroom", (
            f"channel {last['max_channel_depth']}/{CHANNEL_DEPTH} | "
            f"burst {last['max_backlog_run']}/{BACKLOG_LIMIT}"
        )))

        lines.append("")
        lines.append(self._verdict(first, last, delta))

        if last["max_backlog_run"] >= 24:
            lines.append("")
            lines.append(err(
                f"  ! bursts close to the {BACKLOG_LIMIT} limit: frames may have been"
                "\n    lost without being counted.", YELLOW))

        return lines

    def _verdict(self, first, last, delta):
        if first is last:
            return field("VERDICT", err(
                "window too short - capture for more than two seconds", YELLOW))

        lost = [k for k in ("channel_drops", "hw_overruns", "sink_drops") if delta[k]]
        if self.link_gaps or lost:
            return field("VERDICT", err("capture INCOMPLETE", f"{BOLD};{RED}"))
        return field("VERDICT", err("capture complete", f"{BOLD};{GREEN}"))


# ----------------------------------------------------------------- formatting

LABEL_WIDTH = 13
PGN_ROWS = 12
PGN_HEADER = f"  {'PGN':>6}  {'name':<26}{'frames':>8}  {'share':>6}"


def rule(title):
    return err(f"── {title} " + "─" * max(0, 60 - len(title)), CYAN)


def field(label, value):
    return f"  {label:<{LABEL_WIDTH}}{value}"


def describe_counters(counters):
    dropped = counters["channel_drops"] + counters["hw_overruns"] + counters["sink_drops"]
    text = f"{counters['frames_rx']} received"

    if dropped:
        detail = ", ".join(
            f"{name} {counters[key]}"
            for key, name in (("channel_drops", "channel"),
                              ("hw_overruns", "overrun"),
                              ("sink_drops", "USB"))
            if counters[key]
        )
        text += " | " + err(f"{dropped} lost ({detail})", RED)
    else:
        text += " | no loss"

    extra = [f"{name} {counters[key]}"
             for key, name in (("soft_errors", "errors"), ("bus_off", "bus-off"))
             if counters[key]]
    if extra:
        text += " | " + err(", ".join(extra), YELLOW)

    return text


# ---------------------------------------------------------------------- main

FRAME_HEADER = (
    f"{'t (s)':>12} {'d (us)':>9}  {'PGN':>6}  {'name':<26} {'src':>3} {'dst':>4}  data"
)


def main():
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("source", help="capture file or serial port")
    parser.add_argument("--csv", action="store_true", help="CSV output")
    parser.add_argument("--quiet", action="store_true", help="report only")
    args = parser.parse_args()

    integrity = Integrity()
    listing = not args.quiet
    previous_us = None
    header_shown = False

    if args.csv and listing:
        print("timestamp_us,delta_us,priority,pgn,source,destination,len,data")

    try:
        with open_source(args.source) as stream:
            for record in records(stream, integrity):
                if record.startswith(MAGIC):
                    integrity.headers += 1
                    if integrity.bitrate is None:
                        integrity.bitrate = struct.unpack_from("<I", record, 8)[0]
                    continue

                integrity.check_seq(struct.unpack_from("<H", record, 2)[0])

                if record[0] == RECORD_STATS:
                    integrity.record_stats(parse_stats(record))
                    continue

                frame = parse_frame(record)
                priority, pgn, source, destination = decode_n2k_id(frame["id"])
                integrity.record_frame(frame["timestamp_us"], pgn, source)

                if not listing:
                    continue

                delta = None if previous_us is None else frame["timestamp_us"] - previous_us
                previous_us = frame["timestamp_us"]

                if args.csv:
                    print(
                        f"{frame['timestamp_us']},{delta or 0},{priority},{pgn},"
                        f"{source},{destination},{len(frame['data'])},"
                        f"{frame['data'].hex()}"
                    )
                    continue

                if not header_shown:
                    print(out(FRAME_HEADER, DIM))
                    header_shown = True

                elapsed = (frame["timestamp_us"] - integrity.first_us) / 1e6
                print(
                    f"{elapsed:>12.6f} "
                    f"{'—' if delta is None else f'+{delta}':>9}  "
                    f"{pgn:>6}  {out(f'{pgn_name(pgn):<26}', DIM)} "
                    f"{source:>3} "
                    f"{'GLOB' if destination == BROADCAST else destination:>4}  "
                    f"{hex_bytes(frame['data'])}"
                )
    except KeyboardInterrupt:
        pass

    # Without this flush the report (stderr, unbuffered) comes out before the
    # listing as soon as stdout is redirected.
    sys.stdout.flush()
    print("", file=sys.stderr)
    print(integrity.report(), file=sys.stderr)


if __name__ == "__main__":
    main()
