//! Hand-written `CustomerCompute` — the complex columns of the `Customer` read model (ADR-0040). The
//! generated `project_customer` maps the mechanical columns (customer_id/phone/email/locale/…) inline and
//! calls these hooks for the computed ones. `env.event` is the typed, declared `DomainEvent`; `prev` is the
//! row before this event (the accumulation base).

use crate::projections::{CustomerCompute, CustomerRow, Envelope};
use domain::generated::events::DomainEvent;
use serde_json::{json, Value};

/// The `Customer` read-model projector's hand-written business logic. Stateless — the accumulation base is
/// the previous row passed in by the dispatch.
pub struct CustomerProjector;

/// The current array value of a jsonb accumulation column (or empty if unset / not yet created).
fn array_of(v: Option<&Value>) -> Vec<Value> {
    v.and_then(|x| x.as_array().cloned()).unwrap_or_default()
}

impl CustomerCompute for CustomerProjector {
    /// True once an email magic link has been confirmed; sticky thereafter.
    fn email_verified(&self, prev: Option<&CustomerRow>, env: &Envelope) -> bool {
        match &env.event {
            DomainEvent::CustomerEmailVerified(_) => true,
            _ => prev.map(|r| r.email_verified).unwrap_or(false),
        }
    }

    /// The customer's own submitted ratings, appended from each `RestaurantRated`
    /// (`[{ order_id, restaurant_id, stars, comment, rated_at }]`).
    fn ratings(&self, prev: Option<&CustomerRow>, env: &Envelope) -> Value {
        let mut arr = array_of(prev.map(|r| &r.ratings));
        if let DomainEvent::RestaurantRated(e) = &env.event {
            arr.push(json!({
                "order_id": e.order_id,
                "restaurant_id": e.restaurant_id,
                "stars": e.stars,
                "comment": e.comment,
                "rated_at": env.occurred_at,
            }));
        }
        Value::Array(arr)
    }

    /// The set of favorited restaurant ids — add on `RestaurantFavorited`, remove on `RestaurantUnfavorited`.
    fn favorite_restaurant_ids(&self, prev: Option<&CustomerRow>, env: &Envelope) -> Value {
        let mut ids = array_of(prev.map(|r| &r.favorite_restaurant_ids));
        match &env.event {
            DomainEvent::RestaurantFavorited(e) => {
                let id = json!(e.restaurant_id);
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
            DomainEvent::RestaurantUnfavorited(e) => {
                let id = json!(e.restaurant_id);
                ids.retain(|x| x != &id);
            }
            _ => {}
        }
        Value::Array(ids)
    }

    /// Dietary/cuisine preferences, replaced wholesale by the latest `CustomerPreferencesSet`.
    fn preferences(&self, prev: Option<&CustomerRow>, env: &Envelope) -> Option<Value> {
        match &env.event {
            DomainEvent::CustomerPreferencesSet(e) => Some(json!({
                "dietary_tags": e.dietary_tags,
                "favorite_cuisines": e.favorite_cuisines,
            })),
            _ => prev.and_then(|r| r.preferences.clone()),
        }
    }

    /// The saved address book keyed by `address_id` — upsert on `CustomerAddressSet`, drop on
    /// `CustomerAddressRemoved` (`[{ address_id, label, address }]`).
    fn addresses(&self, prev: Option<&CustomerRow>, env: &Envelope) -> Value {
        let mut arr = array_of(prev.map(|r| &r.addresses));
        match &env.event {
            DomainEvent::CustomerAddressSet(e) => {
                let aid = json!(e.address_id);
                arr.retain(|x| x.get("address_id") != Some(&aid));
                arr.push(json!({ "address_id": e.address_id, "label": e.label, "address": e.address }));
            }
            DomainEvent::CustomerAddressRemoved(e) => {
                let aid = json!(e.address_id);
                arr.retain(|x| x.get("address_id") != Some(&aid));
            }
            _ => {}
        }
        Value::Array(arr)
    }
}

#[cfg(test)]
mod tests {
    //! Fold a realistic event sequence through the GENERATED `project_customer` + this hand-written
    //! `CustomerProjector`, proving the identity mapping, the favorites add/remove accumulation, and the
    //! sticky email-verified flag all work end-to-end.
    use super::*;
    use crate::projections::project_customer;
    use domain::generated::events::{
        CustomerEmailVerified, CustomerRegistered, RestaurantFavorited, RestaurantUnfavorited,
    };
    use domain::generated::scalars::{CustomerId, EmailAddress, PhoneNumber, RestaurantId};

    fn ts(secs: i64) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::from_timestamp(secs, 0).unwrap()
    }
    fn cid() -> CustomerId {
        CustomerId("11111111-1111-1111-1111-111111111111".parse().unwrap())
    }
    fn rid(n: &str) -> RestaurantId {
        RestaurantId(format!("22222222-2222-2222-2222-2222222222{}", n).parse().unwrap())
    }
    fn env(event: DomainEvent, at: i64) -> Envelope {
        Envelope { stream_name: "Customer-1".into(), position: at, occurred_at: ts(at), event }
    }
    fn favorites(row: &CustomerRow) -> Vec<String> {
        row.favorite_restaurant_ids.as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect()
    }

    #[test]
    fn folds_identity_favorites_and_email_verified() {
        let p = CustomerProjector;
        // 1. Registration — mechanical fields mapped by the generator; accumulations start empty.
        let reg = DomainEvent::CustomerRegistered(CustomerRegistered {
            mode: None,
            customer_id: cid(),
            auth_ref: None,
            phone: PhoneNumber("+33600000000".into()),
            display_name: None,
            email: Some(EmailAddress("marco@example.com".into())),
            locale: None,
            timezone: None,
        });
        let row = project_customer(&p, None, &env(reg, 10)).unwrap();
        assert_eq!(row.customer_id, cid()); // generated mechanical mapping
        assert!(!row.email_verified);
        assert!(favorites(&row).is_empty());
        assert_eq!(row.created_at, ts(10));

        // 2. Favorite two restaurants, then unfavorite the first.
        let row = project_customer(&p, Some(row), &env(DomainEvent::RestaurantFavorited(RestaurantFavorited { customer_id: cid(), restaurant_id: rid("01") }), 11)).unwrap();
        let row = project_customer(&p, Some(row), &env(DomainEvent::RestaurantFavorited(RestaurantFavorited { customer_id: cid(), restaurant_id: rid("02") }), 12)).unwrap();
        let row = project_customer(&p, Some(row), &env(DomainEvent::RestaurantUnfavorited(RestaurantUnfavorited { customer_id: cid(), restaurant_id: rid("01") }), 13)).unwrap();
        assert_eq!(favorites(&row), vec!["22222222-2222-2222-2222-222222222202".to_string()]);
        assert_eq!(row.updated_at, ts(13)); // stamped by the generated dispatch
        assert_eq!(row.created_at, ts(10)); // preserved

        // 3. Email verification flips the sticky flag.
        let row = project_customer(&p, Some(row), &env(DomainEvent::CustomerEmailVerified(CustomerEmailVerified { customer_id: cid(), email: EmailAddress("marco@example.com".into()) }), 14)).unwrap();
        assert!(row.email_verified);
    }
}
