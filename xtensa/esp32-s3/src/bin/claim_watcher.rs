#![no_std]
#![no_main]

//! Passive observer of the NMEA 2000 address-claiming process.
//!
//! It never claims and never emits, so it cannot disturb the arbitration it
//! watches. Every Address Claim and message request is reported as it happens,
//! with the 64-bit device identity and a plain-English interpretation. An
//! activity summary follows every 10 seconds.
//!
//! A node that stops emitting for a minute moves to the history, so the live
//! table only ever shows what is really on the wire.
//!
//! Flash it on a third board, then power up `dual_run_1` and `dual_run_2`.

use defmt_rtt as _;
use esp_backtrace as _;

use embassy_futures::select::{Either, select};
use embassy_time::{Duration, Instant, Timer};
use esp32_s3::{
    ports::EspCanBus,
    starter::{init_bsp, init_can_listen_only},
};
use korri_n2k::protocol::{
    constants::{
        addr_mgmt_pgns::{CLAIM_PGN_60928, REQUEST_PGN_59904, REQUEST_PGN_LEN},
        address::{GLOBAL, NULL_ADDR_254},
    },
    management::iso_name::IsoName,
    transport::{can_frame::CanFrame, traits::can_bus::CanBus},
};

esp_bootloader_esp_idf::esp_app_desc!();

/// How many NAMEs the table holds. A busy backbone carries far fewer.
const MAX_NODES: usize = 16;

/// How often the bus report is printed.
const REPORT_PERIOD_MS: u64 = 10_000;

/// A claim answering a request lands well inside this window.
/// J1939 allows 250 ms, so this leaves room for a slow node.
const ANSWER_WINDOW_MS: u64 = 1_250;

/// Two claims on one address inside this window are the same fight.
const FIGHT_WINDOW_MS: u64 = 2_000;

/// A node that emitted nothing for this long has left the bus.
/// Six report windows, so a slow talker is never mistaken for a dead one.
const SILENCE_TIMEOUT_MS: u64 = 60_000;

/// How many departures the history keeps. The oldest is overwritten.
const MAX_HISTORY: usize = 8;

/// Closes a report block. Events between two blocks are then easy to spot.
const RULE: &str = "--------------------------------------------------";

/// Blank line between blocks.
///
/// defmt has no newline of its own, and every line carries its level and its
/// call site, so an empty message is the only separator available.
fn blank() {
    defmt::info!("");
}

/// What a claim frame did to the bus.
#[derive(Clone, Copy)]
enum ClaimKind {
    /// A NAME nobody had seen before.
    Enters,
    /// It already held this address and someone challenged it.
    Defends,
    /// It changed address.
    Moves,
    /// It re-announced an address nobody is fighting over.
    Repeats,
    /// Source address 254: it ran out of candidates.
    CannotClaim,
}

impl ClaimKind {
    fn label(self) -> &'static str {
        match self {
            Self::Enters => "new device requesting this address",
            Self::Defends => "defending against another device",
            Self::Moves => "moving from a previous address",
            Self::Repeats => "repeating its current address claim",
            Self::CannotClaim => "no configured address is available",
        }
    }
}

/// The last device identity that requested an address.
#[derive(Clone, Copy)]
struct LastClaim {
    name: IsoName,
    at_ms: u64,
}

/// One NAME seen on the bus, and where it currently sits.
#[derive(Clone, Copy)]
struct Node {
    name: IsoName,
    /// 254 while the node holds nothing.
    address: u8,
    claims: u32,
    /// When it took its current address.
    since_ms: u64,
    last_seen_ms: u64,
    /// Its previous Cannot Claim, so the retry period is visible. 0 means never.
    last_cannot_claim_ms: u64,
}

/// A node that went silent, kept so the report still shows it was here.
#[derive(Clone, Copy)]
struct Departed {
    name: IsoName,
    /// The address it held when it stopped talking.
    address: u8,
    claims: u32,
    last_seen_ms: u64,
}

/// The last ISO Request seen, kept to tell an answer from a spontaneous claim.
#[derive(Clone, Copy)]
struct Request {
    at_ms: u64,
    source_address: u8,
    destination_address: u8,
    requested_parameter_group_number: u32,
}

struct Watcher {
    nodes: [Option<Node>; MAX_NODES],
    /// Departures, oldest overwritten first.
    history: [Option<Departed>; MAX_HISTORY],
    history_next: usize,
    /// Frames per source address in the current window.
    traffic: [u32; 256],
    /// Last claimant on each address, so a defence can be told from a repeat.
    last_claim_on: [Option<LastClaim>; 256],
    last_request: Option<Request>,

    // Window counters, cleared by every report.
    frames: u32,
    claims: u32,
    requests: u32,
    cannot_claims: u32,
    conflicts: u32,
    moves: u32,
    malformed_frames: u32,
    // Session counters, never cleared.
    /// NAMEs that did not fit the table.
    dropped_nodes: u32,
    /// Nodes moved to the history.
    departures: u32,
}

impl Watcher {
    const fn new() -> Self {
        Self {
            nodes: [None; MAX_NODES],
            history: [None; MAX_HISTORY],
            history_next: 0,
            traffic: [0; 256],
            last_claim_on: [None; 256],
            last_request: None,
            frames: 0,
            claims: 0,
            requests: 0,
            cannot_claims: 0,
            conflicts: 0,
            moves: 0,
            malformed_frames: 0,
            dropped_nodes: 0,
            departures: 0,
        }
    }

    fn on_frame(&mut self, frame: &CanFrame, now_ms: u64) {
        self.frames = self.frames.saturating_add(1);
        let source = frame.id.source_address();
        self.traffic[source as usize] = self.traffic[source as usize].saturating_add(1);

        match frame.id.pgn() {
            CLAIM_PGN_60928 => self.on_claim(frame, now_ms),
            REQUEST_PGN_59904 => {
                self.touch(source, now_ms);
                self.on_request(frame, now_ms);
            }
            // Any ordinary frame proves the address holder is alive. Claims are
            // excluded because a competing device may be using the same source
            // address; its 64-bit identity decides which table row to update.
            _ => self.touch(source, now_ms),
        }
    }

    fn on_claim(&mut self, frame: &CanFrame, now_ms: u64) {
        // The NAME is the whole payload, little endian, exactly 8 bytes.
        if frame.len < 8 {
            self.malformed_frames = self.malformed_frames.saturating_add(1);
            blank();
            defmt::warn!(
                "[{=u64:09} ms] MALFORMED ADDRESS CLAIM | source address: {=u8:03} | received {=usize} bytes, expected 8",
                now_ms,
                frame.id.source_address(),
                frame.len
            );
            return;
        }

        let name = IsoName::from_raw(u64::from_le_bytes(frame.data));
        let address = frame.id.source_address();
        self.claims = self.claims.saturating_add(1);

        let challenged_by_another_device = address != NULL_ADDR_254
            && self.last_claim_on[address as usize].is_some_and(|last| {
                last.name != name && now_ms.saturating_sub(last.at_ms) < FIGHT_WINDOW_MS
            });

        // Who else holds this address. Must skip the claimer itself, otherwise a
        // defence would find its own table entry and report no conflict.
        let previous_holder = self.other_holder(address, name);
        let previous_address = self
            .nodes
            .iter()
            .flatten()
            .find(|node| node.name == name)
            .map(|node| node.address);

        let kind = self.record(name, address, now_ms, challenged_by_another_device);
        if address != NULL_ADDR_254 {
            self.last_claim_on[address as usize] = Some(LastClaim {
                name,
                at_ms: now_ms,
            });
        }
        // The retry period, measured on the wire rather than assumed.
        // `CANNOT_CLAIM_RETRY_DELAY_MS` is a tuning knob, not a protocol
        // constant, so a node may well use another one. Printed below the CLAIM
        // line it belongs to, hence the local.
        let mut retry_after_ms = None;
        if matches!(kind, ClaimKind::CannotClaim) {
            self.cannot_claims = self.cannot_claims.saturating_add(1);
            retry_after_ms = self
                .note_cannot_claim(name, now_ms)
                .map(|previous_ms| now_ms.saturating_sub(previous_ms));
        }
        if matches!(kind, ClaimKind::Moves) {
            self.moves = self.moves.saturating_add(1);
        }

        blank();

        defmt::info!(
            "[{=u64:09} ms] {=str} RECEIVED",
            now_ms,
            if address == NULL_ADDR_254 {
                "CANNOT CLAIM ANNOUNCEMENT"
            } else {
                "ADDRESS CLAIM"
            }
        );
        if address == NULL_ADDR_254 {
            defmt::warn!(
                "  requested address: NONE | reserved source address 254 means this device has no address"
            );
        } else {
            defmt::info!("  requested address: {=u8:03}", address);
        }
        defmt::info!(
            "  64-bit device identity (ISO NAME): {=u64:#018x}",
            name.raw()
        );
        defmt::info!(
            "  identity details | manufacturer code: {=u16} | device function: {=u8} | device class: {=u8} | device instance: {=u8} | unique number: {=u32:#x}",
            name.manufacturer_code(),
            name.device_function(),
            name.device_class(),
            name.device_instance(),
            name.unique_number()
        );
        defmt::info!(
            "  identity flag: arbitrary-address capable = {=bool} (true means it may search the full address range) | interpretation: {=str}",
            name.is_arbitrary_address_capable(),
            kind.label()
        );

        if let Some(previous) = previous_address
            && previous != address
            && previous != NULL_ADDR_254
        {
            if address == NULL_ADDR_254 {
                defmt::info!("  address change observed: {=u8:03} -> NONE", previous);
            } else {
                defmt::info!(
                    "  address change observed: {=u8:03} -> {=u8:03}",
                    previous,
                    address
                );
            }
        }

        if let Some(elapsed_ms) = retry_after_ms {
            defmt::info!(
                "  retry observed: this device announced Cannot Claim again after {=u64} ms",
                elapsed_ms
            );
        }

        if let Some(request) = self.pending_answer(address, now_ms) {
            defmt::info!(
                "  response matched: Address Claim request from source address {=u8:03}, received {=u64} ms earlier",
                request.source_address,
                now_ms.saturating_sub(request.at_ms)
            );
        }

        if let Some(previous) = previous_holder {
            self.conflicts = self.conflicts.saturating_add(1);

            // J1939 arbitration: the lower NAME wins the address.
            let (winner, loser) = if name.raw() < previous.name.raw() {
                (name, previous.name)
            } else {
                (previous.name, name)
            };

            // Settle it in the table straight away, the way the bus will settle
            // it a millisecond later. Taking the address from the winner instead
            // would read its next defence as a move, and would let the loser's
            // silence timer live off the winner's traffic.
            self.release(address, winner, now_ms);
            defmt::warn!(
                "  ADDRESS CONFLICT | two device identities requested address {=u8:03}",
                address
            );
            defmt::warn!(
                "  contenders | existing: {=u64:#018x} | new: {=u64:#018x}",
                previous.name.raw(),
                name.raw()
            );
            defmt::warn!(
                "  winner: {=u64:#018x} keeps address {=u8:03} because the lower numeric ISO NAME wins",
                winner.raw(),
                address
            );
            defmt::warn!(
                "  loser: {=u64:#018x} must try another configured address or announce Cannot Claim",
                loser.raw()
            );
        }
    }

    fn on_request(&mut self, frame: &CanFrame, now_ms: u64) {
        if frame.len < REQUEST_PGN_LEN {
            self.malformed_frames = self.malformed_frames.saturating_add(1);
            blank();
            defmt::warn!(
                "[{=u64:09} ms] MALFORMED MESSAGE REQUEST | source address: {=u8:03} | received {=usize} bytes, expected at least {=usize}",
                now_ms,
                frame.id.source_address(),
                frame.len,
                REQUEST_PGN_LEN
            );
            return;
        }

        // Three bytes, little endian, the Parameter Group Number being asked for.
        let requested = u32::from(frame.data[0])
            | (u32::from(frame.data[1]) << 8)
            | (u32::from(frame.data[2]) << 16);
        let destination = frame.id.destination().unwrap_or(GLOBAL);

        self.requests = self.requests.saturating_add(1);
        let request = Request {
            at_ms: now_ms,
            source_address: frame.id.source_address(),
            destination_address: destination,
            requested_parameter_group_number: requested,
        };
        self.last_request = Some(request);

        blank();

        defmt::info!(
            "[{=u64:09} ms] {=str} RECEIVED",
            now_ms,
            if requested == CLAIM_PGN_60928 {
                "ADDRESS CLAIM REQUEST"
            } else {
                "MESSAGE REQUEST"
            }
        );
        defmt::info!(
            "  requested Parameter Group Number: {=u32} | source address: {=u8:03}",
            requested,
            request.source_address
        );
        if destination == GLOBAL {
            defmt::info!("  destination: ALL DEVICES (global address 255)");
        } else {
            defmt::info!("  destination address: {=u8:03}", destination);
        }
    }

    /// The pending request this claim answers, if there is one.
    ///
    /// A request is answered by the node it was addressed to, or by everyone
    /// when it was broadcast.
    fn pending_answer(&self, address: u8, now_ms: u64) -> Option<Request> {
        let request = self.last_request?;
        let fresh = now_ms.saturating_sub(request.at_ms) < ANSWER_WINDOW_MS;
        let for_this_device =
            request.destination_address == GLOBAL || request.destination_address == address;

        (fresh && for_this_device && request.requested_parameter_group_number == CLAIM_PGN_60928)
            .then_some(request)
    }

    /// Insert or update the NAME, and say what the claim did.
    fn record(&mut self, name: IsoName, address: u8, now_ms: u64, contested: bool) -> ClaimKind {
        if let Some(node) = self
            .nodes
            .iter_mut()
            .flatten()
            .find(|node| node.name == name)
        {
            node.claims = node.claims.saturating_add(1);
            node.last_seen_ms = now_ms;

            let kind = if address == NULL_ADDR_254 {
                ClaimKind::CannotClaim
            } else if node.address != address {
                ClaimKind::Moves
            } else if contested {
                ClaimKind::Defends
            } else {
                ClaimKind::Repeats
            };

            if node.address != address {
                node.address = address;
                node.since_ms = now_ms;
            }
            return kind;
        }

        let free = self.nodes.iter_mut().find(|slot| slot.is_none());
        match free {
            Some(slot) => {
                *slot = Some(Node {
                    name,
                    address,
                    claims: 1,
                    since_ms: now_ms,
                    last_seen_ms: now_ms,
                    last_cannot_claim_ms: 0,
                });
            }
            // the table is full and this NAME is ignored. Raise
            // MAX_NODES if a real backbone ever overflows it.
            None => self.dropped_nodes = self.dropped_nodes.saturating_add(1),
        }

        if address == NULL_ADDR_254 {
            ClaimKind::CannotClaim
        } else {
            ClaimKind::Enters
        }
    }

    /// Mark the holder of `address` as alive.
    ///
    /// 254 is skipped: it is the addressless source, so several nodes can share
    /// it and touching the first one would be a guess.
    fn touch(&mut self, address: u8, now_ms: u64) {
        if address == NULL_ADDR_254 {
            return;
        }
        if let Some(node) = self
            .nodes
            .iter_mut()
            .flatten()
            .find(|node| node.address == address)
        {
            node.last_seen_ms = now_ms;
        }
    }

    /// Record this Cannot Claim, and hand back the previous one if there was one.
    fn note_cannot_claim(&mut self, name: IsoName, now_ms: u64) -> Option<u64> {
        let node = self
            .nodes
            .iter_mut()
            .flatten()
            .find(|node| node.name == name)?;
        let previous = (node.last_cannot_claim_ms != 0).then_some(node.last_cannot_claim_ms);
        node.last_cannot_claim_ms = now_ms;
        previous
    }

    /// Take `address` away from every node but `keep`.
    ///
    /// The loser of a conflict is addressless until it claims again, and saying
    /// so keeps `touch` and `holder_of` pointing at one node per address.
    fn release(&mut self, address: u8, keep: IsoName, now_ms: u64) {
        for node in self.nodes.iter_mut().flatten() {
            if node.address == address && node.name != keep {
                node.address = NULL_ADDR_254;
                node.since_ms = now_ms;
            }
        }
    }

    /// Move every silent node to the history and free its slot.
    ///
    /// A node that claimed once and vanished would otherwise sit in the table
    /// forever, and fill it on a bus where nodes come and go.
    fn retire_silent(&mut self, now_ms: u64) {
        for index in 0..MAX_NODES {
            let Some(node) = self.nodes[index] else {
                continue;
            };
            if now_ms.saturating_sub(node.last_seen_ms) < SILENCE_TIMEOUT_MS {
                continue;
            }

            self.nodes[index] = None;
            self.history[self.history_next] = Some(Departed {
                name: node.name,
                address: node.address,
                claims: node.claims,
                last_seen_ms: node.last_seen_ms,
            });
            self.history_next = (self.history_next + 1) % MAX_HISTORY;
            self.departures = self.departures.saturating_add(1);

            blank();

            defmt::warn!("[{=u64:09} ms] DEVICE NO LONGER OBSERVED", now_ms);
            defmt::warn!(
                "  64-bit device identity: {=u64:#018x} | silent for {=u64} seconds | moved to history",
                node.name.raw(),
                now_ms.saturating_sub(node.last_seen_ms) / 1000
            );
            if node.address == NULL_ADDR_254 {
                defmt::warn!("  last known address: NONE");
            } else {
                defmt::warn!("  last known address: {=u8:03}", node.address);
            }
        }
    }

    fn holder_of(&self, address: u8) -> Option<Node> {
        if address == NULL_ADDR_254 {
            return None;
        }
        self.nodes
            .iter()
            .flatten()
            .find(|node| node.address == address)
            .copied()
    }

    /// The holder of `address` that is not `claimer`.
    fn other_holder(&self, address: u8, claimer: IsoName) -> Option<Node> {
        if address == NULL_ADDR_254 {
            return None;
        }
        self.nodes
            .iter()
            .flatten()
            .find(|node| node.address == address && node.name != claimer)
            .copied()
    }

    /// Print the window, then clear its counters. The NAME table survives.
    fn report(&mut self, now_ms: u64, window_ms: u64, receive_errors_since_boot: u32) {
        let seconds = (window_ms / 1000).max(1);

        // Before printing, so a dead node never shows up as live.
        self.retire_silent(now_ms);

        blank();

        defmt::info!("{=str}", RULE);
        defmt::info!(
            "CONTROLLER AREA NETWORK ACTIVITY SUMMARY | time: {=u64} ms | interval: {=u64} seconds",
            now_ms,
            seconds
        );
        defmt::info!("{=str}", RULE);
        if self.frames == 0 {
            defmt::warn!(
                "NO CONTROLLER AREA NETWORK FRAMES RECEIVED during this interval | check transceiver power, common ground, CAN-H/CAN-L wiring, termination, and 250 kbps bitrate"
            );
        }
        defmt::info!(
            "all frames received: {=u32} | average frames per second: {=u32}",
            self.frames,
            (self.frames as u64 / seconds) as u32
        );
        defmt::info!(
            "controller receive errors since startup: {=u32} | malformed address-management frames: {=u32}",
            receive_errors_since_boot,
            self.malformed_frames
        );
        defmt::info!(
            "address claims: {=u32} | message requests: {=u32} | address conflicts: {=u32} | address changes: {=u32} | Cannot Claim announcements: {=u32}",
            self.claims,
            self.requests,
            self.conflicts,
            self.moves,
            self.cannot_claims
        );

        let known = self.nodes.iter().flatten().count();
        defmt::info!(
            "devices currently tracked: {=usize} | devices moved to history: {=u32} | identities dropped because the table was full: {=u32}",
            known,
            self.departures,
            self.dropped_nodes
        );

        for node in self.nodes.iter().flatten() {
            let frames = self.traffic[node.address as usize];

            if node.address == NULL_ADDR_254 {
                defmt::warn!(
                    "  device {=u64:#018x} | current address: NONE | status: CANNOT CLAIM | address announcements: {=u32} | without address for {=u64} seconds | last frame {=u64} ms ago",
                    node.name.raw(),
                    node.claims,
                    now_ms.saturating_sub(node.since_ms) / 1000,
                    now_ms.saturating_sub(node.last_seen_ms)
                );
                continue;
            }

            defmt::info!(
                "  device {=u64:#018x} | current and claimed address: {=u8:03} | status: ADDRESS CLAIMED | address announcements: {=u32} | frames from this address: {=u32} | held for {=u64} seconds | last frame {=u64} ms ago{=str}",
                node.name.raw(),
                node.address,
                node.claims,
                frames,
                now_ms.saturating_sub(node.since_ms) / 1000,
                now_ms.saturating_sub(node.last_seen_ms),
                if frames == 0 { "  SILENT" } else { "" }
            );
        }

        // Traffic from an address no claim ever explained. Either the claim
        // happened before this board booted, or the node never claims at all.
        //
        // 254 and 255 are skipped. Neither is a real source: 254 carries the
        // Cannot Claim frames already reported above, and 255 is the broadcast
        // destination.
        for address in 0..NULL_ADDR_254 as usize {
            let frames = self.traffic[address];
            if frames > 0 && self.holder_of(address as u8).is_none() {
                defmt::warn!(
                    "  UNEXPLAINED TRAFFIC | source address: {=usize:03} | frames: {=u32} | no Address Claim observed since this watcher started",
                    address,
                    frames
                );
            }
        }

        // Oldest first is not worth a sort: the ring is eight entries deep and
        // each line carries its own timestamp.
        if self.history.iter().flatten().next().is_some() {
            defmt::info!(
                "RECENTLY DEPARTED DEVICES | capacity: {=usize}",
                MAX_HISTORY
            );
            for gone in self.history.iter().flatten() {
                if gone.address == NULL_ADDR_254 {
                    defmt::info!(
                        "  device {=u64:#018x} | last address: NONE | address announcements: {=u32} | absent for {=u64} seconds",
                        gone.name.raw(),
                        gone.claims,
                        now_ms.saturating_sub(gone.last_seen_ms) / 1000
                    );
                } else {
                    defmt::info!(
                        "  device {=u64:#018x} | last address: {=u8:03} | address announcements: {=u32} | absent for {=u64} seconds",
                        gone.name.raw(),
                        gone.address,
                        gone.claims,
                        now_ms.saturating_sub(gone.last_seen_ms) / 1000
                    );
                }
            }
        }

        defmt::info!("{=str}", RULE);

        self.traffic = [0; 256];
        self.frames = 0;
        self.claims = 0;
        self.requests = 0;
        self.cannot_claims = 0;
        self.conflicts = 0;
        self.moves = 0;
        self.malformed_frames = 0;
    }
}

#[esp_hal_embassy::main]
async fn main(_spawner: embassy_executor::Spawner) {
    let mut bus = EspCanBus::new(init_can_listen_only(init_bsp()));
    let mut watcher = Watcher::new();
    let mut window_start = Instant::now();
    let report_period = Duration::from_millis(REPORT_PERIOD_MS);
    let mut next_report = window_start + report_period;

    blank();
    defmt::info!("{=str}", RULE);
    defmt::info!("ADDRESS-CLAIM BUS WATCHER");
    defmt::info!(
        "mode: PASSIVE | receives Controller Area Network (CAN) frames | sends nothing | owns no address"
    );
    defmt::info!(
        "device identity: ISO NAME is the standard 64-bit identifier carried by every Address Claim"
    );
    defmt::info!(
        "arbitration rule: when two devices request one address, the lower numeric ISO NAME wins"
    );
    defmt::info!(
        "special value: source address 254 means Cannot Claim; the device has no usable address"
    );
    defmt::info!(
        "ordinary application frames are counted by source address, not printed individually"
    );
    defmt::info!(
        "the first activity summary will be printed in {=u64} seconds even if no frame is received",
        REPORT_PERIOD_MS / 1000
    );
    defmt::info!("{=str}", RULE);

    loop {
        match select(bus.recv(), Timer::at(next_report)).await {
            Either::First(result) => match result {
                Ok(frame) => {
                    let now_ms = Instant::now().as_millis();
                    watcher.on_frame(&frame, now_ms);
                }
                Err(err) => {
                    defmt::error!("Controller Area Network receive failed: {}", err);
                }
            },
            Either::Second(_) => {}
        }

        let now = Instant::now();
        if now >= next_report {
            let elapsed_ms = window_start.elapsed().as_millis();
            watcher.report(now.as_millis(), elapsed_ms, bus.rx_errors());
            window_start = now;
            next_report = now + report_period;
        }
    }
}
