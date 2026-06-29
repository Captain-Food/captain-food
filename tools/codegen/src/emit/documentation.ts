import type { ApiField, Model, SchemaNode } from '../model.ts';
import type { Derived } from '../validate.ts';
import { refName } from '../refs.ts';
import { buildContextMap, CROSS } from './contexts.ts';

/**
 * Emit `documentation.generated.md` — a single, fully detailed, NAVIGABLE product documentation built
 * from the specs. Every item (story, operation, type, actor, view, command, event, entity, scalar,
 * error) gets an anchored subsection with its description and cross-links to the items it relates to,
 * so the whole product can be understood without reading code.
 */

const USER_TYPE_EMOJI: Record<string, string> = {
  PUBLIC: '🌐', CUSTOMER: '🙋', RESTAURANT_ACCOUNT: '🏪', RESTAURANT: '🍽️',
  RIDER: '🛵', ADMIN: '🛠️', EXTERNAL: '🔌',
};

// One consistent emoji per kind, used in EVERY header and cross-link so each concept reads the same
// colour wherever it appears.
const KIND_EMOJI: Record<string, string> = {
  scalar: '🔤', entity: '📦', command: '📩', event: '⚡', view: '🗄️', actor: '🎭',
  type: '🧩', query: '🔎', mutation: '✏️', error: '⛔', property: '🔹',
  story: '🎬', activity: '🧭', test: '🧪', obs: '📡', context: '🔲', container: '🧱', component: '⚙️',
  subscription: '🔔',
};
const emo = (kind: string) => KIND_EMOJI[kind] ?? '•';

// --- anchors & links --------------------------------------------------------------------------
// Anchors are EXPLICIT (`<a id>`), so emoji in the visible header never affects the link target.
const slug = (s: string) => s.toLowerCase().replace(/[^a-z0-9_]+/g, '-');
const pascal = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
const anchor = (kind: string, name: string) => `${kind}-${slug(name)}`;
const propAnchor = (kind: string, owner: string, field: string) => `${anchor(kind, owner)}--${slug(field)}`;
const idTag = (id: string) => `<a id="${id}"></a>`;
const link = (kind: string, name: string) => `[${emo(kind)} \`${name}\`](#${anchor(kind, name)})`;
// Link to a specific PROPERTY of an owner (e.g. an event field): clickable, deep navigation.
const propLink = (kind: string, owner: string, field: string) =>
  `[${emo(kind)} \`${owner}\`.\`${field}\`](#${propAnchor(kind, owner, field)})`;
const itemHead = (kind: string, label: string, name: string) =>
  `${idTag(anchor(kind, name))}\n#### ${emo(kind)} ${label}: \`${name}\``;

function mdTable(header: string[], rows: string[][]): string {
  if (!rows.length) return '';
  const line = (cells: string[]) => `| ${cells.join(' | ')} |`;
  return [line(header), line(header.map(() => '---')), ...rows.map(line)].join('\n');
}

const push = (m: Map<string, string[]>, k: string, v: string) => {
  if (!m.has(k)) m.set(k, []);
  if (!m.get(k)!.includes(v)) m.get(k)!.push(v);
};

export function emitDocumentation(model: Model, derived: Derived): string {
  const defs = model.defs;
  const scalarSet = new Set(Object.keys(defs['scalars.yaml']));
  const entitySet = new Set(Object.keys(defs['entities.yaml']));
  const typeSet = new Set(model.api.types.map((t) => t.name));
  const desc = (file: keyof typeof defs, name: string): string =>
    String((defs[file][name] as Record<string, unknown> | undefined)?.description ?? '').trim().replace(/\s+/g, ' ');

  // --- relationship indexes (derived once from the actor wiring + views) --------------------------
  const cmdHandler = new Map<string, { actor: string; emits: string[]; throws: string[] }>();
  const evtEmittedBy = new Map<string, string[]>();
  const evtConsumedBy = new Map<string, string[]>();
  const errThrownBy = new Map<string, string[]>();
  for (const actor of model.actors) {
    for (const e of actor.receives) {
      const msg = refName(e.message.$ref);
      const emits = e.emits.map((r) => refName(r.$ref)).filter((n): n is string => !!n);
      const throws = e.throws.map((r) => refName(r.$ref)).filter((n): n is string => !!n);
      if (e.message.$ref.startsWith('commands.yaml#/') && msg) {
        cmdHandler.set(msg, { actor: actor.name, emits, throws });
        for (const er of throws) push(errThrownBy, er, msg);
      } else if (e.message.$ref.startsWith('events.yaml#/') && msg) {
        push(evtConsumedBy, msg, actor.name);
      }
      for (const ev of emits) push(evtEmittedBy, ev, actor.name);
    }
  }
  const evtViews = new Map<string, string[]>();
  for (const v of model.views) for (const r of v.fedBy) { const n = refName(r.$ref); if (n) push(evtViews, n, v.name); }
  // command → the mutation that dispatches it; event → views; type → reads
  const mutByCommand = new Map(model.api.mutations.map((m) => [m.command, m.name]));

  // --- type labels (link a referenced type to its anchored subsection) ----------------------------
  const refLabel = (ref: string): string => {
    const [file, name] = [ref.split('#/')[0], ref.split('#/')[1] ?? ''];
    if (file === 'scalars.yaml') return link('scalar', name);
    return link('entity', name); // entities.yaml or local '#/...' in entities
  };
  const rawType = (p: SchemaNode): string => {
    const n = p as Record<string, unknown>;
    if (typeof n.$ref === 'string') return refLabel(n.$ref);
    if (n.type === 'array') return `[${rawType(n.items as SchemaNode)}]`;
    let t = `\`${String(n.type ?? '?')}\``;
    if (Array.isArray(n.enum)) t += ` (${(n.enum as string[]).join(' \\| ')})`;
    if (typeof n.format === 'string') t += ` _${n.format}_`;
    return t;
  };
  const apiType = (f: ApiField): string => {
    let base: string;
    if (f.ref) base = scalarSet.has(f.type) ? link('scalar', f.type) : typeSet.has(f.type) ? link('type', f.type) : entitySet.has(f.type) ? link('entity', f.type) : `\`${f.type}\``;
    else base = `\`${f.type}\`${f.format ? ` _${f.format}_` : ''}`;
    return f.array ? `[${base}]` : base;
  };
  // Each field gets an explicit `<a id>` so it is a clickable navigation target (e.g. a view column's
  // `from` links straight to the event property it copies).
  const propRows = (def: SchemaNode, kind: string, owner: string): string[][] => {
    const props = (def.properties ?? {}) as Record<string, SchemaNode>;
    const required = new Set(Array.isArray(def.required) ? (def.required as string[]) : []);
    return Object.entries(props).map(([n, p]) => {
      const pn = p as Record<string, unknown>;
      const req = required.has(n) ? '✅' : '⬜';
      return [`${idTag(propAnchor(kind, owner, n))}\`${n}\``, rawType(p), req, String(pn.description ?? '').replace(/\s+/g, ' ')];
    });
  };

  // ============================================================================================
  // 1. STORY MAP
  // ============================================================================================
  const storiesSection = model.personas.map((p) => {
    const badge = `${USER_TYPE_EMOJI[p.role] ?? '❔'} \`${p.role}\`${p.locale ? ` · 🗣️ \`${p.locale}\`` : ''}`;
    const rows: string[][] = [];
    for (const act of p.activities) {
      act.steps.forEach((step, i) => {
        const op = step.op && step.opKind ? link(step.opKind, step.op) : step.note ? `📝 ${step.note}` : '—';
        rows.push([i === 0 ? `${emo('activity')} **${act.name}**` : '', step.name, op]);
      });
    }
    return `${idTag(anchor('story', p.name))}\n### ${emo('story')} \`${p.name}\` · ${badge}\n${p.description ? `\n${p.description}\n` : ''}\n${mdTable(['Activity', 'Step', 'Operation'], rows)}`;
  }).join('\n\n');

  // ============================================================================================
  // The whole doc is organized TOP-LEVEL by bounded context (c4-l2); each item is attributed to one.
  // ============================================================================================
  const cx = buildContextMap(model);
  type Doc = { ctx: string; md: string }; // a rendered item tagged with its bounded context
  const inCtx = (docs: Doc[], ctx: string) => docs.filter((d) => d.ctx === ctx);

  // 2. API operations — tagged by bounded context (queries + mutations + subscriptions).
  const apiDocs: Doc[] = [];
  for (const q of model.api.queries) {
    const fieldList = q.args.map((a) => `\`${a.name}${a.required ? '' : '?'}\`: ${apiType(a)}`).join(', ');
    const input = q.args.length ? `- **Input**: 🧩 \`${pascal(q.name)}QueryInput${q.args.some((a) => a.required) ? '!' : ''}\` — ${fieldList}` : `- **Input**: _(none)_`;
    const ret = `${typeSet.has(q.returnsType) || entitySet.has(q.returnsType) ? link(typeSet.has(q.returnsType) ? 'type' : 'entity', q.returnsType) : `\`${q.returnsType}\``}${q.returnsList ? ' (list)' : ''}`;
    const reads = q.reads.map((v) => link('view', v)).join(', ') || '—';
    apiDocs.push({ ctx: cx.ofOperation(q.roles, q.reads.length ? cx.ofReads(q.reads) : cx.ofType(q.returnsType)), md: [
      itemHead('query', 'Query', q.name), q.description ? `\n${q.description}\n` : '',
      input, `- **Returns**: ${ret} · **reads** ${reads}`, `- **Roles**: ${q.roles.join(', ')} · **slice** ${q.slice}`,
    ].join('\n') });
  }
  for (const m of model.api.mutations) {
    const payload = m.payload.map((f) => `\`${f.name}\`: ${apiType(f)}`).join(', ');
    const h = cmdHandler.get(m.command);
    // A mutation belongs to the context of the aggregate/PM that HANDLES its command (declared in c4-l2),
    // not the performer — e.g. placeOrder/rateOrder are customer-performed but live in the order context.
    apiDocs.push({ ctx: cx.ofCommand(m.command), md: [
      itemHead('mutation', 'Mutation', m.name),
      `\n- **Command**: ${link('command', m.command)}${h ? ` → handled by ${link('actor', h.actor)}` : ''}`,
      `- **Roles**: ${m.roles.join(', ')} · **slice** ${m.slice}`,
      `- **Payload**: correlationId${payload ? `, ${payload}` : ''}`,
    ].join('\n') });
  }
  for (const s of model.api.subscriptions) {
    const fieldList = s.args.map((a) => `\`${a.name}${a.required ? '' : '?'}\`: ${apiType(a)}`).join(', ');
    const input = s.args.length ? `- **Input**: 🧩 \`${pascal(s.name)}SubscriptionInput${s.args.some((a) => a.required) ? '!' : ''}\` — ${fieldList}` : `- **Input**: _(none)_`;
    const ret = `${typeSet.has(s.returnsType) || entitySet.has(s.returnsType) ? link(typeSet.has(s.returnsType) ? 'type' : 'entity', s.returnsType) : `\`${s.returnsType}\``}${s.returnsList ? ' (list)' : ''}`;
    apiDocs.push({ ctx: cx.ofOperation(s.roles, cx.ofType(s.returnsType)), md: [
      `${idTag(anchor('subscription', s.name))}\n#### ${emo('subscription')} Subscription: [\`${s.name}\`](#${anchor('subscription', s.name)})`,
      s.description ? `\n${s.description}\n` : '', input, `- **Streams**: ${ret}`, `- **Roles**: ${s.roles.join(', ')} · **slice** ${s.slice}`,
    ].join('\n') });
  }

  const typeDocs: Doc[] = model.api.types.map((t) => {
    const reads = t.reads.map((v) => link('view', v)).join(', ');
    const rows = t.properties.map((f) => [`${idTag(propAnchor('type', t.name, f.name))}\`${f.name}\``, apiType(f), f.nullable ? '⬜' : '✅']);
    return { ctx: cx.ofType(t.name), md: [
      itemHead('type', 'Type', t.name),
      t.description ? `\n${t.description}\n` : '',
      reads ? `- **Read model**: ${reads}` : '- **Read model**: _(resolved within a parent projection)_',
      rows.length ? `\n${mdTable(['Field', 'Type', 'Required'], rows)}` : '',
    ].join('\n') };
  });

  // ============================================================================================
  // 3. ACTORS
  // ============================================================================================
  const actorDocs: Doc[] = model.actors.map((a) => {
    const rows = a.receives.map((e) => {
      const msgName = refName(e.message.$ref) ?? '?';
      const isCmd = e.message.$ref.startsWith('commands.yaml#/');
      const msg = link(isCmd ? 'command' : 'event', msgName);
      const emits = e.emits.map((r) => link('event', refName(r.$ref) ?? '')).join(', ') || (e.effect ? `_${e.effect}_` : '—');
      const throws = e.throws.map((r) => link('error', refName(r.$ref) ?? '')).join(', ') || '—';
      return [msg, emits, throws];
    });
    const kind = a.type === 'aggregate' ? '🧩 aggregate' : '⚙️ process manager';
    return { ctx: cx.ofActor(a.name), md: [
      itemHead('actor', 'Actor', a.name),
      `\n_${kind}_${a.description ? ` — ${a.description}` : ''}\n`,
      mdTable(['Receives', 'Emits →', 'Throws'], rows),
    ].join('\n') };
  });

  // ============================================================================================
  // 4. VIEWS (read models)
  // ============================================================================================
  const viewDocs: Doc[] = model.views.map((v) => {
    const slice = v.slice === 'V1' ? '🔭 V1' : '🛶 V0';
    const fedBy = v.fedBy.map((r) => link('event', refName(r.$ref) ?? '')).join(', ') || '—';
    const cols = v.columns.map((c) => {
      const flags = [c.pk && 'PK', c.unique && 'unique', c.index && 'index', c.nullable && 'nullable'].filter(Boolean).join(', ') || '—';
      const fk = c.fk ? ` → ${link('view', c.fk.split('.')[0] ?? c.fk)}` : '';
      // Link the column's type to its scalar subsection; SQL primitives (text/jsonb/…) stay plain.
      const typeCell = `${scalarSet.has(c.type) ? link('scalar', c.type) : `\`${c.type || '?'}\``}${c.typeDerived ? ' _(derived)_' : ''}`;
      // Lineage: link straight to the source event property (or the whole event for derived columns).
      const source = (c.from ?? []).map((ref) => {
        const segs = (ref.split('#/')[1] ?? '').split('/').filter(Boolean);
        const ev = segs[0] ?? '';
        const prop = segs[1] === 'properties' ? segs[2] : undefined;
        return prop ? propLink('event', ev, prop) : link('event', ev);
      }).join(', ') || '⚠️ _(none)_';
      return [`\`${c.name}\``, `${typeCell}${fk}`, source, flags, (c.note ?? '').replace(/\s+/g, ' ')];
    });
    return { ctx: cx.ofView(v.name), md: [
      itemHead('view', 'View', v.name),
      `\n- **Source**: ${v.reference ? '📦 reference (static seed)' : link('actor', v.aggregate)} · ${slice}${v.internal ? ' · 🔒 internal' : ''}`,
      v.note ? `- **Note**: ${v.note.replace(/\s+/g, ' ')}` : '',
      v.filters.length ? `- **Filters**: ${v.filters.join(' ')}` : '',
      v.rules.length ? `- **Rules**: ${v.rules.join(' ')}` : '',
      `- **Fed by**: ${fedBy}`,
      `\n${mdTable(['Column', 'Type', 'Sourced from', 'Constraints', 'Notes'], cols)}`,
    ].filter(Boolean).join('\n') };
  });

  // ============================================================================================
  // 5. COMMANDS
  // ============================================================================================
  const commandDocs: Doc[] = Object.keys(defs['commands.yaml'])
    .filter((c) => cmdHandler.has(c)) // skip command value objects (not handled by an actor)
    .map((c) => {
      const h = cmdHandler.get(c)!;
      const mut = mutByCommand.get(c);
      const rows = propRows(defs['commands.yaml'][c] ?? {}, 'command', c);
      return { ctx: cx.ofCommand(c), md: [
        itemHead('command', 'Command', c),
        desc('commands.yaml', c) ? `\n${desc('commands.yaml', c)}\n` : '',
        `- **Dispatched by**: ${mut ? link('mutation', mut) : '—'} · **handled by** ${link('actor', h.actor)}`,
        `- **Emits**: ${h.emits.map((e) => link('event', e)).join(', ') || '—'}`,
        `- **Throws**: ${h.throws.map((e) => link('error', e)).join(', ') || '—'}`,
        rows.length ? `\n${mdTable(['Field', 'Type', 'Required', 'Description'], rows)}` : '',
      ].join('\n') };
    });

  // ============================================================================================
  // 6. EVENTS
  // ============================================================================================
  const nonProjected = new Set(model.nonProjectedEvents);
  const eventDocs: Doc[] = Object.keys(defs['events.yaml']).map((ev) => {
    const rows = propRows(defs['events.yaml'][ev] ?? {}, 'event', ev);
    const projected = (evtViews.get(ev) ?? []).map((v) => link('view', v)).join(', ')
      || (nonProjected.has(ev) ? '_non-projected (saga/transient)_' : '—');
    return { ctx: cx.ofEvent(ev), md: [
      itemHead('event', 'Event', ev),
      desc('events.yaml', ev) ? `\n${desc('events.yaml', ev)}\n` : '',
      `- **Emitted by**: ${(evtEmittedBy.get(ev) ?? []).map((a) => link('actor', a)).join(', ') || '_inbound / external_'}`,
      `- **Consumed by**: ${(evtConsumedBy.get(ev) ?? []).map((a) => link('actor', a)).join(', ') || '—'}`,
      `- **Projected into**: ${projected}`,
      rows.length ? `\n${mdTable(['Field', 'Type', 'Required', 'Description'], rows)}` : '',
    ].join('\n') };
  });

  // ============================================================================================
  // 7. ENTITIES (value objects & aggregates)
  // ============================================================================================
  const entityDocs: Doc[] = Object.keys(defs['entities.yaml']).map((e) => {
    const rows = propRows(defs['entities.yaml'][e] ?? {}, 'entity', e);
    return { ctx: cx.ofEntity(e), md: [
      itemHead('entity', 'Entity', e),
      desc('entities.yaml', e) ? `\n${desc('entities.yaml', e)}\n` : '',
      rows.length ? mdTable(['Field', 'Type', 'Required', 'Description'], rows) : '_(no fields)_',
    ].join('\n') };
  });

  // ============================================================================================
  // 8. SCALARS
  // ============================================================================================
  type Row = { ctx: string; cells: string[] };
  const scalarRows: Row[] = Object.entries(defs['scalars.yaml']).map(([name, d]) => {
    const n = d as Record<string, unknown>;
    let t = String(n.type ?? '?');
    if (Array.isArray(n.enum)) t = `enum (${(n.enum as string[]).join(' \\| ')})`;
    else if (typeof n.format === 'string') t += ` _${n.format}_`;
    else if (typeof n.pattern === 'string') t += ` \`${n.pattern}\``;
    return { ctx: cx.ofScalar(name), cells: [`${idTag(anchor('scalar', name))}${emo('scalar')} \`${name}\``, t, String(n.description ?? '').replace(/\s+/g, ' ')] };
  });

  // ============================================================================================
  // 9. ERRORS (referenced by command `throws`)
  // ============================================================================================
  const errorRows: Row[] = Object.entries(defs['errors.yaml']).map(([name, d]) => {
    const n = d as Record<string, unknown>;
    const msgs = (n.messages as Record<string, unknown> | undefined) ?? {};
    const en = (msgs.en as string | undefined) ?? '';
    const fr = (msgs.fr as string | undefined) ?? '';
    const by = (errThrownBy.get(name) ?? []).map((c) => link('command', c)).join(', ') || '—';
    return { ctx: cx.ofError(name), cells: [`${idTag(anchor('error', name))}${emo('error')} \`${name}\``, String(n.description ?? '').replace(/\s+/g, ' '), `🇬🇧 ${en}`, `🇫🇷 ${fr}`, by] };
  });

  // ============================================================================================
  // 10. TESTS (behaviour Given/When/Then — grouped by the aggregate under test)
  // ============================================================================================
  const testDocs: Doc[] = (() => {
    const tDefs = (defs['tests.yaml'] ?? {}) as Record<string, Record<string, SchemaNode>>;
    const fixtures = (tDefs.fixtures ?? {}) as Record<string, { type?: { $ref?: string } }>;
    const tests = (tDefs.tests ?? {}) as Record<string, Record<string, unknown>>;
    const fxEvent = (ref: unknown): string | null => {
      const key = typeof ref === 'string' ? ref.split('/').pop() ?? '' : '';
      const evRef = fixtures[key]?.type?.$ref;
      return typeof evRef === 'string' ? refName(evRef) : null;
    };
    const evLinks = (arr: unknown): string =>
      (Array.isArray(arr) ? arr : []).map((it) => { const e = fxEvent((it as { $ref?: string })?.$ref); return e ? link('event', e) : '—'; }).join(', ');

    return model.actors.map((a): Doc | null => {
      const entries = Object.entries(tests).filter(([, t]) => refName((t.actor as { $ref?: string })?.$ref ?? '') === a.name);
      if (!entries.length) return null;
      const cases = entries.map(([name, t]) => {
        const cmd = refName(((t.when as { type?: { $ref?: string } })?.type)?.$ref ?? '') ?? '?';
        const given = Array.isArray(t.given) && t.given.length ? evLinks(t.given) : '_(none)_';
        const hasThrown = Object.prototype.hasOwnProperty.call(t, 'thrown');
        const thenArr = Array.isArray(t.then) ? t.then : [];
        const then = hasThrown
          ? ''
          : `- **Then**: ${thenArr.length ? evLinks(thenArr) : '∅ _no event (idempotent no-op)_'}`;
        const thrown = hasThrown
          ? `- **Thrown**: ${(t.thrown as Array<{ $ref?: string }>).map((r) => link('error', refName(r.$ref ?? '') ?? '')).join(', ') || '—'}`
          : '';
        return [
          `${idTag(anchor('test', name))}\n#### ${emo('test')} Test: \`${name}\``,
          t.name ? `\n_${String(t.name)}_\n` : '',
          `- **Given**: ${given}`,
          `- **When**: ${link('command', cmd)}`,
          then,
          thrown,
        ].filter(Boolean).join('\n');
      }).join('\n\n');
      return { ctx: cx.ofActor(a.name), md: `**${link('actor', a.name)}**\n\n${cases}` };
    }).filter((d): d is Doc => d !== null);
  })();

  // Link any `$ref` to its anchored subsection, picking the kind from the target file.
  const anyLink = (ref: unknown): string => {
    if (typeof ref !== 'string') return '—';
    const [file, name] = [ref.split('#/')[0], ref.split('#/')[1] ?? ''];
    const kind = file === 'commands.yaml' ? 'command' : file === 'events.yaml' ? 'event'
      : file === 'actors.yaml' ? 'actor' : file === 'views.yaml' ? 'view'
      : file === 'scalars.yaml' ? 'scalar' : 'entity';
    return link(kind, name);
  };
  const refList = (arr: unknown): string =>
    (Array.isArray(arr) ? arr : []).map((r) => anyLink((r as { $ref?: string })?.$ref)).join(', ') || '—';

  // ============================================================================================
  // 10. OBSERVABILITY (workflow contracts)
  // ============================================================================================
  const obsDocs: Doc[] = Object.entries((defs['observability.yaml'] ?? {}) as Record<string, Record<string, unknown>>).map(([feature, c]) => {
    const wf = (c.workflow ?? {}) as Record<string, unknown>;
    const ids = (Array.isArray(c.run_identity) ? c.run_identity : []) as Array<Record<string, unknown>>;
    const idRows = ids.map((i) => [`\`${String(i.name)}\``, `\`${String(i.source ?? '')}\``, i.required ? '✅' : '⬜', i.businessKey ? anyLink((i.businessKey as { $ref?: string }).$ref) : '—']);
    const spans = (Array.isArray(c.spans) ? c.spans : []) as Array<Record<string, unknown>>;
    const spanRows = spans.map((s) => {
      const attrs = (Array.isArray(s.attributes) ? s.attributes : []) as Array<Record<string, unknown>>;
      const a = attrs.map((x) => `\`${String(x.key)}\`${x.required ? '*' : ''}`).join(', ') || '—';
      return [`\`${String(s.name)}\``, `\`${String(s.kind ?? '')}\``, s.required ? '✅' : '⬜', s.multiplicity ? `\`${String(s.multiplicity)}\`` : '—', a];
    });
    const metricList = (key: string) => ((Array.isArray(c[key]) ? c[key] : []) as Array<Record<string, unknown>>).map((m) => `\`${String(m.name)}\` _(${String(m.type)})_`).join(', ') || '—';
    const sr = (c.status_rules ?? {}) as Record<string, Record<string, unknown>>;
    const lat = (c.latency_budget ?? {}) as Record<string, unknown>;
    const err = (c.error_budget ?? {}) as Record<string, unknown>;
    const success = sr.success ? `success ⇐ spans [${(sr.success.required_spans as string[] ?? []).map((s) => `\`${s}\``).join(', ')}]` : '';
    const cmd = refName((wf.command as { $ref?: string })?.$ref ?? '');
    const saga = refName((wf.saga as { $ref?: string })?.$ref ?? '');
    const ctx = cmd ? cx.ofCommand(cmd) : saga ? cx.ofActor(saga) : CROSS;
    return { ctx, md: [
      `${idTag(anchor('obs', feature))}\n#### ${emo('obs')} Contract: \`${feature}\``,
      `\n_criticality: **${String(c.criticality ?? '—')}**_\n`,
      `- **Workflow**: ${wf.saga ? `saga ${anyLink((wf.saga as { $ref?: string }).$ref)}` : ''}${wf.command ? ` · command ${anyLink((wf.command as { $ref?: string }).$ref)}` : ''}`,
      `- **Emits**: ${refList(wf.emits)} · **Inbound**: ${refList(wf.inbound)}`,
      idRows.length ? `\n**Run identity**\n\n${mdTable(['Id', 'Source', 'Req.', 'Business key'], idRows)}` : '',
      spanRows.length ? `\n**Spans** (\`*\` = required attribute)\n\n${mdTable(['Span', 'Kind', 'Req.', 'Multiplicity', 'Attributes'], spanRows)}` : '',
      `\n- **Metrics**: ${metricList('metrics')} · **Business metrics**: ${metricList('business_metrics')}`,
      success ? `- **Status rules**: ${success}` : '',
      `- **SLOs**: p95 ≤ ${String(lat.max_p95_ms ?? '—')}ms · p99 ≤ ${String(lat.max_p99_ms ?? '—')}ms · error rate ≤ ${String(err.max_error_rate_pct ?? '—')}%`,
    ].filter(Boolean).join('\n') };
  });

  // ============================================================================================
  // 11. ARCHITECTURE (C4 L2/L3)
  // ============================================================================================
  const c4Doc = (() => {
    const l2 = (defs['architecture/c4-l2.yaml'] ?? {}) as Record<string, Record<string, unknown>>;
    const l3 = (defs['architecture/c4-l3.yaml'] ?? {}) as Record<string, unknown>;
    const sys = (l2.system ?? {}) as Record<string, unknown>;
    const bcs = (l2.boundedContexts ?? {}) as Record<string, Record<string, unknown>>;
    const containers = (l2.containers ?? {}) as Record<string, Record<string, unknown>>;
    const externals = (l2.externalSystems ?? {}) as Record<string, Record<string, unknown>>;
    const rels = (Array.isArray(l2.relationships) ? l2.relationships : []) as Array<Record<string, unknown>>;
    const comps = (l3.components ?? {}) as Record<string, Record<string, unknown>>;

    const bcRows = Object.entries(bcs).map(([n, bc]) => [`${emo('context')} \`${n}\``, String(bc.description ?? ''), `${refList(bc.aggregates)}${bc.processManagers ? ` · ${refList(bc.processManagers)}` : ''}`]);
    const cRows = Object.entries(containers).map(([n, c]) => [`${emo('container')} \`${n}\``, String(c.technology ?? ''), `${String(c.description ?? '')}${c.realizes ? `<br>realizes: ${refList(c.realizes)}` : ''}`]);
    const xRows = Object.entries(externals).map(([n, x]) => [`🔌 \`${n}\``, String(x.description ?? '')]);
    const relRows = rels.map((r) => [`\`${String(r.from)}\` → \`${String(r.to)}\``, String(r.description ?? '')]);
    const compRows = Object.entries(comps).map(([n, c]) => {
      const bind = c.handles ? `handles ${refList(c.handles)}` : c.updates ? `updates ${refList(c.updates)}` : '—';
      return [`${emo('component')} \`${n}\``, c.instrumented ? '📡 yes' : '— no', String(c.description ?? ''), bind];
    });
    return [
      `**System**: \`${String(sys.name ?? 'Captain.Food')}\` — ${String(sys.description ?? '')}`,
      `\n### 🔲 L2 — Bounded contexts\n\n${mdTable(['Context', 'Description', 'Aggregates / process managers'], bcRows)}`,
      `\n### 🧱 L2 — Containers\n\n${mdTable(['Container', 'Technology', 'Description'], cRows)}`,
      `\n### 🔌 L2 — External systems\n\n${mdTable(['System', 'Description'], xRows)}`,
      `\n### ➡️ L2 — Relationships\n\n${mdTable(['Edge', 'Description'], relRows)}`,
      `\n### ⚙️ L3 — Components of the \`api\` container\n\n${mdTable(['Component', 'Instrumented', 'Description', 'Binds'], compRows)}`,
    ].join('\n');
  })();

  const sec = (id: string, emoji: string, title: string) => `${idTag('sec-' + id)}\n## ${emoji} ${title}`;

  // --- assemble each bounded context as a TOP-LEVEL section (## 🔲), one subsection (###) per kind ---
  const kindSub = (emoji: string, title: string, bodies: string[]) =>
    bodies.length ? `### ${emoji} ${title} _(${bodies.length})_\n\n${bodies.join('\n\n')}` : '';
  const docSub = (emoji: string, title: string, docs: Doc[], ctx: string) =>
    kindSub(emoji, title, inCtx(docs, ctx).map((d) => d.md));
  const rowSub = (emoji: string, title: string, head: string[], rows: Row[], ctx: string) => {
    const r = rows.filter((x) => x.ctx === ctx);
    return r.length ? `### ${emoji} ${title} _(${r.length})_\n\n${mdTable(head, r.map((x) => x.cells))}` : '';
  };
  const ctxBlocks = cx.order.map((ctx) => {
    const parts = [
      docSub('🧰', 'API operations', apiDocs, ctx),
      docSub(emo('type'), 'Output types', typeDocs, ctx),
      docSub(emo('actor'), 'Actors', actorDocs, ctx),
      docSub(emo('view'), 'Views (read models)', viewDocs, ctx),
      docSub(emo('command'), 'Commands', commandDocs, ctx),
      docSub(emo('event'), 'Events', eventDocs, ctx),
      docSub(emo('entity'), 'Entities', entityDocs, ctx),
      rowSub(emo('scalar'), 'Scalars', ['Scalar', 'Type', 'Description'], scalarRows, ctx),
      rowSub(emo('error'), 'Errors', ['Error', 'Description', 'Message (en)', 'Message (fr)', 'Thrown by'], errorRows, ctx),
      docSub(emo('test'), 'Tests', testDocs, ctx),
      docSub(emo('obs'), 'Observability', obsDocs, ctx),
    ].filter(Boolean);
    return { ctx, parts };
  }).filter((b) => b.parts.length);

  const ctxSections = ctxBlocks.map(({ ctx, parts }, i) =>
    `${idTag('sec-ctx-' + slug(ctx))}\n## ${emo('context')} ${i + 1}. ${ctx}\n\n${cx.describe(ctx) ? `_${cx.describe(ctx)}_\n\n` : ''}${parts.join('\n\n')}`,
  ).join('\n\n');
  const ctxToc = ctxBlocks.map(({ ctx }) => `[${emo('context')} ${ctx}](#sec-ctx-${slug(ctx)})`).join(' · ');

  return `<!-- GENERATED by tools/codegen — do not edit by hand. Source: specs/*.yaml. -->
# 📖 Captain.Food — Product Documentation (generated)

A single, navigable view of the whole product, built from the specs and organized **top-level by
bounded context** (🔲). Within each context: its API operations, output types, actors, views, commands,
events, entities, scalars, errors, tests and observability contracts. Every item — and every
**property** 🔹 — is anchored and **cross-linked**; \`cross-cutting\` holds the shared vocabulary and ops
that belong to no single context. Stories and Architecture span all contexts.

**Kinds**: ${emo('query')} query · ${emo('mutation')} mutation · ${emo('subscription')} subscription · ${emo('type')} type · ${emo('actor')} actor · ${emo('view')} view · ${emo('command')} command · ${emo('event')} event · ${emo('entity')} entity · ${emo('scalar')} scalar · ${emo('error')} error · ${emo('property')} property
**Roles**: 🌐 PUBLIC · 🙋 CUSTOMER · 🏪 RESTAURANT_ACCOUNT · 🍽️ RESTAURANT · 🛵 RIDER · 🛠️ ADMIN · 🔌 EXTERNAL
**Markers**: ✅ required · ⬜ optional · 🛶 V0 · 🔭 V1 · 🔒 internal · ⚠️ design hole

**Contents** — [🎬 Stories](#sec-stories) · ${ctxToc} · [🏛️ Architecture](#sec-architecture)

${sec('stories', '🎬', 'Stories')}

How each persona uses the API. \`personaRole\` is the persona's GraphQL path-role (UserType).

${storiesSection}

${ctxSections}

${sec('architecture', '🏛️', 'Architecture (C4)')}

C4 views as source-managed DSL (\`specs/architecture/c4-l{2,3}.yaml\`). Bounded contexts bind their
aggregates; components bind the aggregates they handle and the read models they update.

${c4Doc}
`;
}
