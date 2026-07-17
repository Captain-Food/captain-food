//! Enum ↔ INTEGER ordinal mapping (ADR-0037): enum columns are stored as the DECLARATION-ORDER ordinal
//! (= `ref_<enum>.sort_order`). One impl per enum the `Restaurant` projection table needs; shared by the
//! read repository (row → `RestaurantRow`) and the projection upsert (row → SQL).
//!
//! The declaration order below MUST match `domain::generated::scalars` (which is generated from
//! `specs/scalars.yaml`, the same source the `ref_*` seed rows come from).

use domain::generated::scalars::{
    CuisineCategory, GbpLinkStatus, OrderAcceptanceMode, RestaurantListingStatus, RestaurantStatus,
};
use domain::shared::errors::DomainError;

/// i32 ordinal ↔ domain enum, in declaration order.
pub trait EnumOrd: Sized {
    fn to_ord(&self) -> i32;
    fn from_ord(ord: i32) -> Result<Self, DomainError>;
}

macro_rules! enum_ord {
    ($ty:ident { $($variant:ident => $ord:literal),+ $(,)? }) => {
        impl EnumOrd for $ty {
            fn to_ord(&self) -> i32 {
                match self { $( $ty::$variant => $ord, )+ }
            }
            fn from_ord(ord: i32) -> Result<Self, DomainError> {
                match ord {
                    $( $ord => Ok($ty::$variant), )+
                    other => Err(DomainError::Repository(format!(
                        "unknown {} ordinal {other}", stringify!($ty)
                    ))),
                }
            }
        }
    };
}

enum_ord!(RestaurantStatus { DRAFT => 0, ACTIVE => 1, INACTIVE => 2 });
enum_ord!(RestaurantListingStatus { NON_PARTNER => 0, PASSIVE_PARTNER => 1, ACTIVE_PARTNER => 2 });
enum_ord!(GbpLinkStatus { UNSET => 0, CONFIGURED => 1, VERIFIED => 2, BROKEN => 3 });
enum_ord!(OrderAcceptanceMode { NORMAL => 0, BUSY => 1, PAUSED => 2 });
enum_ord!(CuisineCategory {
    FAST_FOOD => 0,
    PIZZA => 1,
    TRADITIONAL => 2,
    BISTRONOMIC => 3,
    FOOD_TRUCK => 4,
});

/// `to_ord` through an `Option` (nullable enum column).
pub fn opt_to_ord<E: EnumOrd>(v: &Option<E>) -> Option<i32> {
    v.as_ref().map(EnumOrd::to_ord)
}

/// `from_ord` through an `Option` (nullable enum column).
pub fn opt_from_ord<E: EnumOrd>(ord: Option<i32>) -> Result<Option<E>, DomainError> {
    ord.map(E::from_ord).transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinals_round_trip_in_declaration_order() {
        for (ord, v) in [
            (0, RestaurantStatus::DRAFT),
            (1, RestaurantStatus::ACTIVE),
            (2, RestaurantStatus::INACTIVE),
        ] {
            assert_eq!(v.to_ord(), ord);
            assert_eq!(RestaurantStatus::from_ord(ord).unwrap(), v);
        }
        assert_eq!(RestaurantListingStatus::ACTIVE_PARTNER.to_ord(), 2);
        assert_eq!(OrderAcceptanceMode::PAUSED.to_ord(), 2);
        assert_eq!(CuisineCategory::FOOD_TRUCK.to_ord(), 4);
        assert_eq!(GbpLinkStatus::BROKEN.to_ord(), 3);
        assert!(RestaurantStatus::from_ord(99).is_err());
    }
}
