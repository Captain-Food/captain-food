import type { Model } from '../model.ts';
import { collectRefs, parseRef, refName } from '../refs.ts';

/**
 * Bounded-context resolution shared by the documentation emitters.
 *
 * The product documentation is organized **top-level by bounded context** (c4-l2): every domain
 * artifact (actor, view, command, event, entity, scalar, error, output type, API op, test,
 * observability contract) is attributed to exactly one context, falling back to `cross-cutting` when it
 * belongs to none or spans several. The attribution is DERIVED from the actor wiring + c4-l2, never
 * hard-coded, so it tracks the specs:
 *
 *  - actor   → its c4-l2 bounded context (aggregates / processManagers refs)
 *  - view    → the context of its source aggregate (reference views are cross-cutting)
 *  - command → the context of its handling actor
 *  - event   → the context of its emitting actor (else its first consumer; else cross-cutting)
 *  - error   → the context of the command(s) that throw it (single → that context, else cross-cutting)
 *  - type    → the context of the view(s) it reads
 *  - entity  → its own aggregate's context if the name is an aggregate; otherwise the single context of
 *              all artifacts that reference it (else cross-cutting)
 *  - scalar  → the single context of every artifact that references it (else cross-cutting)
 */

export const CROSS = 'cross-cutting';

export interface ContextMap {
  /** Bounded-context ids in declared order, with `cross-cutting` appended last. */
  order: string[];
  /** Human description for a context id (synthetic for `cross-cutting`). */
  describe(ctx: string): string;
  ofActor(name: string): string;
  ofView(name: string): string;
  ofCommand(name: string): string;
  ofEvent(name: string): string;
  ofEntity(name: string): string;
  ofScalar(name: string): string;
  ofError(name: string): string;
  ofType(name: string): string;
  ofReads(reads: string[]): string;
  /**
   * Context for a READ operation (query/subscription), by WHO performs it — reads/discovery are caller-
   * facing (browsing restaurants is a `customer` activity even though it reads restaurant data). Each role
   * maps to a context via the `roles` declared on c4-l2 bounded contexts (ADMIN/EXTERNAL/RIDER are
   * neutral). Exactly one distinct performer context → that context; zero or several → `fallback` (the
   * read model's context, so shared-role reads like `orders` stay in their domain context).
   * MUTATIONS do NOT use this — they are attributed by their handling aggregate's context (`ofCommand`).
   */
  ofOperation(roles: string[], fallback: string): string;
}

const arr = (v: unknown): unknown[] => (Array.isArray(v) ? v : []);

export function buildContextMap(model: Model): ContextMap {
  const defs = model.defs;
  const l2 = (defs['architecture/c4-l2.yaml'] ?? {}) as Record<string, Record<string, unknown>>;
  const l2bc = (l2.boundedContexts ?? {}) as Record<string, Record<string, unknown>>;
  const order = [...Object.keys(l2bc), CROSS];
  const descriptions = new Map<string, string>(
    Object.entries(l2bc).map(([id, bc]) => [id, String(bc.description ?? '')]),
  );
  descriptions.set(CROSS, 'Shared vocabulary and operations that span several bounded contexts (or belong to none).');

  // actor → context, straight from the c4-l2 aggregate / process-manager refs.
  const actorCtx = new Map<string, string>();
  // role (UserType) → context, from the optional `roles` on each bounded context (drives op grouping).
  const roleCtx = new Map<string, string>();
  for (const [cid, bc] of Object.entries(l2bc)) {
    for (const ref of [...arr(bc.aggregates), ...arr(bc.processManagers)]) {
      const n = refName((ref as { $ref?: string })?.$ref ?? '');
      if (n) actorCtx.set(n, cid);
    }
    for (const role of arr(bc.roles)) if (typeof role === 'string') roleCtx.set(role, cid);
  }

  // actor-wiring indexes (the minimum needed to attribute commands/events/errors).
  const cmdActor = new Map<string, string>(); // command → handling actor
  const evtEmitter = new Map<string, string>(); // event → first emitting actor
  const evtConsumer = new Map<string, string>(); // event → first consuming actor
  const errCmds = new Map<string, Set<string>>(); // error → commands that throw it
  for (const a of model.actors) {
    for (const e of a.receives) {
      const msg = refName(e.message.$ref);
      if (e.message.$ref.startsWith('commands.yaml#/') && msg) {
        cmdActor.set(msg, a.name);
        for (const t of e.throws) {
          const er = refName(t.$ref);
          if (er) (errCmds.get(er) ?? errCmds.set(er, new Set()).get(er)!).add(msg);
        }
      } else if (e.message.$ref.startsWith('events.yaml#/') && msg && !evtConsumer.has(msg)) {
        evtConsumer.set(msg, a.name);
      }
      for (const em of e.emits) {
        const ev = refName(em.$ref);
        if (ev && !evtEmitter.has(ev)) evtEmitter.set(ev, a.name);
      }
    }
  }

  const ofActor = (name: string): string => actorCtx.get(name) ?? CROSS;
  const ofView = (name: string): string => {
    const v = model.views.find((x) => x.name === name);
    return !v || v.reference ? CROSS : ofActor(v.aggregate);
  };
  const ofReads = (reads: string[]): string => (reads[0] ? ofView(reads[0]) : CROSS);
  const ofCommand = (name: string): string => {
    const a = cmdActor.get(name);
    return a ? ofActor(a) : CROSS;
  };
  const ofEvent = (name: string): string => {
    const a = evtEmitter.get(name) ?? evtConsumer.get(name);
    return a ? ofActor(a) : CROSS;
  };
  const ofType = (name: string): string => {
    const t = model.api.types.find((x) => x.name === name);
    return t ? ofReads(t.reads) : CROSS;
  };
  const single = (s: Set<string> | undefined): string => (s && s.size === 1 ? [...s][0]! : CROSS);
  const ofError = (name: string): string => {
    const cmds = errCmds.get(name);
    if (!cmds) return CROSS;
    const ctxs = new Set([...cmds].map(ofCommand).filter((c) => c !== CROSS));
    return single(ctxs);
  };

  // --- entities & scalars: attribute by usage across the strongly-anchored artifacts --------------
  const scalarNames = new Set(Object.keys(defs['scalars.yaml']));
  const entityNames = Object.keys(defs['entities.yaml']);
  const entityVotes = new Map<string, Set<string>>();
  const scalarVotes = new Map<string, Set<string>>();
  const vote = (m: Map<string, Set<string>>, name: string, ctx: string) => {
    if (!name || ctx === CROSS) return;
    (m.get(name) ?? m.set(name, new Set()).get(name)!).add(ctx);
  };
  const voteRefs = (def: unknown, ctx: string) => {
    if (ctx === CROSS) return;
    for (const occ of collectRefs(def, '')) {
      const p = parseRef(occ.ref);
      const name = p?.path[0];
      if (!name) continue;
      if (p!.file === 'scalars.yaml') vote(scalarVotes, name, ctx);
      else if (p!.file === 'entities.yaml' || p!.file === '') vote(entityVotes, name, ctx);
    }
  };

  // primary anchored artifacts vote for the entities/scalars they reference
  for (const c of Object.keys(defs['commands.yaml'])) voteRefs(defs['commands.yaml'][c], ofCommand(c));
  for (const ev of Object.keys(defs['events.yaml'])) voteRefs(defs['events.yaml'][ev], ofEvent(ev));
  for (const er of Object.keys(defs['errors.yaml'])) voteRefs(defs['errors.yaml'][er], ofError(er));
  for (const t of model.api.types) {
    const ctx = ofType(t.name);
    for (const f of t.properties) if (f.ref) vote(scalarNames.has(f.type) ? scalarVotes : entityVotes, f.type, ctx);
  }
  for (const q of [...model.api.queries, ...model.api.subscriptions]) {
    const ctx = q.reads.length ? ofReads(q.reads) : ofType(q.returnsType);
    for (const a of q.args) if (a.ref) vote(scalarNames.has(a.type) ? scalarVotes : entityVotes, a.type, ctx);
  }
  for (const m of model.api.mutations) {
    const ctx = ofCommand(m.command);
    for (const f of m.payload) if (f.ref) vote(scalarNames.has(f.type) ? scalarVotes : entityVotes, f.type, ctx);
  }
  for (const v of model.views) {
    const ctx = ofView(v.name);
    for (const col of v.columns) if (scalarNames.has(col.type)) vote(scalarVotes, col.type, ctx);
  }

  // resolve entity context: aggregate-name match wins, else a single usage context
  const entityCtx = new Map<string, string>();
  for (const e of entityNames) entityCtx.set(e, actorCtx.has(e) ? actorCtx.get(e)! : single(entityVotes.get(e)));
  // anchored entities propagate their context to the entities & scalars they reference (one pass)
  for (const e of entityNames) {
    const ctx = entityCtx.get(e)!;
    if (ctx !== CROSS) voteRefs(defs['entities.yaml'][e], ctx);
  }
  for (const e of entityNames) if (entityCtx.get(e) === CROSS) entityCtx.set(e, single(entityVotes.get(e)));

  const scalarCtx = new Map<string, string>();
  for (const s of scalarNames) scalarCtx.set(s, single(scalarVotes.get(s)));

  const ofOperation = (roles: string[], fallback: string): string => {
    const performer = new Set(roles.map((r) => roleCtx.get(r)).filter((c): c is string => !!c));
    return performer.size === 1 ? [...performer][0]! : fallback;
  };

  return {
    order,
    describe: (ctx) => descriptions.get(ctx) ?? '',
    ofActor,
    ofView,
    ofCommand,
    ofEvent,
    ofEntity: (name) => entityCtx.get(name) ?? CROSS,
    ofScalar: (name) => scalarCtx.get(name) ?? CROSS,
    ofError,
    ofType,
    ofReads,
    ofOperation,
  };
}
