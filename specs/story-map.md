# Captain.Food – Story Map (Jeff Patton)

User story map following Jeff Patton's method.
- **Backbone** = the *activities*, read left to right as a narrative.
- **User tasks** = the steps under each activity (the walking skeleton).
- **Release slices** = horizontal lines splitting work into versions (V0 = walking skeleton, then V1+).

> ⚠️ This map **interleaves 3 personas** — Admin (supply), Customer (demand), Restaurant (fulfilment) —
> in the order their steps must exist for an order to complete.
> Strictly, Patton would draw one map per persona; for a single-flow V0, interleaving is enough.
> (Personas are story-mapping participants — not to be confused with the actor-model *actors*
> in [actors.yaml](actors.yaml), the in-domain aggregates/process-managers a command reaches.)

> 🧭 **CQRS link**: a write story triggers a **use case**, hence a **command**
> (see [commands.yaml](commands.yaml)). A command may emit **several events**; commands are NOT
> the mirror of events. Read stories (👁️) generate no command.
>
> 📥 **Not every event comes from a command**: a third-party system (Stripe, HubRise, a delivery
> partner) may just **inform** us that something already happened on their side (Stripe payment
> outcome, HubRise inventory sync, delivery partner status). There is nothing to validate or reject, so these **inbound (integration) events**
> are recorded as facts directly — no command. Rule of thumb: can the originator be told *"no"*? →
> command. Already happened, just reported? → inbound event. See CQRS methodology in `CLAUDE.md`.

> 🔐 **Accounts are required** (Uber Eats model): a customer must verify a phone number to check out.
> Identification is **passwordless phone OTP** (SMS one-time code), delegated to **Supabase Auth**;
> only `CustomerRegistered` is a domain event — OTP send/verify and sessions are provider concerns
> and never hit the event log. No password ⇒ no "lost password" flow. Returning customers are
> offered a **passkey (Face ID/Touch ID)** to re-authenticate without an SMS.

---

## Backbone (left → right)

```
[Admin]    [Admin]   [Customer] [Customer]  [Customer]   [Customer]  [Restaurant] [Resto/Courier] [Customer]
  1          2          3          4            5            6            7             8              9
Onboard → Publish → Discover → Browse &  → Verify     → Pay &     →  Fulfil    →  Deliver /   → Track &
the resto  the catalog   restos     build cart   phone (OTP)  place order  the order    hand over      complete
```

---

## Activities × personas × steps

The backbone above reads as a narrative; this table makes the **persona ↔ story ↔ steps** association
explicit (the columns Patton draws per persona, decomposed into user-task cards). Each step links to
its write side in [commands.yaml](commands.yaml) / [events.yaml](events.yaml), or is a pure read
(👁️) / provider concern (🔑) / inbound fact (📥). The Persona(s) column names the persona that drives
the step (matching the command's `actor` field in [commands.yaml](commands.yaml)). Personas are the
story-mapping participants — distinct from the actor-model aggregates in [actors.yaml](actors.yaml),
which are the in-domain handlers a persona's command reaches.

Legend: ✍️ = command · 👁️ = read model · 📥 = inbound event · 🔑 = auth provider · ⏱️ = post-V0

| # | Activity (story) | Persona(s) | Steps (user tasks) | Write (command / 📥 inbound) |
|---|---|---|---|---|
| 1 | Onboard the resto | Admin | Register a restaurant (starts DRAFT) | ✍️ `RegisterRestaurant` |
| | | Admin | Make it visible & orderable | ✍️ `ActivateRestaurant` |
| | | Admin | Edit / take offline ⏱️ | ✍️ `UpdateRestaurant` · `DeactivateRestaurant` |
| 2 | Publish the catalog | Admin | Create a catalog (catalog) | ✍️ `CreateCatalog` |
| | | Admin | Add a product with its offers | ✍️ `AddProduct` |
| | | Admin | Categories, options, edit/remove ⏱️ | ✍️ `AddCatalogCategory` · `AddOptionList` · `UpdateProduct` · `RemoveProduct` … |
| | | Admin / System | Manage stock ⏱️ (manual **or** HubRise sync) | ✍️ `UpdateOfferStock` · 📥 `OfferStockUpdated` |
| | | System | Import / re-sync a full catalog ⏱️ | ✍️ `ImportCatalog` |
| 3 | Discover | Customer | Browse the list of restaurants | 👁️ `read_restaurants_public` |
| | | Customer | Search / filter / ratings ⏱️ | 👁️ |
| 4 | Browse & build cart | Customer | Open a restaurant + view its catalog | 👁️ |
| | | Customer (guest) | Add / change / remove cart lines (validated per line) | ✍️ `AddCartLine` · `ChangeCartLineQuantity` · `RemoveCartLine` |
| | | Customer | View the priced cart | 👁️ |
| | | Customer | Choose options / modifiers ⏱️ | 👁️ |
| 5 | Verify phone (OTP) | Customer / Auth | Enter phone → SMS code → verify | 🔑 (Supabase Auth) |
| | | Customer | First-time number creates the account | ✍️ `RegisterCustomer` → `CustomerRegistered` |
| | | Customer / Auth | Enrol passkey (biometric re-auth) ⏱️ | 🔑 |
| 6 | Pay & place order | Customer | Confirm contact + service mode | 👁️ |
| | | Customer | Pay (card / Apple Pay) & place order | ✍️ `PlaceOrder` → `PaymentIntentCreated`, `OrderPlaced` |
| | | Stripe | Payment outcome reported back | 📥 `PaymentCaptured` / `PaymentFailed` |
| 7 | Fulfil | Restaurant | Accept the order | ✍️ `AcceptOrder` |
| | | Restaurant | Reject the order (+ refund) | ✍️ `RejectOrder` · 📥 `PaymentRefunded` |
| | | Restaurant | Start preparation ⏱️ · busy/paused mode ⏱️ | ✍️ `StartPreparation` · `ChangeOrderAcceptanceMode` |
| 8 | Deliver / hand over | Restaurant | Mark order ready | ✍️ `MarkOrderReady` |
| | | Restaurant / Courier | Mark delivered / handed over | ✍️ `MarkOrderDelivered` |
| | | Delivery partner | Partner delivery status ⏱️ | 📥 `DeliveryStatusUpdated` |
| 9 | Track & complete | Customer | Track order status (polling) | 👁️ |
| | | Customer | Cancel before the restaurant accepts (+ refund) | ✍️ `CancelOrderByCustomer` · 📥 `PaymentRefunded` |
| | | Restaurant | Cancel after acceptance ⏱️ (+ refund) | ✍️ `CancelOrderByRestaurant` · 📥 `PaymentRefunded` |

---

## The map, split into release slices

Legend: ✍️ = command (write) · 👁️ = read model (no command) · 📥 = inbound event from an external system (recorded as a fact, no command) · 🔑 = auth provider (Supabase Auth, no domain command) · ⏱️ = post-V0

| Activity | 🟢 **V0 — Walking skeleton** | 🔵 V1 / later |
|---|---|---|
| **1. Onboard the resto** | Register a resto ✍️ `RegisterRestaurant` · Activate ✍️ `ActivateRestaurant` | Self-onboarding via `restos.captain.food` ⏱️ · Deactivate ✍️ · HubRise import ✍️ `ImportCatalog` ⏱️ · Custom domain ⏱️ |
| **2. Publish the catalog** | Create a catalog ✍️ `CreateCatalog` · Add products ✍️ `AddProduct` | Update/remove product ✍️ · Categories & options ✍️ · Manage stock/availability ✍️ `UpdateOfferStock` (manual) **or** 📥 `OfferStockUpdated` (HubRise sync) · Photos ⏱️ |
| **3. Discover** | List restaurants 👁️ `read_restaurants_public` | Search/filter by category 👁️ · Ratings & badges ⏱️ |
| **4. Browse & build cart** | View resto + catalog 👁️ · Build cart ✍️ `AddCartLine` / `ChangeCartLineQuantity` / `RemoveCartLine` (server-side `Cart` aggregate, guest-allowed, validated per line) | Choose options/modifiers ⏱️ · Cross-device cart sync ⏱️ |
| **5. Verify phone (OTP)** | Enter phone → SMS code → verify 🔑 · First-time number creates account ✍️ `RegisterCustomer` → `CustomerRegistered` | **Enrol passkey — Face ID/Touch ID, biometric re-auth, skips SMS** 🔑 ⏱️ · Social login (Google/Apple/Facebook) 🔑 ⏱️ · Optional email for receipts ⏱️ · Manage profile & saved addresses ✍️ ⏱️ |
| **6. Pay & place order** | Confirm contact + mode 👁️ · Pay via Stripe — **card + Apple Pay** (Express Checkout Element) + place order ✍️ `PlaceOrder` (saga; Stripe outcome arrives as 📥 `PaymentCaptured` / `PaymentFailed`) | Google Pay ⏱️ · Stripe Connect 3-way ⏱️ |
| **7. Fulfil** | Accept ✍️ `AcceptOrder` · Reject ✍️ `RejectOrder` (+refund) | Start preparation ✍️ `StartPreparation` *(event to create)* · Busy/paused mode ✍️ · HubRise ⏱️ |
| **8. Deliver / hand over** | Mark ready ✍️ `MarkOrderReady` · Delivered/handed over ✍️ `MarkOrderDelivered` | Request partner delivery ✍️ ⏱️ · partner status (`OUT_FOR_DELIVERY`…) 📥 `DeliveryStatusUpdated` ⏱️ · DeliveryJob ⏱️ |
| **9. Track & complete** | Tracking page (polling) 👁️ · Cancel before accept ✍️ `CancelOrderByCustomer` | Real-time subscription ⏱️ · Timeline ⏱️ · Restaurant cancel post-accept ✍️ |

> Admin and Restaurant back-offices also require authentication (roles `admin`, `restaurant`),
> handled by the same Supabase Auth setup. Not drawn here to keep the customer narrative readable.

---

## V0 walking skeleton (thinnest end-to-end path)

Read left to right, this is the minimal story that proves value:

1. An **admin** registers a resto (`RegisterRestaurant`), creates a catalog (`CreateCatalog`) with at least one product (`AddProduct`), then activates it (`ActivateRestaurant`).
2. A **customer** sees the resto in the list, opens the catalog, and builds the cart — a server-side `Cart` aggregate validated line by line (`AddCartLine`…), so a guest gets immediate feedback on stock/options before checkout.
3. To check out, they **verify their phone number** via an SMS one-time code (Supabase Auth). A new number creates the account (`RegisterCustomer` → `CustomerRegistered`); a known one just signs in. Account is required.
4. They confirm contact details, pick delivery/collection, **pay via Stripe** and place the order (`PlaceOrder`).
5. The **restaurant** accepts (`AcceptOrder`) or rejects (`RejectOrder`), marks ready (`MarkOrderReady`), then delivered (`MarkOrderDelivered`).
6. The **customer** tracks the status in near real-time (polling).

Everything blue (🔵) can wait without breaking this story.

---

## What the map reveals

1. **Heavily read/write asymmetric system** → validates the CQRS-light choice: many read models, few commands. Columns 3 and 9 are almost pure reads (column 4 became a write zone once the cart moved server-side).
2. **V0 backend effort concentrates on 5 write zones**: supply setup (1-2), cart building (4), customer registration (5), `PlaceOrder` (6), order lifecycle (7-8).
3. **Auth is mostly NOT domain logic** — it's delegated to Supabase Auth. Only `CustomerRegistered` is a domain event; this keeps the event log clean of credentials/sessions.
4. **`PlaceOrder` (column 6) is the risk point**: the only skeleton story that depends on an external persona (Stripe). Treat it as a **saga** + a dedicated technical spike.
5. **Not all events come from commands** (📥): payment, HubRise-sync and delivery-partner events are **inbound integration events** — external systems reporting facts that already happened. They are ingested through the ACL and recorded directly (no command, no rejection), which keeps the integration/saga boundaries explicit. Watch the request/report split: a command may *request* an action (refund) while the *fact* it succeeded (`PaymentRefunded`) is an inbound event.

---

## Gaps to resolve before implementation

- **`StartPreparation`**: the `PREPARING` status exists in `OrderStatus` but no event produces it. Add `OrderPreparationStarted` or drop the status from V0.
- **Refund after `RejectOrder`**: no refund event is modelled yet.
- **Payment saga**: failure branch (`PaymentFailed`) and the order aggregate's birth order vs Stripe capture — see the `PlaceOrder` saga.
- **Customer ↔ auth identity link**: phone number is the primary identifier; define how a Supabase Auth user id maps to the `Customer` aggregate (`CustomerRegistered.authRef`) and how a returning number resolves to the existing `Customer`.
- **SMS provider**: phone OTP requires an SMS provider behind Supabase Auth (Twilio / Vonage / MessageBird) — a per-message cost and a new external integration to pick.
- **Passkeys vs subdomains**: biometric re-auth uses WebAuthn passkeys (Supabase Auth, beta), scoped to a Relying Party ID with **up to 5 allowed origins**. But customers order on per-restaurant subdomains (`{slug}.captain.food`). Decide to route identification/checkout through a **single origin** (e.g. `captain.food`) with RP ID = `captain.food`, so one passkey works everywhere instead of one per subdomain.
- **HubRise** (resto + catalog import): use case *UC-import*, classified V1; see [integrations/hubrise.md](integrations/hubrise.md).
- **Cart persistence & guest sessions**: the `Cart` is a server-side aggregate, so every line edit is an event. Carts **persist indefinitely** (Uber Eats style — no abandonment/expiry use case, hence no sweeper); the open question is the storage footprint of long-lived, high-churn carts — whether cart events live in the durable `domain_events` log or a dedicated cart store. Also: cart mutations are `@public` (guest, pre-auth) — bind sessions to a cart token so one guest cannot mutate another's cart.
