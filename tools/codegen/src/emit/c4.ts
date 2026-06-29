import type { Model, SchemaNode } from '../model.ts';
import { refName } from '../refs.ts';

/**
 * Emit C4 architecture views from `specs/architecture/c4-l2.yaml` + `c4-l3.yaml` (and the actor/view
 * model): a **Structurizr DSL** workspace (`c4.generated.dsl`) and **Mermaid** diagrams
 * (`c4.generated.md`). Both are GENERATED — do not hand-edit; change the C4 YAML and regenerate.
 *
 * Mapping: the runtime `containers` become Structurizr containers; `externalSystems` become external
 * software systems; `relationships` become edges. The `api` container is populated with components —
 * the domain aggregates/process-managers (grouped by bounded context) plus the technical components
 * from L3 (grouped as Infrastructure) — wired by the canonical CQRS/ES pipeline.
 */

const id = (prefix: string, s: string) => `${prefix}${String(s).replace(/[^a-zA-Z0-9]+/g, '_')}`;
const q = (s: unknown) => `"${String(s ?? '').replace(/"/g, '\\"').replace(/\s+/g, ' ').trim()}"`;
const names = (arr: unknown): string[] =>
  (Array.isArray(arr) ? arr : []).map((r) => refName((r as { $ref?: string })?.$ref ?? '')).filter((n): n is string => !!n);

interface C4 {
  system: Record<string, unknown>;
  contexts: Array<{ id: string; description: string; aggregates: string[]; processManagers: string[] }>;
  containers: Array<{ id: string; technology: string; description: string; realizes: string[] }>;
  externals: Array<{ id: string; description: string }>;
  relationships: Array<{ from: string; to: string; description: string }>;
  components: Array<{ id: string; description: string; instrumented: boolean }>;
}

function readC4(model: Model): C4 {
  const l2 = (model.defs['architecture/c4-l2.yaml'] ?? {}) as Record<string, Record<string, unknown>>;
  const l3 = (model.defs['architecture/c4-l3.yaml'] ?? {}) as Record<string, unknown>;
  const ent = (o: unknown) => Object.entries((o ?? {}) as Record<string, Record<string, unknown>>);
  return {
    system: (l2.system ?? {}) as Record<string, unknown>,
    contexts: ent(l2.boundedContexts).map(([cid, bc]) => ({ id: cid, description: String(bc.description ?? ''), aggregates: names(bc.aggregates), processManagers: names(bc.processManagers) })),
    containers: ent(l2.containers).map(([cid, c]) => ({ id: cid, technology: String(c.technology ?? ''), description: String(c.description ?? ''), realizes: names(c.realizes) })),
    externals: ent(l2.externalSystems).map(([xid, x]) => ({ id: xid, description: String(x.description ?? '') })),
    relationships: (Array.isArray(l2.relationships) ? l2.relationships : []).map((r) => ({ from: String((r as Record<string, unknown>).from), to: String((r as Record<string, unknown>).to), description: String((r as Record<string, unknown>).description ?? '') })),
    components: ent((l3 as Record<string, unknown>).components).map(([compid, c]) => ({ id: compid, description: String(c.description ?? ''), instrumented: c.instrumented === true })),
  };
}

// The canonical CQRS/ES pipeline among the L3 api components (matches their descriptions in c4-l3.yaml).
const PIPELINE: Array<[string, string, string]> = [
  ['graphql-gateway', 'command-bus', 'dispatches command'],
  ['command-bus', 'command-handlers', 'invokes handler'],
  ['command-handlers', 'event-store-adapter', 'appends events'],
  ['event-store-adapter', 'event-publisher', 'publishes appended'],
  ['event-publisher', 'message-consumers', 'delivers events'],
  ['message-consumers', 'projection-updaters', 'feeds projections'],
  ['process-managers', 'command-bus', 'issues commands'],
];

export function emitStructurizr(model: Model): string {
  const c4 = readC4(model);
  const compIds = new Set(c4.components.map((c) => c.id));
  const idForContainer = (cid: string) => id('ct_', cid);
  const idForExternal = (xid: string) => id('x_', xid);
  const nodeId = (key: string) => (compIds.has(key) ? id('c_', key) : c4.containers.some((c) => c.id === key) ? idForContainer(key) : c4.externals.some((x) => x.id === key) ? idForExternal(key) : id('n_', key));

  const L: string[] = [];
  L.push(`workspace ${q(c4.system.name ?? 'Captain.Food')} ${q(c4.system.description)} {`);
  L.push('  model {');
  L.push(`    ss = softwareSystem ${q(c4.system.name ?? 'Captain.Food')} ${q(c4.system.description)} {`);
  for (const c of c4.containers) {
    const open = `      ${idForContainer(c.id)} = container ${q(c.id)} ${q(c.description)} ${q(c.technology)}`;
    if (c.id !== 'api') { L.push(open); continue; }
    L.push(`${open} {`);
    // Domain components: one per aggregate / process-manager, grouped by bounded context.
    for (const ctx of c4.contexts) {
      const members = [...ctx.aggregates.map((a) => ({ n: a, tag: 'Aggregate' })), ...ctx.processManagers.map((p) => ({ n: p, tag: 'ProcessManager' }))];
      if (!members.length) continue;
      L.push(`        group ${q(ctx.id)} {`);
      for (const m of members) L.push(`          ${id('a_', m.n)} = component ${q(m.n)} ${q(ctx.description)} ${q(m.tag)}`);
      L.push('        }');
    }
    // Technical components from L3.
    L.push(`        group "Infrastructure" {`);
    for (const comp of c4.components) L.push(`          ${id('c_', comp.id)} = component ${q(comp.id)} ${q(comp.description)} ${q(comp.instrumented ? 'Instrumented' : 'Domain')}`);
    L.push('        }');
    L.push('      }');
  }
  L.push('    }');
  // External systems.
  for (const x of c4.externals) L.push(`    ${idForExternal(x.id)} = softwareSystem ${q(x.id)} ${q(x.description)} "External"`);
  L.push('');
  // L2 relationships.
  for (const r of c4.relationships) L.push(`    ${nodeId(r.from)} -> ${nodeId(r.to)} ${q(r.description)}`);
  // Canonical api component pipeline (only edges whose endpoints exist).
  for (const [from, to, desc] of PIPELINE) if (compIds.has(from) && compIds.has(to)) L.push(`    ${id('c_', from)} -> ${id('c_', to)} ${q(desc)}`);
  if (compIds.has('projection-updaters')) L.push(`    ${id('c_', 'projection-updaters')} -> ${idForContainer('read-models')} "writes read models"`);
  if (compIds.has('event-store-adapter')) L.push(`    ${id('c_', 'event-store-adapter')} -> ${idForContainer('event-store')} "appends to domain_events"`);
  L.push('  }');
  // Views. Structurizr DSL requires each statement on its own line (no inline `{ … }`).
  L.push('  views {');
  const view = (decl: string) => { L.push(`    ${decl} {`); L.push('      include *'); L.push('      autolayout lr'); L.push('    }'); };
  view('systemContext ss "SystemContext"');
  view('container ss "Containers"');
  view(`component ${idForContainer('api')} "ApiComponents"`);
  L.push('    styles {');
  const style = (tag: string, props: string[]) => { L.push(`      element "${tag}" {`); for (const p of props) L.push(`        ${p}`); L.push('      }'); };
  style('Element', ['color #ffffff']);
  style('Software System', ['background #2d4f4a']);
  style('Container', ['background #313335']);
  style('External', ['background #cc7832']);
  style('Aggregate', ['background #4ec9b0', 'color #11201d']);
  style('ProcessManager', ['background #56a0c0']);
  style('Instrumented', ['background #c586c0']);
  style('Domain', ['background #313335']);
  L.push('    }');
  L.push('  }');
  L.push('}');
  L.push('');
  return L.join('\n');
}

export function emitMermaid(model: Model): string {
  const c4 = readC4(model);
  const internal = new Set(c4.containers.map((c) => c.id));
  const cid = (s: string) => id('n_', s);

  // 1) Container diagram (L2) with real relationships.
  const container: string[] = ['flowchart LR'];
  container.push('  subgraph CaptainFood["Captain.Food"]');
  for (const c of c4.containers) container.push(`    ${cid(c.id)}["${c.id}<br/><small>${c.technology}</small>"]`);
  container.push('  end');
  for (const x of c4.externals) container.push(`  ${cid(x.id)}[/"${x.id}"/]`);
  for (const r of c4.relationships) container.push(`  ${cid(r.from)} -->|"${r.description.replace(/"/g, "'")}"| ${cid(r.to)}`);

  // 2) Domain diagram: bounded contexts -> aggregates -> the read models they feed.
  const evtViews = new Map<string, string[]>();
  for (const v of model.views) for (const r of v.fedBy) { const e = refName(r.$ref); if (e) (evtViews.get(e) ?? evtViews.set(e, []).get(e)!).push(v.name); }
  const emitsOf = new Map<string, Set<string>>();
  for (const a of model.actors) {
    const s = new Set<string>();
    for (const e of a.receives) for (const ev of e.emits) { const n = refName(ev.$ref); if (n) s.add(n); }
    emitsOf.set(a.name, s);
  }
  const domain: string[] = ['flowchart LR'];
  for (const ctx of c4.contexts) {
    domain.push(`  subgraph ${id('g_', ctx.id)}["${ctx.id}"]`);
    for (const a of [...ctx.aggregates, ...ctx.processManagers]) domain.push(`    ${id('a_', a)}["${a}"]`);
    domain.push('  end');
  }
  const viewIds = new Set<string>();
  const edges = new Set<string>();
  for (const a of model.actors) {
    const views = new Set<string>();
    for (const ev of emitsOf.get(a.name) ?? []) for (const v of evtViews.get(ev) ?? []) views.add(v);
    for (const v of views) { viewIds.add(v); edges.add(`  ${id('a_', a.name)} --> ${id('v_', v)}`); }
  }
  for (const v of viewIds) domain.push(`  ${id('v_', v)}[("${v}")]`);
  domain.push(...edges);

  // 3) Saga sequence diagrams: for each process manager, the time-ordered message → emitted-events flow.
  const sagas = model.actors.filter((a) => a.type === 'process-manager').map((a) => {
    const L = ['sequenceDiagram', '  autonumber', '  participant C as Caller / inbound', `  participant P as ${a.name}`, '  participant S as Event store'];
    for (const e of a.receives) {
      const msg = refName(e.message.$ref) ?? '?';
      const kind = e.message.$ref.startsWith('commands.yaml#/') ? 'command' : 'event';
      L.push(`  C->>P: ${msg} (${kind})`);
      const emits = e.emits.map((r) => refName(r.$ref)).filter((n): n is string => !!n);
      if (emits.length) for (const ev of emits) L.push(`  P->>S: ${ev}`);
      else L.push(`  Note over P: ${(e.effect ?? 'no event emitted').replace(/[\n:;]/g, ' ').slice(0, 60)}`);
    }
    return { name: a.name, code: L.join('\n') };
  });
  const sagaBlocks = sagas.flatMap((s) => [`### ${s.name}`, '', '```mermaid', s.code, '```', '']);

  return [
    '<!-- GENERATED by tools/codegen — do not edit by hand. Source: specs/architecture/c4-*.yaml. -->',
    '# Captain.Food — C4 diagrams (Mermaid, generated)',
    '',
    'Rendered by any Mermaid-aware viewer (GitHub, VS Code, mermaid.live). The authoritative source is',
    '`specs/architecture/c4-l2.yaml` / `c4-l3.yaml`; regenerate with `npm run generate`.',
    '',
    '## L2 — Containers & external systems',
    '',
    '```mermaid',
    container.join('\n'),
    '```',
    '',
    '## Domain — bounded contexts → aggregates → read models',
    '',
    'Each aggregate links to the `View_*` read models its emitted events project into.',
    '',
    '```mermaid',
    domain.join('\n'),
    '```',
    '',
    '## Saga sequences — message → emitted events, in order',
    '',
    'Each process manager (saga) as a time-ordered sequence: the command/event it receives and the',
    'events it emits in response (derived from `actors.yaml`).',
    '',
    ...sagaBlocks,
  ].join('\n');
}
