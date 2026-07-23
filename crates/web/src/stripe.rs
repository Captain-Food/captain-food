//! Stripe payment element interop (split 3/4 of #21) — the reason checkout is `sdui: false`.
//!
//! Contract boundaries, stated once:
//!
//!   * The client only ever holds the **`clientSecret`** (from `paymentStatus.byOrder` /
//!     `paymentStatusChanged`) and the publishable key — never a secret key, never card data (the
//!     element is a Stripe-hosted iframe; PAN/CVC never touch our DOM, our WASM, or our API).
//!   * The **capture verdict is server truth**: Stripe's webhook lands as the inbound
//!     `PaymentCaptured`/`PaymentFailed` fact and folds into the read models. What
//!     `confirm_payment` returns here only drives immediate UX (show the 3DS sheet, surface a
//!     decline) — the tracking screen resolves the REAL outcome from the platform's own reads.
//!
//! Split like the WS layer: a native-testable description of WHAT we ask of Stripe.js
//! ([`ElementsConfig`] / [`ConfirmParams`]), and a thin `hydrate`-only driver that hands it to the
//! real `Stripe.js` global. The js interop surface is 4 calls: `Stripe(pk)`,
//! `stripe.elements({clientSecret, ...})`, `elements.create('expressCheckout').mount(sel)`,
//! `stripe.confirmPayment({elements, ...})` — kept to the minimum the screen spec names
//! (`stripe_express_checkout_element`, `on_confirm: confirm_payment`).

/// The DOM id the checkout screen renders for the element (`checkout.rs` view) and the driver
/// mounts into — the one string both sides must agree on.
pub const MOUNT_ID: &str = "stripe-payment-element";

/// What we configure `stripe.elements(...)` with. The clientSecret pins the elements instance to
/// THE PaymentIntent the PlaceOrderProcess created — amount/currency live server-side on the
/// intent, so the client cannot even express a different charge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementsConfig {
    pub client_secret: String,
}

/// What `stripe.confirmPayment` is called with. `return_url` is where Stripe lands redirect-based
/// methods (3DS challenges, wallets) — the confirmation route, which re-resolves everything by
/// `orderId` (the whole flow is reload-proof by construction, #12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmParams {
    pub return_url: String,
}

/// The immediate (UX-only) result of a confirm call — see the module docs for why this is NOT the
/// payment verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmUx {
    /// Stripe accepted the confirmation (or took over via redirect) — show "processing" and let
    /// the platform reads deliver the verdict.
    Submitted,
    /// Stripe reported an immediate problem (validation, decline surfaced synchronously). The
    /// message is Stripe's localized user-facing one.
    Declined(String),
}

/// The `hydrate`-only driver over the `Stripe.js` browser global (loaded from a `<script>` tag —
/// Stripe requires their hosted bundle; no npm/wasm packaging of it exists by policy).
#[cfg(all(target_arch = "wasm32", feature = "hydrate"))]
pub mod browser {
    use super::*;
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    extern "C" {
        /// The `Stripe` constructor from https://js.stripe.com/v3/.
        #[wasm_bindgen(js_name = Stripe, catch)]
        fn stripe_js(publishable_key: &str) -> Result<JsStripe, JsValue>;

        type JsStripe;
        #[wasm_bindgen(method, catch)]
        fn elements(this: &JsStripe, options: &JsValue) -> Result<JsElements, JsValue>;
        #[wasm_bindgen(method, js_name = confirmPayment)]
        fn confirm_payment(this: &JsStripe, options: &JsValue) -> js_sys::Promise;

        type JsElements;
        #[wasm_bindgen(method, catch)]
        fn create(this: &JsElements, element_type: &str) -> Result<JsElement, JsValue>;

        type JsElement;
        #[wasm_bindgen(method, catch)]
        fn mount(this: &JsElement, selector: &str) -> Result<(), JsValue>;
    }

    /// A mounted payment element, ready to confirm.
    pub struct PaymentElement {
        stripe: JsStripe,
        elements: JsElements,
    }

    /// Everything the driver can fail with — all of it is "Stripe.js said no", stringified for the
    /// error surface (the user-facing message channel is [`ConfirmUx::Declined`], not this).
    #[derive(Debug, thiserror::Error)]
    #[error("stripe.js: {0}")]
    pub struct StripeJsError(String);

    fn js_err(v: JsValue) -> StripeJsError {
        StripeJsError(v.as_string().unwrap_or_else(|| format!("{v:?}")))
    }

    impl PaymentElement {
        /// `Stripe(pk)` + `elements({clientSecret})` + `create('expressCheckout').mount('#…')` —
        /// the element renders Stripe's wallet/card UI inside [`super::MOUNT_ID`].
        pub fn mount(publishable_key: &str, config: &ElementsConfig) -> Result<Self, StripeJsError> {
            let stripe = stripe_js(publishable_key).map_err(js_err)?;
            let options = js_sys::Object::new();
            js_sys::Reflect::set(
                &options,
                &"clientSecret".into(),
                &config.client_secret.as_str().into(),
            )
            .map_err(js_err)?;
            let elements = stripe.elements(&options).map_err(js_err)?;
            let element = elements.create("expressCheckout").map_err(js_err)?;
            element.mount(&format!("#{}", super::MOUNT_ID)).map_err(js_err)?;
            Ok(Self { stripe, elements })
        }

        /// `stripe.confirmPayment({elements, confirmParams:{return_url}})`. Resolves to the
        /// UX-only verdict — an `error` in the resolution is a synchronous decline; a clean
        /// resolution (or a redirect that never resolves) is `Submitted`.
        pub async fn confirm(&self, params: &ConfirmParams) -> Result<ConfirmUx, StripeJsError> {
            let confirm_params = js_sys::Object::new();
            js_sys::Reflect::set(
                &confirm_params,
                &"return_url".into(),
                &params.return_url.as_str().into(),
            )
            .map_err(js_err)?;
            let options = js_sys::Object::new();
            js_sys::Reflect::set(&options, &"elements".into(), self.elements.as_ref())
                .map_err(js_err)?;
            js_sys::Reflect::set(&options, &"confirmParams".into(), &confirm_params)
                .map_err(js_err)?;

            let resolved =
                wasm_bindgen_futures::JsFuture::from(self.stripe.confirm_payment(&options))
                    .await
                    .map_err(js_err)?;
            // Stripe resolves `{error}` on synchronous failure; anything else means submitted
            // (redirect flows navigate away before resolving at all).
            let decline = js_sys::Reflect::get(&resolved, &"error".into())
                .ok()
                .filter(|e| !e.is_undefined() && !e.is_null())
                .and_then(|e| {
                    js_sys::Reflect::get(&e, &"message".into()).ok().and_then(|m| m.as_string())
                });
            Ok(match decline {
                Some(message) => ConfirmUx::Declined(message),
                None => ConfirmUx::Submitted,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mount_id_is_a_stable_dom_contract() {
        // checkout.rs renders this id; the driver mounts `#<id>`. A rename must break a test.
        assert_eq!(MOUNT_ID, "stripe-payment-element");
    }
}
