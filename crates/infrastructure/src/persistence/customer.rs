//! sqlx read-model repository over the materialized `customer` projection table (ADR-0040). Backs the
//! Customer aggregate's write-side uniqueness/resolution lookups (VerifyPhone register-vs-identify,
//! `PhoneAlreadyInUse`, `EmailAlreadyInUse`) and the `me` / `favoriteRestaurants` GraphQL queries via
//! `application::queries::CustomerReadRepository`.

use application::queries::{CustomerReadRepository, CustomerRow};
use async_trait::async_trait;
use domain::generated::scalars::{CustomerId, EmailAddress, ExternalReference, PhoneNumber};
use domain::shared::errors::DomainError;
use sqlx::PgPool;

use super::customer_store;
use super::db_err;

/// Postgres adapter for the Customer read model.
pub struct PgCustomerRepository {
    pool: PgPool,
}

impl PgCustomerRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn fetch_by(&self, column: &str, value: String) -> Result<Option<CustomerRow>, DomainError> {
        let sql = format!(
            "SELECT {} FROM customer WHERE {} = $1",
            customer_store::COLUMNS,
            column
        );
        let row = sqlx::query(&sql)
            .bind(value)
            .fetch_optional(&self.pool)
            .await
            .map_err(db_err)?;
        row.as_ref().map(customer_store::decode).transpose()
    }
}

#[async_trait]
impl CustomerReadRepository for PgCustomerRepository {
    async fn by_phone(&self, phone: PhoneNumber) -> Result<Option<CustomerRow>, DomainError> {
        self.fetch_by("phone", phone.0).await
    }

    async fn by_email(&self, email: EmailAddress) -> Result<Option<CustomerRow>, DomainError> {
        self.fetch_by("email", email.0).await
    }

    async fn by_id(&self, id: CustomerId) -> Result<Option<CustomerRow>, DomainError> {
        customer_store::load(&self.pool, id).await
    }

    async fn by_auth_ref(&self, auth_ref: ExternalReference) -> Result<Option<CustomerRow>, DomainError> {
        self.fetch_by("auth_ref", auth_ref.0).await
    }
}
