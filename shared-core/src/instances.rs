//! ISO identities to flash on different boards.
//!
//! `IDENTITY_1` to `IDENTITY_5` all ask for the same address, so any two of them
//! collide and settle it by arbitration.
//!
//! The lowest NAME wins. Only `unique_number` varies there, and it sits in the
//! low bits of the NAME, so the order is the obvious one: `IDENTITY_1` beats
//! everyone, `IDENTITY_5` loses to everyone.
//!
//! The strategy decides what the loser does. `IDENTITY_1` and `IDENTITY_2` are
//! `Fixed`, so the loser of that pair has nowhere to go and ends in Cannot
//! Claim. That is what `dual_run_1` and `dual_run_2` are for. `IDENTITY_3` to
//! `IDENTITY_5` are `Arbitrary` and walk the bus instead.
//!
//! Two more identities cover the remaining strategy. No binary uses them.

use korri_n2k::protocol::management::address_claiming::AddressClaimStrategy;

/// The NAME fields a node needs, plus how it goes looking for an address.
pub struct IsoIdentity {
    pub strategy: AddressClaimStrategy<'static>,
    pub unique_number: u32,
    pub manufacturer_code: u16,
    pub device_function: u8,
    pub device_class: u8,
    pub device_instance: u8,
    pub system_instance: u8,
    pub industry_group: u8,
}

impl IsoIdentity {
    /// The Arbitrary Address Capable bit of the NAME.
    ///
    /// Not a free choice. `Arbitrary` needs it set, the other two need it clear,
    /// and `AddressManager::new` rejects any other pairing. Deriving it from the
    /// strategy is what makes that failure unreachable.
    pub const fn is_arbitrary_address_capable(&self) -> bool {
        matches!(self.strategy, AddressClaimStrategy::Arbitrary { .. })
    }
}

/// The address every identity below asks for first.
const PREFERRED_ADDRESS: u8 = 80;

/// Arbitrary Address Capable: start at the preferred address, then walk the bus.
const ARBITRARY: AddressClaimStrategy<'static> = AddressClaimStrategy::Arbitrary {
    preferred: PREFERRED_ADDRESS,
};
const FIXED: AddressClaimStrategy<'static> = AddressClaimStrategy::Fixed {
    preferred: PREFERRED_ADDRESS,
};

/// The short list `IDENTITY_LIST` is allowed to walk.
const SELF_CONFIGURABLE_ADDRESSES: [u8; 3] = [
    PREFERRED_ADDRESS,
    PREFERRED_ADDRESS + 1,
    PREFERRED_ADDRESS + 2,
];

/// Everything the identities share. Only the NAME and the strategy differ.
const fn identity(unique_number: u32, strategy: AddressClaimStrategy<'static>) -> IsoIdentity {
    IsoIdentity {
        strategy,
        unique_number,
        manufacturer_code: 229,
        device_function: 145,
        device_class: 75,
        device_instance: 1,
        system_instance: 0,
        industry_group: 4,
    }
}

/// Highest priority. Keeps the address against all the others.
pub const IDENTITY_1: IsoIdentity = identity(0x1ABCD1, FIXED);

/// Loses to `IDENTITY_1` and, being `Fixed`, cannot move. It ends in Cannot
/// Claim, which is the state `dual_run_2` exists to show.
pub const IDENTITY_2: IsoIdentity = identity(0x1ABCD2, FIXED);

pub const IDENTITY_3: IsoIdentity = identity(0x1ABCD3, ARBITRARY);

pub const IDENTITY_4: IsoIdentity = identity(0x1ABCD4, ARBITRARY);

/// Lowest priority. Gives the address up to any of the others.
pub const IDENTITY_5: IsoIdentity = identity(0x1ABCD5, ARBITRARY);

/// Single Address Capable: one address, no fallback.
/// It goes to Cannot Claim as soon as a stronger NAME takes 80.
/// Spare: same strategy as `IDENTITY_1`, kept for a fourth node.
pub const IDENTITY_FIXED: IsoIdentity = identity(
    0x1ABCE0,
    AddressClaimStrategy::Fixed {
        preferred: PREFERRED_ADDRESS,
    },
);

/// Single Address Capable with a short list to walk.
/// It gives up once the three addresses are taken.
pub const IDENTITY_LIST: IsoIdentity = identity(
    0x1ABCE1,
    AddressClaimStrategy::SelfConfigurable {
        addresses: &SELF_CONFIGURABLE_ADDRESSES,
    },
);
