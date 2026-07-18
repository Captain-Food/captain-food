//! Supabase Auth seam adapter (ADR-0015). The application's `AuthProviderGateway` port IS the ACL
//! boundary for the wrapped auth provider (passwordless phone-OTP + email magic-link); the REAL
//! `supabase-acl` adapter — Supabase HTTP/SDK calls, Twilio SMS delivery, token semantics — is
//! TODO(integration). Until it lands the composition root injects this deliberate stand-in, exactly as
//! the port contract documents ("sends error, verifications report `Invalid` — never silently accept"):
//!
//! - send operations FAIL with a clear "not configured" error (never pretend an OTP/magic link was
//!   delivered — the caller would wait for a code that cannot arrive), and
//! - verify operations FAIL CLOSED (`Invalid` → the canonical `InvalidVerificationCode` /
//!   `InvalidVerificationToken` rejections), so no identity is ever silently accepted.

use application::ports::{AuthProviderGateway, EmailTokenCheck, PhoneOtpCheck};
use async_trait::async_trait;
use domain::generated::scalars::{
    DialingCode, EmailAddress, EmailVerificationToken, Locale, NationalPhoneNumber, OtpCode,
};
use domain::shared::errors::DomainError;

/// Fail-closed [`AuthProviderGateway`]: sends error ("not configured"), verifications report
/// `Invalid` — so the identity flows reject cleanly until the real Supabase ACL adapter lands.
pub struct FailClosedAuthProviderGateway;

/// The uniform "not configured" send failure.
fn not_configured(what: &str) -> DomainError {
    DomainError::Repository(format!(
        "auth provider not configured — cannot send {what} (supabase-acl adapter pending, ADR-0015)"
    ))
}

#[async_trait]
impl AuthProviderGateway for FailClosedAuthProviderGateway {
    async fn send_phone_otp(
        &self,
        _dialing_code: &DialingCode,
        _national_number: &NationalPhoneNumber,
        _locale: Option<&Locale>,
    ) -> Result<(), DomainError> {
        // TODO(integration): Supabase Auth → Twilio SMS OTP delivery.
        Err(not_configured("phone OTP"))
    }

    async fn verify_phone_otp(
        &self,
        _dialing_code: &DialingCode,
        _national_number: &NationalPhoneNumber,
        _code: &OtpCode,
    ) -> Result<PhoneOtpCheck, DomainError> {
        // TODO(integration): verify the OTP with Supabase Auth and return the provider's authRef.
        Ok(PhoneOtpCheck::Invalid)
    }

    async fn send_email_magic_link(
        &self,
        _email: &EmailAddress,
        _locale: Option<&Locale>,
    ) -> Result<(), DomainError> {
        // TODO(integration): Supabase Auth magic-link email delivery.
        Err(not_configured("email magic link"))
    }

    async fn verify_email_token(
        &self,
        _token: &EmailVerificationToken,
    ) -> Result<EmailTokenCheck, DomainError> {
        // TODO(integration): verify the magic-link token server-side with Supabase Auth.
        Ok(EmailTokenCheck::Invalid)
    }
}
