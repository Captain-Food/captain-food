# Captain.Food – Architecture Overview

## 1. Goals

Captain.Food is a local-first food ordering and delivery platform for independent restaurants and food trucks.

Primary goals for V0:
- Validate product–market fit in one city (Tours) with a minimal but robust stack.
- Support a high-quality mobile web UX for customers.
- Keep the backend architecture evolvable towards full CQRS + event sourcing.
- Integrate cleanly with existing restaurant tools via HubRise (order aggregation, POS, delivery platforms). 
- Avoid over-engineering (no premature microservices, no premature infra complexity). 

## 2. Scope – V0

For V0 we only ship:

- Customer-facing app:
  - Browse restaurants and menus.
  - Place orders (delivery or pickup).
  - Pay online via Stripe.
  - Track order status.

- Admin app (for founder/ops):
  - Onboard restaurants (create/update restaurants and menus).
  - View/manage orders.
  - Manage restaurant subdomains.
  - Basic monitoring and troubleshooting.

Restaurant and courier UIs will be added later as separate apps, but V0 assumes:
- Restaurants may still work via their existing systems, integrated through HubRise.
- Delivery can be fulfilled through external partners (e.g. Avelo37) via their APIs where possible.

## 3. Domains and Subdomains

Top-level domains:

- `captain.food`  
  - Public marketing + customer web app.

- `restos.captain.food`  
  - Restaurant onboarding + restaurant dashboard (later).

- `riders.captain.food`  
  - Courier documentation and portal (later).

- `system.captain.food`  
  - Internal admin/back-office for Captain.Food operations.

- `api.captain.food`  
  - GraphQL API, exposed under per-role paths (`/public`, `/customer`, `/restaurant`, `/rider`,
    `/admin`, `/external`). See §8 for the role-by-path model.

Per-restaurant subdomains:

- Pattern: `{restaurantSlug}.captain.food`
  - Each restaurant gets a dedicated subdomain for its ordering page.
  - Example: `marco.captain.food`, `sushitime.captain.food`.
  - Later, a custom domain (e.g. `marco.restaurant` or `monresto.fr`) can point to the same tenant.

DNS assumptions:
- Wildcard `*.captain.food` points to the same hosting environment.
- Tenant resolution is done at runtime based on the `Host` header.

Identification & checkout origin:
- Customers browse menus on per-restaurant subdomains, but **identification (phone OTP) and
  checkout must run on a single origin** — `captain.food` — rather than the restaurant subdomain.
- Reason: biometric re-authentication uses WebAuthn passkeys (Supabase Auth), which bind to a
  Relying Party ID (`captain.food`) with a small, fixed number of allowed origins (≤5). Enrolling
  passkeys per `{slug}.captain.food` would not scale and would force re-enrolment per restaurant.
- Consequence: the checkout flow (cart → identify → pay) redirects from the restaurant subdomain
  to `captain.food` (carrying the cart/restaurant context), so one passkey works everywhere.
- The bare-domain RP ID also covers subdomains for the SMS-OTP path, keeping a consistent
  identity across the whole `*.captain.food` space.

## 4. Monorepo structure

We use a monorepo managed by Turborepo (or Nx). All apps and shared packages live in a single repository.

```text
captain-food/
  apps/
    web-client/       # Customer app – Next.js
    web-admin/        # Internal admin – Next.js
    api/              # Backend API – Node.js + GraphQL
    # future:
    # web-restaurant/
    # web-delivery/
  packages/
    ui/               # Shared React components
    types/            # Shared TypeScript domain types
    config/           # Shared ESLint, Prettier, Tailwind, tsconfig
```

Primary tech choices:
- Frontend: Next.js (App Router), React, TypeScript, Tailwind CSS.
- Backend: Node.js (TypeScript) with a minimal framework (Hono or NestJS).
- Database: PostgreSQL (managed, e.g. Supabase).
- API: GraphQL as the primary read/write interface between frontend and backend.
- Events: append-only `domain_events` table in Postgres.

## 5. Backend Architecture (CQRS-light + Event Log)

We use a pragmatic CQRS + event-log approach:

- **Commands**:
  - Expressed as application services / mutations.
  - Validate invariants, then write domain events into an append-only `domain_events` table.

- **Event log**:
  - Single table `domain_events`:
    - `id`, `aggregate_type`, `aggregate_id`, `version`, `type`, `payload`, `occurred_at`, `metadata`.

- **Read models**:
  - We do NOT query `domain_events` directly for the main user flows.
  - Instead, we maintain dedicated read tables, e.g.:
    - `read_orders_by_restaurant`
    - `read_orders_by_customer`
    - `read_restaurants_public`
  - These read tables are updated by projections that consume events and apply transformations.

- **GraphQL**:
  - All frontends use the GraphQL API.
  - Queries read from read tables.
  - Mutations create commands that write events and then update read models.

We are NOT doing full-blown event sourcing with complex snapshots and replay logic at V0.  
We are simply:
- Writing events into `domain_events`.
- Maintaining a small number of read tables for the main flows.

## 6. Core Aggregates and Events (V0)

Aggregates:

- Restaurant
- Menu / MenuItem
- Order
- Customer
- DeliveryJob (optional, depending on integration with delivery partners)

Events (examples):

Restaurant:
- `RestaurantRegistered`
- `RestaurantUpdated`
- `RestaurantActivated`
- `RestaurantDeactivated`

Menu:
- `MenuCreated`
- `MenuItemAdded`
- `MenuItemUpdated`
- `MenuItemRemoved`

Order:
- `OrderPlaced`
- `OrderAcceptedByRestaurant`
- `OrderRejectedByRestaurant`
- `OrderMarkedReady`
- `OrderHandedToCourier`
- `OrderDelivered`
- `OrderCancelledByCustomer`
- `OrderCancelledByRestaurant`

Payments:
- `PaymentIntentCreated`
- `PaymentCaptured`
- `PaymentFailed`
- `PayoutCreated` (when Stripe Connect transfers to restaurant / courier are done)

Delivery integration (if needed in V0):
- `DeliveryRequestedFromPartner`
- `DeliveryAcceptedByPartner`
- `DeliveryStatusUpdated`

V0 will only implement the subset strictly needed to:
- Place an order.
- Accept / reject it.
- Mark it ready and delivered.
- Record successful payments.

## 7. Integrations

- **Stripe**:
  - Used for collecting customer payments.
  - Eventually used with Stripe Connect for 3-way split (restaurant, courier, platform), but V0 can start with a simplified 2-party flow.

- **HubRise**:
  - Used to integrate with existing restaurant systems (Uber Eats, Deliveroo, POS).
  - Our backend will push orders into HubRise where appropriate, and listen for updates when needed.

- **Delivery partner (e.g. Avelo37)**:
  - If an API is available, the backend may:
    - Create delivery jobs.
    - Receive status updates.
  - Otherwise, delivery can be handled manually in V0.

- **Supabase Auth (identity)**:
  - Customer identification is passwordless: phone number + SMS one-time code (OTP).
  - Returning customers may enrol a passkey (WebAuthn / Face ID / Touch ID) to re-authenticate
    without an SMS. Passkeys are bound to RP ID `captain.food` (see Domains, single-origin checkout).
  - Identity is a provider concern; only `CustomerRegistered` is a domain event.

- **SMS provider (e.g. Twilio / Vonage / MessageBird)**:
  - Sends the OTP codes behind Supabase Auth. Per-message cost.

## 8. Hosting and API Management (V0)

V0 hosting assumptions:
- Frontend apps (Next.js) deployed on Vercel (or equivalent).
- Backend API deployed either:
  - as Vercel serverless functions, or
  - as a small Node container on a managed platform (Fly.io, Render, etc.).
- Database: PostgreSQL managed (Supabase or equivalent).

API protection — role = path, one schema per role:
- It is **one app and one master GraphQL schema** ([specs/schema.graphql](specs/schema.graphql)), but
  it is exposed under **per-role paths**, and the caller's role is established by the path:
  - `api.captain.food/public/graphql`     → `PUBLIC`     (anonymous visitor)
  - `api.captain.food/customer/graphql`   → `CUSTOMER`   (authenticated end user)
  - `api.captain.food/restaurant/graphql` → `RESTAURANT` (restaurant staff)
  - `api.captain.food/rider/graphql`       → `RIDER`      (delivery partner / courier)
  - `api.captain.food/admin/graphql`       → `ADMIN`      (platform operator)
  - `api.captain.food/external/graphql`    → `EXTERNAL`   (third-party ACL: HubRise, Avelo37, …)
- The **schema served on each path is generated** from the master by filtering on the `@auth` /
  `@public` directives: `schema(role) = { fields whose @auth.requires includes that role } ∪ { @public fields }`.
  So a role physically cannot see or call a field outside its path-schema (small attack surface), and
  there is a single source of truth (no hand-maintained per-role schemas → no drift).
- Each path is **protected, observed and rate-limited independently** at the edge:
  - HTTPS only; JWT (Supabase Auth) verified per path — `/admin` behind SSO/allowlist, `/external`
    IP-allowlisted to partner ranges + partner API keys, `/public` heavily rate-limited & cacheable.
  - Per-path metrics/dashboards fall out naturally (traffic, latency, errors by audience).
- **Stripe stays a REST webhook** (Stripe controls the payload); HubRise/Avelo37 ingestion uses
  `/external/graphql` as the Anti-Corruption-Layer surface. Internal jobs (projections, schedulers)
  call the command bus **in-process**, not GraphQL — there is no `SYSTEM` GraphQL path.
- Fine-grained ownership (e.g. "this order is mine") remains a **server-side** check in the command
  invariants ([specs/commands.yaml](specs/commands.yaml)); the path only gates the role *type*.