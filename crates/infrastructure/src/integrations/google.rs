//! Google Business Profile seam adapters (ADR-0019/0021). Two application ports point at Google:
//! ownership-proof verification (`ClaimRestaurantListing` / `OptOutRestaurantListing`) and the GBP
//! 'Order online' link probe (`VerifyGoogleBusinessProfileOrderLink`). The REAL Google API adapters
//! are TODO(integration); until they land the composition root injects these deliberate stand-ins:
//!
//! - ownership proofs are REJECTED (fail closed — a listing is never silently claimable), and
//! - link probes report `CONFIGURED` (present but unverified), never a false `VERIFIED`.

use application::ports::{GbpOrderLinkProbe, GoogleOwnershipVerifier};
use async_trait::async_trait;
use domain::generated::scalars::{GbpLinkStatus, RestaurantId, WebUrl};
use domain::shared::errors::DomainError;

/// Fail-closed [`GoogleOwnershipVerifier`]: every proof is refused until the real Google Business
/// Profile verification adapter lands, so claims/opt-outs reject with `ListingOwnershipNotVerified`
/// rather than accepting an unchecked proof.
pub struct FailClosedGoogleOwnershipVerifier;

#[async_trait]
impl GoogleOwnershipVerifier for FailClosedGoogleOwnershipVerifier {
    async fn verify(&self, _restaurant_id: RestaurantId, _proof: &str) -> Result<bool, DomainError> {
        // TODO(integration): call the Google Business Profile API to validate the ownership proof.
        Ok(false)
    }
}

/// Unverifying [`GbpOrderLinkProbe`]: reports the link as `CONFIGURED` (never falsely `VERIFIED`)
/// until the real HTTP ping adapter lands.
pub struct UnverifiedGbpOrderLinkProbe;

#[async_trait]
impl GbpOrderLinkProbe for UnverifiedGbpOrderLinkProbe {
    async fn probe(&self, _url: &WebUrl) -> Result<GbpLinkStatus, DomainError> {
        // TODO(integration): ping the configured {slug}.captain.food link and report VERIFIED when live.
        Ok(GbpLinkStatus::CONFIGURED)
    }
}
