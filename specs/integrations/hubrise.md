# HubRise Integration

HubRise is the **interoperability standard** chosen for Captain.Food (order aggregation, POS,
delivery platforms). The Captain.Food domain model is aligned with the HubRise structure, but
**more strongly typed** where it helps (see [Refinements](#refinements-vs-hubrise)).

> 🛡️ **Anti-Corruption Layer (ACL)**: HubRise → domain translation happens at the integration
> boundary. HubRise-only concepts (`SKU`, `option_list`, `"9.80 EUR"` string prices) must **never**
> leak into the domain. The `Ref` scalar (= HubRise `ref`) is the idempotent import key.

Sources: [API Catalogs](https://www.hubrise.com/developers/api/catalogs) ·
[API Locations & Accounts](https://www.hubrise.com/developers/api/accounts) ·
[Catalog concepts](https://www.hubrise.com/docs/catalog)

---

## 1. Data exposed by HubRise

### Location (= a point of sale → our `Restaurant`)
`name`, `address`, `postal_code`, `city`, `country`, `timezone`, `opening_hours` (slots per day,
`HH:mm`), `cutoff_time`, `preparation_time` (min), `order_acceptance` (`normal`/`busy`/`paused`),
attached to an **Account** that carries the `currency`.
❌ **No `phone`/`email`** at Location level.

### Catalog (= our `Menu`)
- **Categories**: tree (`ref`, `parent_ref`, `name`, `description`, `tags`, `image_ids`)
- **Products**: `name`, `description`, `tags`, `image_ids`, `nutrition`, `tax_rate` (triplet
  `delivery`/`collection`/`eat_in`), array of **SKUs**
- **SKUs** (variants): `price` (`"9.80 EUR"`), `price_overrides`, `restrictions`,
  `option_list_ids`, `barcodes`…
- **Option lists / Options**: modifiers (`min/max_selections`, `multiple_selection`,
  `price`, `default`…)
- **Deals**: promotional bundles
- **Inventories**: stock per location (`stock`, `expires_at`)

Continuous sync is possible via **Callbacks** (webhooks) — not only a one-shot import.

---

## 2. HubRise → Captain.Food domain mapping

| HubRise | Domain ([entities.yaml](../entities.yaml) / [scalars.yaml](../scalars.yaml)) | Note |
|---|---|---|
| Location `name` | `Restaurant.displayName` | direct |
| Location `address/postal_code/city/country` | `Restaurant.address` (`Address`) | direct |
| Location `id` | `Restaurant.ref` | idempotent import key |
| Account `currency` | `Restaurant.defaultCurrency` | direct |
| Location `opening_hours` | `Restaurant.openingHours` (`OpeningHoursSlot[]`) | `HH:mm` → `TimeOfDay` |
| Location `timezone` | `Restaurant.timezone` (`TimeZone`) | direct |
| Location `preparation_time` | `Restaurant.preparationTimeMinutes` | direct |
| Location `order_acceptance` | `Restaurant.orderAcceptance` (`OrderAcceptanceMode`) | `normal/busy/paused` → `NORMAL/BUSY/PAUSED` |
| **(none)** `phone`/`email` | `RestaurantContact` (optional) | 🔧 filled manually by the admin |
| Category (`ref`, `parent_ref`, `name`…) | `Category` (`parentRef`) | tree preserved |
| Product (`name`, `description`, `tax_rate`…) | `Product` | `tax_rate` triplet → `TaxRate` |
| Product → SKUs | `Product.variants` (`Variant[]`, min 1) | 1 SKU = 1 `Variant` |
| SKU `price` `"9.80 EUR"` | `Variant.price` (`Money`) | parse + ×100, currency extracted |
| SKU `option_list_ids` | `Variant.optionListIds` | direct |
| Option list / Option | `OptionList` / `Option` | direct |
| Inventory `stock` / `expires_at` | `Variant.stock` (`Stock`) | `stock` → `quantity`, `expires_at` → `expiresAt` |
| SKU `restrictions.enabled` | `Variant.availability` (`MenuItemAvailability`) | `enabled` → `AVAILABLE`/`UNAVAILABLE` |
| Deals | *not modelled* | out of V0 scope |

---

## 3. Refinements vs HubRise

Where Captain.Food is more precise than HubRise, we **keep our model**:

- **`Money`** value object (`amountCents` int + `currency`) instead of the `"9.80 EUR"` string.
  Conversion only at the ACL boundary.
- **`Stock`** explicit + derived **`StockStatus`** (`IN_STOCK`/`LOW_STOCK`/`OUT_OF_STOCK`).
  `LOW_STOCK` = `quantity <= lowStockThreshold` (risk threshold, absent from HubRise).
- **Availability ≠ stock**: `MenuItemAvailability` (manual UI flag) distinct from derived stock status.
- **Strong typing** throughout (one name = one dedicated scalar), `$ref` everywhere.

---

## 4. Gaps / decisions to be aware of

1. **Restaurant contact**: HubRise exposes neither email nor phone at Location level.
   `RestaurantContact` is therefore **optional**; to be completed manually after import.
2. **`ServiceType`**: HubRise = `delivery`/`collection`/`eat_in`. Captain.Food = `DELIVERY`/`COLLECTION`
   (`collection` = pickup); `eat_in` not offered but kept in `TaxRate` for catalog fidelity.
3. **Deals** and advanced **price_overrides**/**restrictions**: not modelled in V0.
4. **Variants**: we adopt the `Product → Variant[]` structure (a simple product = 1 variant).

---

## 5. Import path (events)

Two modes, both going through the ACL:

- **Full import / sync** → event `CatalogImported` (`source: HUBRISE`) carrying
  `categories[]`, `products[]`, `optionLists[]` and **replacing** the catalog content.
- **Inventory sync** (HubRise callback) → targeted `VariantStockUpdated` event, without rewriting the product.

For the restaurant itself, the import feeds the `RegisterRestaurant` command
(then manual contact completion). See the story map: import use case classified **V1**
([../story-map.md](../story-map.md)).
