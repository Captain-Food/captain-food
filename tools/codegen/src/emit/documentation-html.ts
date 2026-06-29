import type { ApiField, Model, SchemaNode } from '../model.ts';
import { refName } from '../refs.ts';

/**
 * Emit `documentation.generated.html` — the same product documentation as the Markdown version, but as
 * a self-contained, RICH page: ReSharper/Rider-style syntax colours per kind (type, property,
 * parameter, constant…), COLLAPSIBLE sections (`<details>`), and clickable, copyable deep links to
 * every item AND property (each header carries a 🔗 permalink that sets the URL hash).
 *
 * The returned string is BODY CONTENT (a `<style>` block + markup) — no doctype/head/body — so it can
 * be wrapped into a standalone file by the CLI and also published directly as a web artifact.
 */

const ROLE_EMOJI: Record<string, string> = {
  PUBLIC: '🌐', CUSTOMER: '🙋', RESTAURANT_ACCOUNT: '🏪', RESTAURANT: '🍽️',
  RIDER: '🛵', ADMIN: '🛠️', EXTERNAL: '🔌',
};
const KIND_EMOJI: Record<string, string> = {
  scalar: '🔤', entity: '📦', command: '📩', event: '⚡', view: '🗄️', actor: '🎭',
  type: '🧩', query: '🔎', mutation: '✏️', error: '⛔', property: '🔹', story: '🎬', test: '🧪',
  obs: '📡', context: '🔲', container: '🧱', component: '⚙️',
};
const emo = (k: string) => KIND_EMOJI[k] ?? '•';

const esc = (s: string) =>
  String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
const slug = (s: string) => s.toLowerCase().replace(/[^a-z0-9_]+/g, '-');
const pascal = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
const anchor = (kind: string, name: string) => `${kind}-${slug(name)}`;
const propAnchor = (kind: string, owner: string, field: string) => `${anchor(kind, owner)}--${slug(field)}`;

// CSS classes map a kind to a ReSharper/Rider-Darcula colour (see <style> below).
const KIND_CLASS: Record<string, string> = {
  type: 'k-type', entity: 'k-type', view: 'k-type', actor: 'k-type',
  scalar: 'k-scalar', query: 'k-op', mutation: 'k-op', command: 'k-op',
  event: 'k-event', error: 'k-error', property: 'k-prop', test: 'k-op',
  obs: 'k-event', context: 'k-type', container: 'k-type', component: 'k-op',
};
const cls = (k: string) => KIND_CLASS[k] ?? 'k-id';

const push = (m: Map<string, string[]>, k: string, v: string) => {
  if (!m.has(k)) m.set(k, []);
  if (!m.get(k)!.includes(v)) m.get(k)!.push(v);
};

const THEME = `<style>
  :root {
    --bg:#2b2b2b; --bg2:#313335; --bg3:#3c3f41; --fg:#a9b7c6; --muted:#808080; --line:#4b4b4b;
    --type:#4ec9b0; --scalar:#4fc1ff; --op:#dcdcaa; --event:#c586c0; --error:#f44747;
    --prop:#9cdcfe; --param:#d7ba7d; --const:#b5cea8; --kw:#cc7832; --accent:#ffc66d;
  }
  * { box-sizing:border-box; }
  body { margin:0; background:#2b2b2b; }
  .doc { background:var(--bg); color:var(--fg); font:14px/1.55 "JetBrains Mono","SFMono-Regular",Consolas,"Liberation Mono",monospace; padding:0 0 40vh; }
  .doc .wrap { max-width:1100px; margin:0 auto; padding:24px 20px; }
  .doc h1 { color:#fff; font-size:24px; border-bottom:2px solid var(--line); padding-bottom:10px; }
  .doc h3 { color:var(--accent); margin:18px 0 6px; }
  .doc a { color:var(--prop); text-decoration:none; }
  .doc a:hover { text-decoration:underline; }
  .doc code, .doc .id { font-family:inherit; }
  .k-type { color:var(--type); } .k-scalar { color:var(--scalar); } .k-op { color:var(--op); }
  .k-event { color:var(--event); } .k-error { color:var(--error); } .k-prop { color:var(--prop); }
  .k-param { color:var(--param); } .k-const { color:var(--const); } .k-id { color:var(--fg); }
  .kw { color:var(--kw); } .muted { color:var(--muted); } .req { color:var(--const); } .opt { color:var(--muted); }
  /* collapsible sections + items */
  details.sec { border:1px solid var(--line); border-radius:6px; margin:14px 0; background:var(--bg2); }
  details.sec > summary { cursor:pointer; padding:12px 16px; font-size:18px; color:#fff; list-style:none; position:sticky; top:0; background:var(--bg2); border-radius:6px; z-index:1; }
  details.sec[open] > summary { border-bottom:1px solid var(--line); border-radius:6px 6px 0 0; }
  details.sec > .body { padding:8px 16px 16px; }
  details.item { border-left:2px solid var(--line); margin:10px 0; padding-left:12px; }
  details.item > summary { cursor:pointer; list-style:none; padding:3px 0; }
  summary::-webkit-details-marker { display:none; }
  summary .tw { color:var(--muted); display:inline-block; width:1em; }
  .perma { color:var(--muted); opacity:0; margin-left:8px; font-size:.85em; }
  summary:hover .perma, h2:hover .perma { opacity:1; }
  .desc { color:var(--fg); margin:4px 0 8px; opacity:.92; }
  .rel { margin:2px 0; } .rel .lbl { color:var(--muted); }
  table { border-collapse:collapse; margin:6px 0 4px; width:100%; }
  th,td { border:1px solid var(--line); padding:4px 8px; text-align:left; vertical-align:top; }
  th { background:var(--bg3); color:#fff; font-weight:600; }
  .badge { background:var(--bg3); border:1px solid var(--line); border-radius:4px; padding:0 6px; font-size:.85em; }
  .toolbar { position:sticky; top:0; background:var(--bg); padding:10px 0; z-index:2; border-bottom:1px solid var(--line); }
  .toolbar button { background:var(--bg3); color:var(--fg); border:1px solid var(--line); border-radius:4px; padding:4px 10px; cursor:pointer; font:inherit; }
  .toolbar button:hover { border-color:var(--accent); color:#fff; }
  .toc a { margin-right:14px; white-space:nowrap; }
  .hole { color:var(--error); }
  /* interactive C4 / flow map */
  .cfmap { border:1px solid var(--line); border-radius:6px; background:#262626; padding:8px; }
  .cfmap-bar { display:flex; align-items:center; gap:10px; padding:4px 6px; flex-wrap:wrap; }
  .cfmap-bar button { background:var(--bg3); color:var(--fg); border:1px solid var(--line); border-radius:4px; padding:3px 10px; cursor:pointer; font:inherit; }
  .cfmap-bar button:hover { border-color:var(--accent); color:#fff; }
  #cf-svg { width:100%; height:auto; display:block; background:#262626; border-radius:4px; }
  .cf-node { cursor:pointer; }
  .cf-node:hover rect { filter:brightness(1.3); }
  .cf-node text { pointer-events:none; }
  .cfmap-info { padding:6px; font-size:.88em; }
</style>
<script>
  function setAll(open){ document.querySelectorAll('details').forEach(d=>d.open=open); }
</script>`;

export function emitDocumentationHtml(model: Model): string {
  const defs = model.defs;
  const scalarSet = new Set(Object.keys(defs['scalars.yaml']));
  const entitySet = new Set(Object.keys(defs['entities.yaml']));
  const typeSet = new Set(model.api.types.map((t) => t.name));
  const dDesc = (file: keyof typeof defs, name: string) =>
    String((defs[file][name] as Record<string, unknown> | undefined)?.description ?? '').trim().replace(/\s+/g, ' ');

  // --- relationship indexes (same as the markdown emitter) --------------------------------------
  const cmdHandler = new Map<string, { actor: string; emits: string[]; throws: string[] }>();
  const evtEmittedBy = new Map<string, string[]>();
  const evtConsumedBy = new Map<string, string[]>();
  const errThrownBy = new Map<string, string[]>();
  for (const a of model.actors) {
    for (const e of a.receives) {
      const msg = refName(e.message.$ref);
      const emits = e.emits.map((r) => refName(r.$ref)).filter((n): n is string => !!n);
      const throws = e.throws.map((r) => refName(r.$ref)).filter((n): n is string => !!n);
      if (e.message.$ref.startsWith('commands.yaml#/') && msg) {
        cmdHandler.set(msg, { actor: a.name, emits, throws });
        for (const er of throws) push(errThrownBy, er, msg);
      } else if (e.message.$ref.startsWith('events.yaml#/') && msg) push(evtConsumedBy, msg, a.name);
      for (const ev of emits) push(evtEmittedBy, ev, a.name);
    }
  }
  const evtViews = new Map<string, string[]>();
  for (const v of model.views) for (const r of v.fedBy) { const n = refName(r.$ref); if (n) push(evtViews, n, v.name); }
  const mutByCommand = new Map(model.api.mutations.map((m) => [m.command, m.name]));

  // --- link / token helpers ---------------------------------------------------------------------
  const link = (kind: string, name: string) =>
    `<a class="${cls(kind)}" href="#${anchor(kind, name)}">${emo(kind)}&nbsp;${esc(name)}</a>`;
  const plink = (kind: string, owner: string, field: string) =>
    `<a class="${cls(kind)}" href="#${propAnchor(kind, owner, field)}">${emo(kind)}&nbsp;${esc(owner)}.<span class="k-prop">${esc(field)}</span></a>`;
  const refLabel = (ref: string) => {
    const [file, name] = [ref.split('#/')[0], ref.split('#/')[1] ?? ''];
    return file === 'scalars.yaml' ? link('scalar', name) : link('entity', name);
  };
  const rawType = (p: SchemaNode): string => {
    const n = p as Record<string, unknown>;
    if (typeof n.$ref === 'string') return refLabel(n.$ref);
    if (n.type === 'array') return `[${rawType(n.items as SchemaNode)}]`;
    let t = `<span class="k-const">${esc(String(n.type ?? '?'))}</span>`;
    if (Array.isArray(n.enum)) t += ` <span class="muted">(${(n.enum as string[]).map(esc).join(' | ')})</span>`;
    if (typeof n.format === 'string') t += ` <span class="muted">${esc(n.format)}</span>`;
    return t;
  };
  const apiType = (f: ApiField): string => {
    let base: string;
    if (f.ref) base = scalarSet.has(f.type) ? link('scalar', f.type) : typeSet.has(f.type) ? link('type', f.type) : entitySet.has(f.type) ? link('entity', f.type) : `<span class="k-id">${esc(f.type)}</span>`;
    else base = `<span class="k-const">${esc(f.type)}</span>${f.format ? ` <span class="muted">${esc(f.format)}</span>` : ''}`;
    return f.array ? `[${base}]` : base;
  };
  const reqCell = (required: boolean, nullable?: boolean) => required ? '<span class="req">✅ required</span>' : `<span class="opt">⬜ ${nullable ? 'nullable' : 'optional'}</span>`;
  const table = (head: string[], rows: string[][]) =>
    rows.length ? `<table><thead><tr>${head.map((h) => `<th>${h}</th>`).join('')}</tr></thead><tbody>${rows.map((r) => `<tr>${r.map((c) => `<td>${c}</td>`).join('')}</tr>`).join('')}</tbody></table>` : '';

  // Anchored, collapsible item with a copyable permalink.
  const item = (kind: string, label: string, name: string, bodyHtml: string, descTxt?: string) => {
    const id = anchor(kind, name);
    const perma = `<a class="perma" href="#${id}" title="Lien vers cette section">🔗 #${id}</a>`;
    const desc = descTxt ? `<div class="desc">${esc(descTxt)}</div>` : '';
    return `<details class="item" id="${id}" open><summary><span class="tw">▸</span><span class="muted">${label}:</span> <span class="${cls(kind)}">${emo(kind)} ${esc(name)}</span>${perma}</summary>${desc}${bodyHtml}</details>`;
  };
  // property rows with their own anchor (clickable target)
  const propRows = (def: SchemaNode, kind: string, owner: string): string[][] => {
    const props = (def.properties ?? {}) as Record<string, SchemaNode>;
    const required = new Set(Array.isArray(def.required) ? (def.required as string[]) : []);
    return Object.entries(props).map(([n, p]) => {
      const pn = p as Record<string, unknown>;
      return [`<span id="${propAnchor(kind, owner, n)}" class="k-prop">${esc(n)}</span>`, rawType(p), reqCell(required.has(n), pn.nullable === true), esc(String(pn.description ?? '').replace(/\s+/g, ' '))];
    });
  };

  // ============================== sections ==============================
  const sec = (id: string, emoji: string, title: string, body: string) =>
    `<details class="sec" id="sec-${id}" open><summary>${emoji} ${esc(title)} <a class="perma" href="#sec-${id}">🔗</a></summary><div class="body">${body}</div></details>`;

  // 1. Stories
  const storiesHtml = model.personas.map((p) => {
    const badge = `<span class="badge">${ROLE_EMOJI[p.role] ?? '❔'} ${esc(p.role)}</span>${p.locale ? ` <span class="badge">🗣️ ${esc(p.locale)}</span>` : ''}`;
    const rows = p.activities.flatMap((act) => act.steps.map((s, i) => [
      i === 0 ? `<span class="kw">${esc(act.name)}</span>` : '',
      esc(s.name),
      s.op && s.opKind ? link(s.opKind, s.op) : s.note ? `📝 <span class="muted">${esc(s.note)}</span>` : '—',
    ]));
    return item('story', 'Persona', p.name, `${table(['Activity', 'Step', 'Operation'], rows)}`, p.description ? `${p.description}` : undefined)
      .replace('</summary>', ` ${badge}</summary>`);
  }).join('');

  // 2. API
  const queriesHtml = model.api.queries.map((q) => {
    const fieldList = q.args.map((a) => `<span class="k-param">${esc(a.name)}${a.required ? '' : '?'}</span>: ${apiType(a)}`).join(', ');
    // Queries with args take a single generated input class (`<Query>QueryInput`); args are never inlined.
    const inputType = q.args.length ? `${pascal(q.name)}QueryInput${q.args.some((a) => a.required) ? '!' : ''}` : '';
    const inputRel = q.args.length
      ? `<div class="rel"><span class="lbl">input:</span> <span class="k-type">🧩 ${esc(inputType)}</span> <span class="muted">{ ${fieldList} }</span></div>`
      : `<div class="rel"><span class="lbl">input:</span> <span class="muted">(none)</span></div>`;
    const retName = q.returnsType;
    const ret = (typeSet.has(retName) ? link('type', retName) : entitySet.has(retName) ? link('entity', retName) : `<span class="k-id">${esc(retName)}</span>`) + (q.returnsList ? ' []' : '');
    const reads = q.reads.map((v) => link('view', v)).join(', ') || '—';
    const body = inputRel
      + `<div class="rel"><span class="lbl">returns:</span> ${ret} · <span class="lbl">reads</span> ${reads}</div>`
      + `<div class="rel"><span class="lbl">roles:</span> ${esc(q.roles.join(', '))} · <span class="badge">${q.slice}</span></div>`;
    return item('query', 'Query', q.name, body, q.description);
  }).join('');
  const mutationsHtml = model.api.mutations.map((m) => {
    const h = cmdHandler.get(m.command);
    const payload = m.payload.map((f) => `<span class="k-prop">${esc(f.name)}</span>: ${apiType(f)}`).join(', ');
    const body = `<div class="rel"><span class="lbl">command:</span> ${link('command', m.command)}${h ? ` → ${link('actor', h.actor)}` : ''}</div>`
      + `<div class="rel"><span class="lbl">roles:</span> ${esc(m.roles.join(', '))} · <span class="badge">${m.slice}</span></div>`
      + `<div class="rel"><span class="lbl">payload:</span> <span class="muted">correlationId</span>${payload ? `, ${payload}` : ''}</div>`;
    return item('mutation', 'Mutation', m.name, body);
  }).join('');
  const typesHtml = model.api.types.map((t) => {
    const reads = t.reads.map((v) => link('view', v)).join(', ');
    const rows = t.properties.map((f) => [`<span id="${propAnchor('type', t.name, f.name)}" class="k-prop">${esc(f.name)}</span>`, apiType(f), reqCell(!f.nullable, f.nullable)]);
    const body = `<div class="rel"><span class="lbl">read model:</span> ${reads || '<span class="muted">(within a parent projection)</span>'}</div>${table(['Field', 'Type', 'Req.'], rows)}`;
    return item('type', 'Type', t.name, body, t.description);
  }).join('');
  const apiHtml = `<h3>${emo('query')} Queries</h3>${queriesHtml}<h3>${emo('mutation')} Mutations</h3>${mutationsHtml}<h3>${emo('type')} Output types</h3>${typesHtml}`;

  // 3. Actors
  const actorsHtml = model.actors.map((a) => {
    const kind = a.type === 'aggregate' ? '🧩 aggregate' : '⚙️ process manager';
    const rows = a.receives.map((e) => {
      const isCmd = e.message.$ref.startsWith('commands.yaml#/');
      const emits = e.emits.map((r) => link('event', refName(r.$ref) ?? '')).join(', ') || (e.effect ? `<span class="muted">${esc(e.effect)}</span>` : '—');
      const throws = e.throws.map((r) => link('error', refName(r.$ref) ?? '')).join(', ') || '—';
      return [link(isCmd ? 'command' : 'event', refName(e.message.$ref) ?? '?'), emits, throws];
    });
    return item('actor', 'Actor', a.name, `<div class="rel muted">${kind}</div>${table(['Receives', 'Emits →', 'Throws'], rows)}`, a.description);
  }).join('');

  // 4. Views
  const viewsHtml = model.views.map((v) => {
    const slice = v.slice === 'V1' ? '🔭 V1' : '🛶 V0';
    const fedBy = v.fedBy.map((r) => link('event', refName(r.$ref) ?? '')).join(', ') || '—';
    const rows = v.columns.map((c) => {
      const typeCell = (scalarSet.has(c.type) ? link('scalar', c.type) : `<span class="k-const">${esc(c.type || '?')}</span>`) + (c.typeDerived ? ' <span class="muted">(derived)</span>' : '') + (c.fk ? ` → ${link('view', c.fk.split('.')[0] ?? c.fk)}` : '');
      const src = (c.from ?? []).map((ref) => {
        const segs = (ref.split('#/')[1] ?? '').split('/').filter(Boolean);
        const prop = segs[1] === 'properties' ? segs[2] : undefined;
        return prop ? plink('event', segs[0] ?? '', prop) : link('event', segs[0] ?? '');
      }).join(', ') || '<span class="hole">⚠️ none</span>';
      const flags = [c.pk && 'PK', c.unique && 'unique', c.index && 'index', c.nullable && 'nullable'].filter(Boolean).join(', ') || '—';
      return [`<span id="${propAnchor('view', v.name, c.name)}" class="k-prop">${esc(c.name)}</span>`, typeCell, src, flags, esc((c.note ?? '').replace(/\s+/g, ' '))];
    });
    const body = `<div class="rel"><span class="lbl">aggregate:</span> ${link('actor', v.aggregate)} · ${slice}${v.internal ? ' · 🔒 internal' : ''}</div>`
      + (v.note ? `<div class="desc">${esc(v.note.replace(/\s+/g, ' '))}</div>` : '')
      + `<div class="rel"><span class="lbl">fed by:</span> ${fedBy}</div>`
      + table(['Column', 'Type', 'Sourced from', 'Constraints', 'Notes'], rows);
    return item('view', 'View', v.name, body);
  }).join('');

  // 5. Commands
  const commandsHtml = Object.keys(defs['commands.yaml']).filter((c) => cmdHandler.has(c)).map((c) => {
    const h = cmdHandler.get(c)!;
    const mut = mutByCommand.get(c);
    const body = `<div class="rel"><span class="lbl">dispatched by:</span> ${mut ? link('mutation', mut) : '—'} · <span class="lbl">handled by</span> ${link('actor', h.actor)}</div>`
      + `<div class="rel"><span class="lbl">emits:</span> ${h.emits.map((e) => link('event', e)).join(', ') || '—'}</div>`
      + `<div class="rel"><span class="lbl">throws:</span> ${h.throws.map((e) => link('error', e)).join(', ') || '—'}</div>`
      + table(['Field', 'Type', 'Req.', 'Description'], propRows(defs['commands.yaml'][c] ?? {}, 'command', c));
    return item('command', 'Command', c, body, dDesc('commands.yaml', c));
  }).join('');

  // 6. Events
  const nonProjected = new Set(model.nonProjectedEvents);
  const eventsHtml = Object.keys(defs['events.yaml']).map((ev) => {
    const projected = (evtViews.get(ev) ?? []).map((v) => link('view', v)).join(', ') || (nonProjected.has(ev) ? '<span class="muted">non-projected</span>' : '—');
    const body = `<div class="rel"><span class="lbl">emitted by:</span> ${(evtEmittedBy.get(ev) ?? []).map((a) => link('actor', a)).join(', ') || '<span class="muted">inbound / external</span>'}</div>`
      + `<div class="rel"><span class="lbl">consumed by:</span> ${(evtConsumedBy.get(ev) ?? []).map((a) => link('actor', a)).join(', ') || '—'}</div>`
      + `<div class="rel"><span class="lbl">projected into:</span> ${projected}</div>`
      + table(['Field', 'Type', 'Req.', 'Description'], propRows(defs['events.yaml'][ev] ?? {}, 'event', ev));
    return item('event', 'Event', ev, body, dDesc('events.yaml', ev));
  }).join('');

  // 7. Entities
  const entitiesHtml = Object.keys(defs['entities.yaml']).map((e) =>
    item('entity', 'Entity', e, table(['Field', 'Type', 'Req.', 'Description'], propRows(defs['entities.yaml'][e] ?? {}, 'entity', e)), dDesc('entities.yaml', e))).join('');

  // 8. Scalars
  const scalarRows = Object.entries(defs['scalars.yaml']).map(([name, d]) => {
    const n = d as Record<string, unknown>;
    let t = `<span class="k-const">${esc(String(n.type ?? '?'))}</span>`;
    if (Array.isArray(n.enum)) t = `<span class="kw">enum</span> <span class="muted">(${(n.enum as string[]).map(esc).join(' | ')})</span>`;
    else if (typeof n.format === 'string') t += ` <span class="muted">${esc(n.format)}</span>`;
    else if (typeof n.pattern === 'string') t += ` <span class="muted">${esc(String(n.pattern))}</span>`;
    return [`<span id="${anchor('scalar', name)}" class="k-scalar">${emo('scalar')} ${esc(name)}</span>`, t, esc(String(n.description ?? '').replace(/\s+/g, ' '))];
  });

  // 9. Errors
  const errorRows = Object.entries(defs['errors.yaml']).map(([name, d]) => {
    const n = d as Record<string, unknown>;
    const msgs = (n.messages as Record<string, unknown> | undefined) ?? {};
    const en = (msgs.en as string | undefined) ?? '';
    const fr = (msgs.fr as string | undefined) ?? '';
    const by = (errThrownBy.get(name) ?? []).map((c) => link('command', c)).join(', ') || '—';
    return [`<span id="${anchor('error', name)}" class="k-error">${emo('error')} ${esc(name)}</span>`, esc(String(n.description ?? '').replace(/\s+/g, ' ')), `🇬🇧 ${esc(en)}`, `🇫🇷 ${esc(fr)}`, by];
  });

  // 10. Tests (behaviour Given/When/Then, grouped by the aggregate under test)
  const testsHtml = (() => {
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

    return model.actors.map((a) => {
      const entries = Object.entries(tests).filter(([, t]) => refName((t.actor as { $ref?: string })?.$ref ?? '') === a.name);
      if (!entries.length) return '';
      const cases = entries.map(([name, t]) => {
        const cmd = refName(((t.when as { type?: { $ref?: string } })?.type)?.$ref ?? '') ?? '?';
        const given = Array.isArray(t.given) && t.given.length ? evLinks(t.given) : '<span class="muted">(none)</span>';
        const hasThrown = Object.prototype.hasOwnProperty.call(t, 'thrown');
        const thenArr = Array.isArray(t.then) ? t.then : [];
        const outcome = hasThrown
          ? `<div class="rel"><span class="lbl">thrown:</span> ${(t.thrown as Array<{ $ref?: string }>).map((r) => link('error', refName(r.$ref ?? '') ?? '')).join(', ') || '—'}</div>`
          : `<div class="rel"><span class="lbl">then:</span> ${thenArr.length ? evLinks(thenArr) : '<span class="k-const">∅ no event (idempotent no-op)</span>'}</div>`;
        const body = `<div class="rel"><span class="lbl">given:</span> ${given}</div>`
          + `<div class="rel"><span class="lbl">when:</span> ${link('command', cmd)}</div>`
          + outcome;
        return item('test', 'Test', name, body, typeof t.name === 'string' ? t.name : undefined);
      }).join('');
      return `<h3>${link('actor', a.name)}</h3>${cases}`;
    }).filter(Boolean).join('');
  })();

  // any $ref -> a colored link, kind chosen from the target file.
  const anyLink = (ref: unknown): string => {
    if (typeof ref !== 'string') return '—';
    const [file, name] = [ref.split('#/')[0], ref.split('#/')[1] ?? ''];
    const kind = file === 'commands.yaml' ? 'command' : file === 'events.yaml' ? 'event'
      : file === 'actors.yaml' ? 'actor' : file === 'views.yaml' ? 'view'
      : file === 'scalars.yaml' ? 'scalar' : 'entity';
    return link(kind, name);
  };
  const refLinks = (arr: unknown): string =>
    (Array.isArray(arr) ? arr : []).map((r) => anyLink((r as { $ref?: string })?.$ref)).join(', ') || '—';

  // 11. Observability contracts
  const obsHtml = Object.entries((defs['observability.yaml'] ?? {}) as Record<string, Record<string, unknown>>).map(([feature, c]) => {
    const wf = (c.workflow ?? {}) as Record<string, unknown>;
    const ids = (Array.isArray(c.run_identity) ? c.run_identity : []) as Array<Record<string, unknown>>;
    const idRows = ids.map((i) => [`<span class="k-prop">${esc(String(i.name))}</span>`, `<span class="muted">${esc(String(i.source ?? ''))}</span>`, i.required ? '<span class="req">✅</span>' : '<span class="opt">⬜</span>', i.businessKey ? anyLink((i.businessKey as { $ref?: string }).$ref) : '—']);
    const spans = (Array.isArray(c.spans) ? c.spans : []) as Array<Record<string, unknown>>;
    const spanRows = spans.map((s) => {
      const attrs = (Array.isArray(s.attributes) ? s.attributes : []) as Array<Record<string, unknown>>;
      const a = attrs.map((x) => `<span class="k-prop">${esc(String(x.key))}</span>${x.required ? '<span class="req">*</span>' : ''}`).join(', ') || '—';
      return [`<span class="k-op">${esc(String(s.name))}</span>`, `<span class="kw">${esc(String(s.kind ?? ''))}</span>`, s.required ? '<span class="req">✅</span>' : '<span class="opt">⬜</span>', s.multiplicity ? `<span class="muted">${esc(String(s.multiplicity))}</span>` : '—', a];
    });
    const metricList = (key: string) => ((Array.isArray(c[key]) ? c[key] : []) as Array<Record<string, unknown>>).map((m) => `<span class="k-const">${esc(String(m.name))}</span> <span class="muted">(${esc(String(m.type))})</span>`).join(', ') || '—';
    const sr = (c.status_rules ?? {}) as Record<string, Record<string, unknown>>;
    const lat = (c.latency_budget ?? {}) as Record<string, unknown>;
    const err = (c.error_budget ?? {}) as Record<string, unknown>;
    const reqSpans = (sr.success?.required_spans as string[] ?? []).map((s) => `<span class="k-op">${esc(s)}</span>`).join(', ');
    const body = `<div class="rel"><span class="lbl">workflow:</span> ${wf.saga ? `saga ${anyLink((wf.saga as { $ref?: string }).$ref)}` : ''}${wf.command ? ` · command ${anyLink((wf.command as { $ref?: string }).$ref)}` : ''}</div>`
      + `<div class="rel"><span class="lbl">emits:</span> ${refLinks(wf.emits)} · <span class="lbl">inbound:</span> ${refLinks(wf.inbound)}</div>`
      + (idRows.length ? `<div class="rel"><span class="lbl">run identity</span></div>${table(['Id', 'Source', 'Req.', 'Business key'], idRows)}` : '')
      + (spanRows.length ? `<div class="rel"><span class="lbl">spans</span> <span class="muted">(* = required attribute)</span></div>${table(['Span', 'Kind', 'Req.', 'Multiplicity', 'Attributes'], spanRows)}` : '')
      + `<div class="rel"><span class="lbl">metrics:</span> ${metricList('metrics')} · <span class="lbl">business:</span> ${metricList('business_metrics')}</div>`
      + (reqSpans ? `<div class="rel"><span class="lbl">success ⇐ spans:</span> ${reqSpans}</div>` : '')
      + `<div class="rel"><span class="lbl">SLOs:</span> p95 ≤ ${esc(String(lat.max_p95_ms ?? '—'))}ms · p99 ≤ ${esc(String(lat.max_p99_ms ?? '—'))}ms · error ≤ ${esc(String(err.max_error_rate_pct ?? '—'))}%</div>`;
    return item('obs', 'Contract', feature, body, `criticality: ${String(c.criticality ?? '—')}`);
  }).join('');

  // 12. Architecture (C4)
  const c4Html = (() => {
    const l2 = (defs['architecture/c4-l2.yaml'] ?? {}) as Record<string, Record<string, unknown>>;
    const l3 = (defs['architecture/c4-l3.yaml'] ?? {}) as Record<string, unknown>;
    const sys = (l2.system ?? {}) as Record<string, unknown>;
    const bcs = (l2.boundedContexts ?? {}) as Record<string, Record<string, unknown>>;
    const containers = (l2.containers ?? {}) as Record<string, Record<string, unknown>>;
    const externals = (l2.externalSystems ?? {}) as Record<string, Record<string, unknown>>;
    const rels = (Array.isArray(l2.relationships) ? l2.relationships : []) as Array<Record<string, unknown>>;
    const comps = (l3.components ?? {}) as Record<string, Record<string, unknown>>;
    const bcRows = Object.entries(bcs).map(([n, bc]) => [`${emo('context')} <span class="k-type">${esc(n)}</span>`, esc(String(bc.description ?? '')), `${refLinks(bc.aggregates)}${bc.processManagers ? ` · ${refLinks(bc.processManagers)}` : ''}`]);
    const cRows = Object.entries(containers).map(([n, c]) => [`${emo('container')} <span class="k-type">${esc(n)}</span>`, `<span class="muted">${esc(String(c.technology ?? ''))}</span>`, `${esc(String(c.description ?? ''))}${c.realizes ? `<br>realizes: ${refLinks(c.realizes)}` : ''}`]);
    const xRows = Object.entries(externals).map(([n, x]) => [`🔌 <span class="k-id">${esc(n)}</span>`, esc(String(x.description ?? ''))]);
    const relRows = rels.map((r) => [`<span class="k-id">${esc(String(r.from))}</span> → <span class="k-id">${esc(String(r.to))}</span>`, esc(String(r.description ?? ''))]);
    const compRows = Object.entries(comps).map(([n, c]) => [`${emo('component')} <span class="k-op">${esc(n)}</span>`, c.instrumented ? '📡 yes' : '<span class="muted">— no</span>', esc(String(c.description ?? '')), c.handles ? `handles ${refLinks(c.handles)}` : c.updates ? `updates ${refLinks(c.updates)}` : '—']);
    return `<div class="rel"><span class="lbl">system:</span> <span class="k-type">${esc(String(sys.name ?? 'Captain.Food'))}</span> — ${esc(String(sys.description ?? ''))}</div>`
      + `<h3>🔲 L2 — Bounded contexts</h3>${table(['Context', 'Description', 'Aggregates / process managers'], bcRows)}`
      + `<h3>🧱 L2 — Containers</h3>${table(['Container', 'Technology', 'Description'], cRows)}`
      + `<h3>🔌 L2 — External systems</h3>${table(['System', 'Description'], xRows)}`
      + `<h3>➡️ L2 — Relationships</h3>${table(['Edge', 'Description'], relRows)}`
      + `<h3>⚙️ L3 — Components of the api container</h3>${table(['Component', 'Instrumented', 'Description', 'Binds'], compRows)}`;
  })();

  // 13. Interactive system map — drill from System → containers → bounded contexts → aggregate flows.
  const mapData = (() => {
    const l2 = (defs['architecture/c4-l2.yaml'] ?? {}) as Record<string, Record<string, unknown>>;
    const nm = (arr: unknown) => (Array.isArray(arr) ? arr : []).map((r) => refName((r as { $ref?: string })?.$ref ?? '')).filter((n): n is string => !!n);
    const sys = (l2.system ?? {}) as Record<string, unknown>;
    const contexts = Object.entries((l2.boundedContexts ?? {}) as Record<string, Record<string, unknown>>).map(([id, bc]) => ({ id, description: String(bc.description ?? ''), aggregates: nm(bc.aggregates), processManagers: nm(bc.processManagers) }));
    const containers = Object.entries((l2.containers ?? {}) as Record<string, Record<string, unknown>>).map(([id, c]) => ({ id, technology: String(c.technology ?? ''), description: String(c.description ?? ''), realizes: nm(c.realizes) }));
    const externals = Object.entries((l2.externalSystems ?? {}) as Record<string, Record<string, unknown>>).map(([id, x]) => ({ id, description: String(x.description ?? '') }));
    const relationships = (Array.isArray(l2.relationships) ? l2.relationships : []).map((r) => ({ from: String((r as Record<string, unknown>).from), to: String((r as Record<string, unknown>).to), description: String((r as Record<string, unknown>).description ?? '') }));
    const actors: Record<string, unknown> = {};
    for (const a of model.actors) actors[a.name] = { type: a.type, receives: a.receives.map((e) => ({ message: refName(e.message.$ref), isCommand: e.message.$ref.startsWith('commands.yaml#/'), emits: e.emits.map((r) => refName(r.$ref)).filter(Boolean), throws: e.throws.map((r) => refName(r.$ref)).filter(Boolean) })) };
    const views = model.views.map((v) => ({ name: v.name, fedBy: v.fedBy.map((r) => refName(r.$ref)).filter(Boolean) }));
    return { system: { name: String(sys.name ?? 'Captain.Food'), description: String(sys.description ?? '') }, contexts, containers, externals, relationships, actors, views };
  })();

  // The renderer is plain JS (single quotes + string concat — NO backticks / template literals — so it
  // embeds safely in this TS template). `__CF_DATA__` is replaced with the model JSON.
  const MAP_JS = "(function(){var M=__CF_DATA__;var svg=document.getElementById('cf-svg'),crumb=document.getElementById('cf-crumb'),info=document.getElementById('cf-info'),back=document.getElementById('cf-back');if(!svg)return;var NS='http://www.w3.org/2000/svg';var stack=[{key:'system',title:'System'}];"
    + "function slug(s){return String(s).toLowerCase().replace(/[^a-z0-9_]+/g,'-');}"
    + "function el(t,a,x){var e=document.createElementNS(NS,t);for(var k in a)e.setAttribute(k,a[k]);if(x!=null)e.textContent=x;return e;}"
    + "var K={container:'#4ec9b0',external:'#cc7832',context:'#ffc66d',actor:'#4ec9b0','process':'#56a0c0',command:'#dcdcaa',event:'#c586c0',view:'#9cdcfe'};"
    + "function find(a,id){for(var i=0;i<a.length;i++)if(a[i].id===id)return a[i];return null;}"
    + "function frame(key){"
    + "if(key==='system'){var nodes=[];M.containers.forEach(function(c){nodes.push({id:c.id,label:c.id,kind:'container',sub:'container:'+c.id,desc:c.technology+' — '+c.description});});M.externals.forEach(function(x){nodes.push({id:x.id,label:x.id,kind:'external',desc:x.description});});var ids={};nodes.forEach(function(n){ids[n.id]=1;});var edges=M.relationships.filter(function(r){return ids[r.from]&&ids[r.to];}).map(function(r){return {from:r.from,to:r.to,label:r.description};});return {title:'System',nodes:nodes,edges:edges,note:'Containers (teal) and external systems (orange). Click a container to see its bounded contexts.'};}"
    + "if(key.indexOf('container:')===0){var id=key.slice(10);var c=find(M.containers,id)||{realizes:[]};var nodes=[];M.contexts.forEach(function(ctx){var inIt=(ctx.aggregates||[]).some(function(a){return (c.realizes||[]).indexOf(a)>=0;});if(inIt)nodes.push({id:ctx.id,label:ctx.id,kind:'context',sub:'context:'+ctx.id,desc:ctx.description});});return {title:id,nodes:nodes,edges:[],note:nodes.length?'Bounded contexts running in this container. Click one to see its aggregates.':'No bounded context runs in this container (infrastructure/runtime unit).'};}"
    + "if(key.indexOf('context:')===0){var id=key.slice(8);var ctx=find(M.contexts,id)||{aggregates:[],processManagers:[]};var nodes=(ctx.aggregates||[]).map(function(a){return {id:a,label:a,kind:'actor',sub:'actor:'+a,anchor:'actor-'+slug(a)};});(ctx.processManagers||[]).forEach(function(a){nodes.push({id:a,label:a,kind:'process',sub:'actor:'+a,anchor:'actor-'+slug(a)});});return {title:id,nodes:nodes,edges:[],note:'Aggregates and process managers (sagas). Click one to see its command → event → view flow.'};}"
    + "if(key.indexOf('actor:')===0){var name=key.slice(6);var a=M.actors[name]||{receives:[]};var nodes=[],edges=[],seen={};function add(id,label,kind,anchor){if(!seen[id]){seen[id]=1;nodes.push({id:id,label:label,kind:kind,anchor:anchor});}}add('A',name,a.type==='process-manager'?'process':'actor','actor-'+slug(name));a.receives.forEach(function(r){var mid=(r.isCommand?'c:':'e:')+r.message;add(mid,r.message,r.isCommand?'command':'event',(r.isCommand?'command-':'event-')+slug(r.message));edges.push({from:'A',to:mid,label:'receives'});(r.emits||[]).forEach(function(ev){add('e:'+ev,ev,'event','event-'+slug(ev));edges.push({from:mid,to:'e:'+ev,label:'emits'});M.views.forEach(function(v){if((v.fedBy||[]).indexOf(ev)>=0){add('v:'+v.name,v.name,'view','view-'+slug(v.name));edges.push({from:'e:'+ev,to:'v:'+v.name,label:'projects'});}});});});return {title:name,nodes:nodes,edges:edges,note:'Flow: message (yellow=command, purple=event) → emitted events → read models (blue). Click a box to jump to its section.'};}"
    + "return {title:'?',nodes:[],edges:[]};}"
    + "function render(){var f=frame(stack[stack.length-1].key);crumb.textContent=stack.map(function(s){return s.title;}).join('  ›  ');back.style.visibility=stack.length>1?'visible':'hidden';while(svg.firstChild)svg.removeChild(svg.firstChild);var defs=el('defs');var mk=el('marker',{id:'cf-arrow',viewBox:'0 0 10 10',refX:'9',refY:'5',markerWidth:'7',markerHeight:'7',orient:'auto'});mk.appendChild(el('path',{d:'M0,0 L10,5 L0,10 z',fill:'#888'}));defs.appendChild(mk);svg.appendChild(defs);var W=960,H=560,n=f.nodes.length||1;var cols=Math.max(1,Math.ceil(Math.sqrt(n)));var rows=Math.ceil(n/cols);var nw=180,nh=48;var gx=(W-cols*nw)/(cols+1),gy=(H-rows*nh)/(rows+1);var pos={};f.nodes.forEach(function(nd,i){var r=Math.floor(i/cols),c=i%cols;pos[nd.id]={x:gx+c*(nw+gx),y:gy+r*(nh+gy)};});f.edges.forEach(function(e){var a=pos[e.from],b=pos[e.to];if(!a||!b)return;var x1=a.x+nw/2,y1=a.y+nh/2,x2=b.x+nw/2,y2=b.y+nh/2;var ln=el('line',{x1:x1,y1:y1,x2:x2,y2:y2,stroke:'#6a6a6a','stroke-width':'1.3','marker-end':'url(#cf-arrow)'});if(e.label)ln.appendChild(el('title',null,e.label));svg.appendChild(ln);});f.nodes.forEach(function(nd){var p=pos[nd.id];var g=el('g',{'class':'cf-node',transform:'translate('+p.x+','+p.y+')'});g.appendChild(el('rect',{width:nw,height:nh,rx:'7',fill:'#313335',stroke:(K[nd.kind]||'#888'),'stroke-width':'1.6'}));var label=nd.label.length>24?nd.label.slice(0,23)+'…':nd.label;g.appendChild(el('text',{x:nw/2,y:nh/2+4,'text-anchor':'middle',fill:'#e6e6e6','font-size':'12'},label));if(nd.desc)g.appendChild(el('title',null,nd.desc));g.addEventListener('click',function(){if(nd.sub){stack.push({key:nd.sub,title:nd.label});render();}else if(nd.anchor){location.hash=nd.anchor;}});svg.appendChild(g);});info.textContent=f.note||'';}"
    + "back.addEventListener('click',function(){if(stack.length>1){stack.pop();render();}});render();})();";

  const mapHtml = '<div class="cfmap"><div class="cfmap-bar"><button id="cf-back">◀ back</button> <span id="cf-crumb" class="muted"></span></div>'
    + '<svg id="cf-svg" viewBox="0 0 960 560" preserveAspectRatio="xMidYMid meet" role="img" aria-label="Captain.Food system map"></svg>'
    + '<div id="cf-info" class="cfmap-info muted"></div></div>'
    + '<script>' + MAP_JS.replace('__CF_DATA__', JSON.stringify(mapData)) + '</script>';

  const legend = [
    `${emo('query')} <span class="k-op">query</span>`, `${emo('mutation')} <span class="k-op">mutation</span>`,
    `${emo('type')} <span class="k-type">type</span>`, `${emo('actor')} <span class="k-type">actor</span>`,
    `${emo('view')} <span class="k-type">view</span>`, `${emo('command')} <span class="k-op">command</span>`,
    `${emo('event')} <span class="k-event">event</span>`, `${emo('entity')} <span class="k-type">entity</span>`,
    `${emo('scalar')} <span class="k-scalar">scalar</span>`, `${emo('error')} <span class="k-error">error</span>`,
    `🔹 <span class="k-prop">property</span>`, `<span class="k-param">parameter</span>`, `${emo('test')} <span class="k-op">test</span>`, `${emo('obs')} <span class="k-event">observability</span>`,
  ].join(' · ');
  const toc = [['stories', '🎬 Stories'], ['api', '🧰 API'], ['actors', `${emo('actor')} Actors`], ['views', `${emo('view')} Views`], ['commands', `${emo('command')} Commands`], ['events', `${emo('event')} Events`], ['entities', `${emo('entity')} Entities`], ['scalars', `${emo('scalar')} Scalars`], ['errors', `${emo('error')} Errors`], ['tests', `${emo('test')} Tests`], ['observability', `${emo('obs')} Observability`], ['architecture', '🏛️ Architecture'], ['map', '🗺️ Map']]
    .map(([id, t]) => `<a href="#sec-${id}">${t}</a>`).join('');

  return `${THEME}
<div class="doc"><div class="wrap">
  <h1>📖 Captain.Food — Product Documentation</h1>
  <p class="muted">Generated from the specs. Every item and property is anchored — click 🔗 to copy a deep link. Sections are collapsible.</p>
  <p><strong>Kinds:</strong> ${legend}</p>
  <p><strong>Roles:</strong> ${Object.entries(ROLE_EMOJI).map(([r, e]) => `${e} ${r}`).join(' · ')}</p>
  <div class="toolbar"><button onclick="setAll(true)">⊞ Expand all</button> <button onclick="setAll(false)">⊟ Collapse all</button> &nbsp; <span class="toc">${toc}</span></div>
  ${sec('stories', '🎬', '1. Stories', storiesHtml)}
  ${sec('api', '🧰', '2. API', apiHtml)}
  ${sec('actors', emo('actor'), '3. Actors', actorsHtml)}
  ${sec('views', emo('view'), '4. Views (read models)', viewsHtml)}
  ${sec('commands', emo('command'), '5. Commands', commandsHtml)}
  ${sec('events', emo('event'), '6. Events', eventsHtml)}
  ${sec('entities', emo('entity'), '7. Entities', entitiesHtml)}
  ${sec('scalars', emo('scalar'), '8. Scalars', table(['Scalar', 'Type', 'Description'], scalarRows))}
  ${sec('errors', emo('error'), '9. Errors', table(['Error', 'Description', 'Message (en)', 'Message (fr)', 'Thrown by'], errorRows))}
  ${sec('tests', emo('test'), '10. Tests', testsHtml)}
  ${sec('observability', emo('obs'), '11. Observability', obsHtml)}
  ${sec('architecture', '🏛️', '12. Architecture (C4)', c4Html)}
  ${sec('map', '🗺️', '13. System map (interactive)', '<p class="muted">Drill in: <strong>System → container → bounded context → aggregate flow</strong>. Boxes are colored by kind (containers/aggregates teal, externals orange, contexts gold, commands yellow, events purple, views blue). Click to go deeper; leaf boxes jump to their section; use ◀ back to climb out.</p>' + mapHtml)}
</div></div>`;
}
