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
};
const emo = (k: string) => KIND_EMOJI[k] ?? '•';

const esc = (s: string) =>
  String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
const slug = (s: string) => s.toLowerCase().replace(/[^a-z0-9_]+/g, '-');
const anchor = (kind: string, name: string) => `${kind}-${slug(name)}`;
const propAnchor = (kind: string, owner: string, field: string) => `${anchor(kind, owner)}--${slug(field)}`;

// CSS classes map a kind to a ReSharper/Rider-Darcula colour (see <style> below).
const KIND_CLASS: Record<string, string> = {
  type: 'k-type', entity: 'k-type', view: 'k-type', actor: 'k-type',
  scalar: 'k-scalar', query: 'k-op', mutation: 'k-op', command: 'k-op',
  event: 'k-event', error: 'k-error', property: 'k-prop', test: 'k-op',
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
    const args = q.args.map((a) => `<span class="k-param">${esc(a.name)}${a.required ? '' : '?'}</span>: ${apiType(a)}`).join(', ') || '—';
    const retName = q.returnsType;
    const ret = (typeSet.has(retName) ? link('type', retName) : entitySet.has(retName) ? link('entity', retName) : `<span class="k-id">${esc(retName)}</span>`) + (q.returnsList ? ' []' : '');
    const reads = q.reads.map((v) => link('view', v)).join(', ') || '—';
    const body = `<div class="rel"><span class="lbl">args:</span> ${args}</div>`
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

  const legend = [
    `${emo('query')} <span class="k-op">query</span>`, `${emo('mutation')} <span class="k-op">mutation</span>`,
    `${emo('type')} <span class="k-type">type</span>`, `${emo('actor')} <span class="k-type">actor</span>`,
    `${emo('view')} <span class="k-type">view</span>`, `${emo('command')} <span class="k-op">command</span>`,
    `${emo('event')} <span class="k-event">event</span>`, `${emo('entity')} <span class="k-type">entity</span>`,
    `${emo('scalar')} <span class="k-scalar">scalar</span>`, `${emo('error')} <span class="k-error">error</span>`,
    `🔹 <span class="k-prop">property</span>`, `<span class="k-param">parameter</span>`, `${emo('test')} <span class="k-op">test</span>`,
  ].join(' · ');
  const toc = [['stories', '🎬 Stories'], ['api', '🧰 API'], ['actors', `${emo('actor')} Actors`], ['views', `${emo('view')} Views`], ['commands', `${emo('command')} Commands`], ['events', `${emo('event')} Events`], ['entities', `${emo('entity')} Entities`], ['scalars', `${emo('scalar')} Scalars`], ['errors', `${emo('error')} Errors`], ['tests', `${emo('test')} Tests`]]
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
</div></div>`;
}
