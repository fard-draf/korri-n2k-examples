//! PGN generators, free of any runtime.
//!
//! Each one holds whatever drifts between two messages and hands back a filled
//! PGN. Nothing here sleeps, sends, or logs, so the same generator feeds the
//! embassy tasks on a board and the tokio loop on Linux.
//!
//! Add a PGN by writing one struct here and implementing [`Sample`].

use korri_n2k::infra::codec::traits::PgnData;
use korri_n2k::protocol::lookups::*;
use korri_n2k::protocol::messages::*;

/// A PGN that can be published on a fixed period.
pub trait Sample {
    type Data: PgnData;

    /// The PGN number to emit under.
    const PGN: u32;
    /// NMEA 2000 priority, 0 is the highest.
    const PRIORITY: u8;
    /// Milliseconds between two messages.
    const PERIOD_MS: u64;

    /// The next message. Called once per period.
    fn next(&mut self) -> Self::Data;
}

/// The transmission interval canboat declares, or `fallback` when it declares none.
const fn declared_interval_ms(declared: Option<u16>, fallback: u64) -> u64 {
    match declared {
        Some(interval_ms) => interval_ms as u64,
        None => fallback,
    }
}

//================================================================================ 126985

/// A technical alarm, always the same one.
#[derive(Default)]
pub struct AlertText;

impl AlertText {
    pub const fn new() -> Self {
        Self
    }
}

impl Sample for AlertText {
    type Data = Pgn126985;
    const PGN: u32 = 126985;
    const PRIORITY: u8 = 6;
    const PERIOD_MS: u64 = 1_000;

    fn next(&mut self) -> Self::Data {
        let mut alert = Pgn126985::new();
        alert.alert_type = AlertType::Alarm;
        alert.alert_category = AlertCategory::Technical;
        alert.alert_system = 1;
        alert.alert_sub_system = 0;
        alert.alert_id = 100;
        alert.language_id = AlertLanguageId::EnglishUs;
        alert
    }
}

//================================================================================ 126993

/// Heartbeat.
///
/// The declared interval is 60 s. One millisecond is a deliberate stress
/// setting: `stress_all` uses it to saturate the backbone.
#[derive(Default)]
pub struct Heartbeat;

impl Heartbeat {
    pub const fn new() -> Self {
        Self
    }
}

impl Sample for Heartbeat {
    type Data = Pgn126993;
    const PGN: u32 = 126993;
    const PRIORITY: u8 = 2;
    const PERIOD_MS: u64 = 1;

    fn next(&mut self) -> Self::Data {
        let mut heartbeat = Pgn126993::new();
        heartbeat.equipment_status = EquipmentStatus::Operational;
        heartbeat.controller1_state = ControllerState::ErrorPassive;
        heartbeat.data_transmit_offset = 0.0;
        heartbeat
    }
}

//================================================================================ 127237

/// Autopilot heading control, rudder order sweeping a full turn.
pub struct HeadingControl {
    rudder_angle: f32,
}

impl HeadingControl {
    pub const fn new() -> Self {
        Self { rudder_angle: 0.0 }
    }
}

impl Default for HeadingControl {
    fn default() -> Self {
        Self::new()
    }
}

impl Sample for HeadingControl {
    type Data = Pgn127237;
    const PGN: u32 = 127237;
    const PRIORITY: u8 = 2;
    const PERIOD_MS: u64 = 250;

    fn next(&mut self) -> Self::Data {
        let mut heading = Pgn127237::new();
        heading.rudder_limit_exceeded = YesNo::No;
        heading.off_heading_limit_exceeded = YesNo::No;
        heading.off_track_limit_exceeded = YesNo::No;
        heading.override_field = YesNo::No;
        heading.steering_mode = SteeringMode::MainSteering;
        heading.turn_mode = TurnMode::RudderLimitControlled;
        heading.heading_reference = DirectionReference::Magnetic1;
        heading.commanded_rudder_direction = DirectionRudder::NoOrder;
        heading.commanded_rudder_angle = self.rudder_angle;
        heading.heading_to_steer_course = 0.0;
        heading.track = 0.0;

        self.rudder_angle = (self.rudder_angle + 0.5) % 360.0;
        heading
    }
}

//================================================================================ 127245

/// Rudder position, sweeping a full turn.
pub struct Rudder {
    position: f32,
}

impl Rudder {
    pub const fn new() -> Self {
        Self { position: 0.0 }
    }
}

impl Default for Rudder {
    fn default() -> Self {
        Self::new()
    }
}

impl Sample for Rudder {
    type Data = Pgn127245;
    const PGN: u32 = 127245;
    const PRIORITY: u8 = 2;
    const PERIOD_MS: u64 =
        declared_interval_ms(Pgn127245::PGN_127245_DESCRIPTOR.trans_interval, 100);

    fn next(&mut self) -> Self::Data {
        let mut rudder = Pgn127245::new();
        rudder.instance = 0;
        rudder.direction_order = DirectionRudder::NoOrder;
        rudder.angle_order = 0.0;
        rudder.position = self.position;

        self.position = (self.position + 0.5) % 360.0;
        rudder
    }
}

//================================================================================ 127488

/// Engine rapid update, revolutions climbing and trim sweeping.
pub struct EngineRapid {
    rpm: u16,
    tilt_trim: i8,
}

impl EngineRapid {
    pub const fn new() -> Self {
        Self {
            rpm: 0,
            tilt_trim: 0,
        }
    }
}

impl Default for EngineRapid {
    fn default() -> Self {
        Self::new()
    }
}

impl Sample for EngineRapid {
    type Data = Pgn127488;
    const PGN: u32 = 127488;
    const PRIORITY: u8 = 2;
    const PERIOD_MS: u64 = 100;

    fn next(&mut self) -> Self::Data {
        let mut engine = Pgn127488::new();
        engine.instance = EngineInstance::SingleEngineOrDualEnginePort;
        engine.speed = (1000.0 + (self.rpm as f32)) % 2500.0;
        engine.boost_pressure = 1478.0;
        engine.tilt_trim = self.tilt_trim;

        self.rpm = self.rpm.wrapping_add(1);
        self.tilt_trim = (self.tilt_trim + 1) % 101;
        engine
    }
}

//================================================================================ 127489

/// Engine dynamic parameters, every reading drifting off one counter.
pub struct EngineDynamic {
    tilt: u8,
}

impl EngineDynamic {
    pub const fn new() -> Self {
        Self { tilt: 0 }
    }
}

impl Default for EngineDynamic {
    fn default() -> Self {
        Self::new()
    }
}

impl Sample for EngineDynamic {
    type Data = Pgn127489;
    const PGN: u32 = 127489;
    const PRIORITY: u8 = 2;
    const PERIOD_MS: u64 = 200;

    fn next(&mut self) -> Self::Data {
        let tilt = self.tilt;

        let mut engine = Pgn127489::new();
        engine.instance = EngineInstance::SingleEngineOrDualEnginePort;
        engine.oil_pressure = (50.0 + (tilt as f32)) % 151.0;
        engine.oil_temperature = (10.0 + (tilt as f32)) % 90.0;
        engine.temperature = (tilt as f32) % 90.0;
        engine.alternator_potential = (12.4 + (tilt as f32)) % 14.2;
        engine.fuel_rate = (100.0 - (tilt as f32)) % 100.0;
        engine.total_engine_hours = 15201 + (tilt as u32);
        engine.coolant_pressure = (123.0 + (tilt as f32)) % 150.0;
        engine.fuel_pressure = (168.7 - tilt as f32) % 150.0;
        engine.set_discrete_status1_bit(EngineStatus1::LowOilLevel, true);
        engine.set_discrete_status2_bit(EngineStatus2::EngineCommError, true);
        engine.engine_load = 1 + (tilt as i8) % 100;
        engine.engine_torque = 1 + (tilt as i8) % 100;

        self.tilt = tilt.wrapping_add(1);
        engine
    }
}

//================================================================================ 127503

/// AC input status, a fixed reading.
#[derive(Default)]
pub struct AcInput;

impl AcInput {
    pub const fn new() -> Self {
        Self
    }
}

impl Sample for AcInput {
    type Data = Pgn127503;
    const PGN: u32 = 127503;
    const PRIORITY: u8 = 2;
    const PERIOD_MS: u64 = 1_000;

    fn next(&mut self) -> Self::Data {
        let mut ac_input = Pgn127503::new();
        ac_input.instance = 210;
        ac_input.number_of_lines = 185;
        ac_input
    }
}

//================================================================================ 128259

/// Speed through water and over ground, fixed.
#[derive(Default)]
pub struct Speed;

impl Speed {
    pub const fn new() -> Self {
        Self
    }
}

impl Sample for Speed {
    type Data = Pgn128259;
    const PGN: u32 = 128259;
    const PRIORITY: u8 = 2;
    const PERIOD_MS: u64 =
        declared_interval_ms(Pgn128259::PGN_128259_DESCRIPTOR.trans_interval, 5_000);

    fn next(&mut self) -> Self::Data {
        let mut speed = Pgn128259::new();
        speed.sid = 240;
        speed.speed_water_referenced = 5.5;
        speed.speed_ground_referenced = 55.8;
        speed.speed_water_referenced_type = WaterReference::PaddleWheel;
        speed.speed_direction = 158;
        speed
    }
}

//================================================================================ 128267

/// Water depth, climbing then wrapping.
pub struct Depth {
    tick: u16,
}

impl Depth {
    pub const fn new() -> Self {
        Self { tick: 1 }
    }
}

impl Default for Depth {
    fn default() -> Self {
        Self::new()
    }
}

impl Sample for Depth {
    type Data = Pgn128267;
    const PGN: u32 = 128267;
    const PRIORITY: u8 = 2;
    const PERIOD_MS: u64 = 1_000;

    fn next(&mut self) -> Self::Data {
        let mut depth = Pgn128267::new();
        depth.sid = 42;
        depth.depth = (25.0 + (self.tick as f64)) % 255.0;
        depth.offset = 0.0;
        depth.range = 50.0;

        self.tick = self.tick.wrapping_add(1);
        depth
    }
}

//================================================================================ 129025

/// Position, drifting inside a box off southern Brittany.
pub struct Position {
    latitude: f64,
    longitude: f64,
}

impl Position {
    const MAX_LAT: f64 = 47.40;
    const MIN_LAT: f64 = 44.40;
    const MAX_LONG: f64 = -3.00;
    const MIN_LONG: f64 = -5.00;

    pub const fn new() -> Self {
        Self {
            latitude: 46.00,
            longitude: -3.70,
        }
    }
}

impl Default for Position {
    fn default() -> Self {
        Self::new()
    }
}

impl Sample for Position {
    type Data = Pgn129025;
    const PGN: u32 = 129025;
    const PRIORITY: u8 = 2;
    const PERIOD_MS: u64 = 1_000;

    fn next(&mut self) -> Self::Data {
        let mut position = Pgn129025::new();
        position.latitude = self.latitude;
        position.longitude = self.longitude;

        self.latitude = if self.latitude > Self::MAX_LAT {
            Self::MIN_LAT
        } else {
            self.latitude + 0.03
        };
        self.longitude = if self.longitude > Self::MAX_LONG {
            Self::MIN_LONG
        } else {
            self.longitude + 0.02
        };

        position
    }
}

//================================================================================ 129038

/// AIS class A position report, a fixed target.
#[derive(Default)]
pub struct AisClassA;

impl AisClassA {
    pub const fn new() -> Self {
        Self
    }
}

impl Sample for AisClassA {
    type Data = Pgn129038;
    const PGN: u32 = 129038;
    const PRIORITY: u8 = 2;
    const PERIOD_MS: u64 = 6_000;

    fn next(&mut self) -> Self::Data {
        let mut ais = Pgn129038::new();
        ais.message_id = AisMessageId::ScheduledClassAPositionReport;
        ais.repeat_indicator = RepeatIndicator::Initial;
        ais.user_id = 123456789;
        ais.longitude = -2.71842;
        ais.latitude = 47.64425;
        ais.position_accuracy = PositionAccuracy::High;
        ais.raim = RaimFlag::NotInUse;
        ais.time_stamp = TimeStamp::NotAvailable;
        ais.cog = 45.0;
        ais.sog = 5.0;
        ais
    }
}

//================================================================================ 129039

/// AIS class B position report, a fixed target.
#[derive(Default)]
pub struct AisClassB;

impl AisClassB {
    pub const fn new() -> Self {
        Self
    }
}

impl Sample for AisClassB {
    type Data = Pgn129039;
    const PGN: u32 = 129039;
    const PRIORITY: u8 = 2;
    const PERIOD_MS: u64 = 30_000;

    fn next(&mut self) -> Self::Data {
        let mut ais = Pgn129039::new();
        ais.message_id = AisMessageId::ScheduledClassAPositionReport;
        ais.repeat_indicator = RepeatIndicator::Initial;
        ais.user_id = 987654321;
        ais.longitude = -2.71842;
        ais.latitude = 47.64425;
        ais.position_accuracy = PositionAccuracy::High;
        ais.raim = RaimFlag::NotInUse;
        ais.time_stamp = TimeStamp::NotAvailable;
        ais.cog = 90.0;
        ais.sog = 3.0;
        ais
    }
}

//================================================================================ 129044

/// Datum, WGS84 with no offset.
#[derive(Default)]
pub struct Datum;

impl Datum {
    pub const fn new() -> Self {
        Self
    }
}

impl Sample for Datum {
    type Data = Pgn129044;
    const PGN: u32 = 129044;
    const PRIORITY: u8 = 2;
    const PERIOD_MS: u64 =
        declared_interval_ms(Pgn129044::PGN_129044_DESCRIPTOR.trans_interval, 10_000);

    fn next(&mut self) -> Self::Data {
        let mut datum = Pgn129044::new();
        datum.local_datum = *b"WGS8";
        datum.delta_latitude = 0.0;
        datum.delta_longitude = 0.0;
        datum.delta_altitude = 0.0;
        datum.reference_datum = *b"WGS8";
        datum
    }
}

//================================================================================ 129284

/// Navigation data, counting down to a waypoint then starting over.
pub struct Navigation {
    distance: f64,
}

impl Navigation {
    const START_DISTANCE: f64 = 1000.0;

    pub const fn new() -> Self {
        Self {
            distance: Self::START_DISTANCE,
        }
    }
}

impl Default for Navigation {
    fn default() -> Self {
        Self::new()
    }
}

impl Sample for Navigation {
    type Data = Pgn129284;
    const PGN: u32 = 129284;
    const PRIORITY: u8 = 2;
    const PERIOD_MS: u64 = 100;

    fn next(&mut self) -> Self::Data {
        let mut nav = Pgn129284::new();
        nav.sid = 1;
        nav.distance_to_waypoint = self.distance;
        nav.course_bearing_reference = DirectionReference::Magnetic1;
        nav.perpendicular_crossed = YesNo::No;
        nav.arrival_circle_entered = YesNo::No;
        nav.calculation_type = BearingMode::GreatCircle;
        nav.eta_time = 0.0;
        nav.eta_date = 0;
        nav.bearing_origin_to_destination_waypoint = 45.0;
        nav.bearing_position_to_destination_waypoint = 47.0;
        nav.origin_waypoint_number = 1;
        nav.destination_waypoint_number = 2;
        nav.destination_latitude = 47.65;
        nav.destination_longitude = -2.72;

        self.distance = (self.distance - 10.0).max(0.0);
        if self.distance <= 0.0 {
            self.distance = Self::START_DISTANCE;
        }
        nav
    }
}

//================================================================================ 130310

/// Environmental parameters, water temperature drifting over five degrees.
pub struct Environmental {
    water_temperature: f32,
}

impl Environmental {
    const BASE_TEMPERATURE: f32 = 18.0;

    pub const fn new() -> Self {
        Self {
            water_temperature: Self::BASE_TEMPERATURE,
        }
    }
}

impl Default for Environmental {
    fn default() -> Self {
        Self::new()
    }
}

impl Sample for Environmental {
    type Data = Pgn130310;
    const PGN: u32 = 130310;
    const PRIORITY: u8 = 2;
    const PERIOD_MS: u64 =
        declared_interval_ms(Pgn130310::PGN_130310_DESCRIPTOR.trans_interval, 500);

    fn next(&mut self) -> Self::Data {
        let mut env = Pgn130310::new();
        env.sid = 1;
        env.water_temperature = self.water_temperature;
        env.outside_ambient_air_temperature = 22.0;
        env.atmospheric_pressure = 101325.0;

        self.water_temperature =
            Self::BASE_TEMPERATURE + (self.water_temperature - Self::BASE_TEMPERATURE + 0.1) % 5.0;
        env
    }
}
