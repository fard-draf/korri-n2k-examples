pub mod inst1;
pub mod inst2;
pub mod inst3;
pub mod inst4;
pub mod inst5;

pub struct IsoIdentity {
    pub preferred_address: u8,
    pub unique_number: u32,
    pub manufacturer_code: u16,
    pub device_function: u8,
    pub device_class: u8,
    pub device_instance: u8,
    pub system_instance: u8,
    pub industry_group: u8,
}
