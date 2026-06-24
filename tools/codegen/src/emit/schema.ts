import type { Api, ApiField, Model, SchemaNode, SourceFile } from '../model.ts';
import { collectRefs, parseRef, refTargetFile } from '../refs.ts';

const I = '  ';
const pascal = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
const camel = (s: string) => s.charAt(0).toLowerCase() + s.slice(1);

/** Map an inline JSON-schema primitive to a GraphQL built-in. */
function inlinePrimitive(type: string, format?: string): string {
  if (type === 'integer') return 'Int';
  if (type === 'boolean') return 'Boolean';
  if (type === 'string') return format === 'date-time' ? 'DateTime' : 'String';
  return 'String';
}

/** GraphQL base type (no nullability) for a JSON-schema property node. */
function baseType(model: Model, node: SchemaNode, ctx: SourceFile, input: boolean): string {
  const n = node as Record<string, unknown>;
  if (typeof n.$ref === 'string') {
    const file = refTargetFile(n.$ref, ctx);
    const name = parseRef(n.$ref)?.path[0] ?? 'String';
    if (file === 'scalars.yaml') return name; // scalar or enum — same name
    return input ? `${name}Input` : name; // object type
  }
  if (n.type === 'array') return `[${baseType(model, n.items as SchemaNode, ctx, input)}!]`;
  return inlinePrimitive(String(n.type ?? 'string'), typeof n.format === 'string' ? n.format : undefined);
}

/** Emit `name: Type[!]` lines for a JSON-schema object definition. */
function objectFields(model: Model, def: SchemaNode, ctx: SourceFile, input: boolean): string[] {
  const props = (def.properties ?? {}) as Record<string, SchemaNode>;
  const required = new Set(Array.isArray(def.required) ? (def.required as string[]) : []);
  return Object.entries(props)
    // server-derived fields (readOnly) are outputs only — never part of an input type.
    .filter(([, p]) => !(input && (p as Record<string, unknown>).readOnly === true))
    .map(([name, p]) => {
      const base = baseType(model, p, ctx, input);
      // output: non-null unless `nullable: true`. input: non-null only when required.
      const nonNull = input ? required.has(name) : (p as Record<string, unknown>).nullable !== true;
      return `${I}${name}: ${base}${nonNull ? '!' : ''}`;
    });
}

function scalarNames(model: Model): Set<string> {
  return new Set(Object.keys(model.defs['scalars.yaml']));
}

/** GraphQL type for an api.yaml field (payload/arg/projection field). */
function apiFieldType(model: Model, f: ApiField, input: boolean): string {
  let base: string;
  if (f.ref) {
    base = f.type;
    if (input && !scalarNames(model).has(f.type)) base = `${f.type}Input`; // entity ref used as input
  } else {
    base = inlinePrimitive(f.type, f.format);
  }
  if (f.array) base = `[${base}!]`;
  const nonNull = input ? f.required === true : f.nullable !== true;
  return `${base}${nonNull ? '!' : ''}`;
}

// --- Scalars & enums (from scalars.yaml) ----------------------------------------------------------
function scalarsBlock(model: Model): string {
  const lines = ['scalar DateTime'];
  for (const [name, def] of Object.entries(model.defs['scalars.yaml'])) {
    if (!Array.isArray((def as Record<string, unknown>).enum)) lines.push(`scalar ${name}`);
  }
  return lines.join('\n');
}

function enumsBlock(model: Model): string {
  const blocks: string[] = [];
  for (const [name, def] of Object.entries(model.defs['scalars.yaml'])) {
    const values = (def as Record<string, unknown>).enum;
    if (Array.isArray(values)) blocks.push(`enum ${name} {\n${values.map((v) => `${I}${v}`).join('\n')}\n}`);
  }
  return blocks.join('\n\n');
}

const DIRECTIVES = `directive @auth(requires: [UserType!]!) on FIELD_DEFINITION
directive @public on FIELD_DEFINITION
directive @command(name: String!) on FIELD_DEFINITION
directive @reads(views: [String!]!) on FIELD_DEFINITION`;

// --- FK-derived navigation (views.yaml foreign keys → GraphQL relation fields) --------------------
function navByEntity(model: Model, entityNames: Set<string>): Map<string, string[]> {
  const viewAggregate = new Map(model.views.map((v) => [v.name, v.aggregate]));
  const seen = new Map<string, Set<string>>();
  const out = new Map<string, string[]>();
  const add = (entity: string, field: string, line: string) => {
    if (!entityNames.has(entity)) return;
    if (!seen.has(entity)) { seen.set(entity, new Set()); out.set(entity, []); }
    if (seen.get(entity)!.has(field)) return;
    seen.get(entity)!.add(field);
    out.get(entity)!.push(`${I}${line}`);
  };
  for (const v of model.views) {
    for (const col of v.columns) {
      if (!col.fk) continue;
      const targetView = col.fk.split('.')[0] ?? '';
      const tgt = viewAggregate.get(targetView);
      const src = v.aggregate;
      if (!tgt) continue;
      if (entityNames.has(tgt)) add(src, camel(tgt), `${camel(tgt)}: ${tgt}${col.nullable ? '' : '!'}`); // to-one
      if (entityNames.has(tgt)) add(tgt, `${camel(src)}s`, `${camel(src)}s: [${src}!]!`); // to-many reverse
    }
  }
  return out;
}

function outputTypesBlock(model: Model): string {
  // The api.yaml `types` registry is authoritative for GraphQL output types: each declares its shape
  // INLINE (the read/API shape, decoupled from entities.yaml). FK navigation applies to these names.
  const registered = new Set(model.api.types.map((t) => t.name));
  const nav = navByEntity(model, registered);
  const blocks: string[] = [];
  // entities.yaml value objects / sub-types NOT redefined in the registry are emitted as shared shapes
  // (referenced by the registered types via $ref: Money, Address, OptionList, OrderLineItem, ...).
  for (const [name, def] of Object.entries(model.defs['entities.yaml'])) {
    if (registered.has(name)) continue;
    const fields = objectFields(model, def, 'entities.yaml', false);
    const navFields = nav.get(name) ?? [];
    blocks.push(`type ${name} {\n${[...fields, ...navFields].join('\n')}\n}`);
  }
  // Registered output types: shape from inline `properties`, + FK-derived navigation.
  for (const t of model.api.types) {
    const fields = t.properties.map((f) => `${I}${f.name}: ${apiFieldType(model, f, false)}`);
    const navFields = nav.get(t.name) ?? [];
    blocks.push(`type ${t.name} {\n${[...fields, ...navFields].join('\n')}\n}`);
  }
  return blocks.join('\n\n');
}

// --- Input types (mutation command inputs + their referenced value objects) -----------------------
function inputTypesBlock(model: Model): string {
  // Collect every object type reachable from a mutation's command payload.
  const needed: { name: string; file: SourceFile }[] = [];
  const visited = new Set<string>();
  const visit = (name: string, file: SourceFile) => {
    const key = `${file}#${name}`;
    if (visited.has(key)) return;
    visited.add(key);
    const def = model.defs[file][name];
    if (!def) return;
    for (const occ of collectRefs(def, file)) {
      const tf = refTargetFile(occ.ref, file);
      const refName = parseRef(occ.ref)?.path[0];
      if (tf && tf !== 'scalars.yaml' && refName) {
        needed.push({ name: refName, file: tf });
        visit(refName, tf);
      }
    }
  };

  const commandInputs: string[] = [];
  for (const m of model.api.mutations) {
    const def = model.defs['commands.yaml'][m.command];
    if (!def) continue;
    commandInputs.push(`input ${m.command}Input {\n${objectFields(model, def, 'commands.yaml', true).join('\n')}\n}`);
    visit(m.command, 'commands.yaml');
  }

  // De-dupe the referenced value-object inputs (a value object reached from several commands).
  const emitted = new Set<string>();
  const objectInputs: string[] = [];
  for (const { name, file } of needed) {
    if (emitted.has(name)) continue;
    emitted.add(name);
    const def = model.defs[file][name];
    if (!def) continue;
    objectInputs.push(`input ${name}Input {\n${objectFields(model, def, file, true).join('\n')}\n}`);
  }
  return [...commandInputs, ...objectInputs].join('\n\n');
}

// --- Payloads (mutation results: always correlationId + minimal extras) ---------------------------
function payloadsBlock(model: Model): string {
  return model.api.mutations
    .map((m) => {
      const fields = [`${I}correlationId: CorrelationId!`, ...m.payload.map((f) => `${I}${f.name}: ${apiFieldType(model, f, false)}`)];
      return `type ${pascal(m.name)}Payload {\n${fields.join('\n')}\n}`;
    })
    .join('\n\n');
}

// --- Operations -----------------------------------------------------------------------------------
function authDirective(roles: string[]): string {
  return roles.includes('PUBLIC') ? '@public' : `@auth(requires: [${roles.join(', ')}])`;
}

function queryBlock(model: Model): string {
  const fields = model.api.queries.map((q) => {
    const args = q.args.map((a) => `${a.name}: ${apiFieldType(model, a, true)}`).join(', ');
    const argStr = args ? `(${args})` : '';
    const inner = q.returnsList ? `[${q.returnsType}!]` : q.returnsType;
    const ret = `${inner}${q.returnsNullable ? '' : '!'}`;
    const reads = `@reads(views: [${q.reads.map((v) => `"${v}"`).join(', ')}])`;
    return `${I}${q.name}${argStr}: ${ret} ${authDirective(q.roles)} ${reads}`;
  });
  return `type Query {\n${fields.join('\n')}\n}`;
}

function mutationBlock(model: Model): string {
  const fields = model.api.mutations.map((m) => {
    const payload = `${pascal(m.name)}Payload!`;
    return `${I}${m.name}(input: ${m.command}Input!): ${payload} ${authDirective(m.roles)} @command(name: "${m.command}")`;
  });
  return `type Mutation {\n${fields.join('\n')}\n}`;
}

const H = (title: string) =>
  `# ${'='.repeat(78)}\n# ${title}\n# ${'='.repeat(78)}`;

/** Generate the full GraphQL SDL from api.yaml + the domain model. */
export function emitSchema(model: Model): string {
  return `# GENERATED by tools/codegen from specs/api.yaml (+ scalars/entities/commands/views) — do not edit by hand.
# Strong typing: one scalars.yaml type = one GraphQL scalar/enum. Navigation fields on output types
# are derived from views.yaml foreign keys. Mutations return <Name>Payload (always carrying correlationId).

${H('Custom scalars')}
${scalarsBlock(model)}

${H('Enums')}
${enumsBlock(model)}

${H('Directives — ACL (@auth/@public) + declared links (@command/@reads)')}
${DIRECTIVES}

${H('Output types (entities.yaml + FK-derived navigation + projections)')}
${outputTypesBlock(model)}

${H('Input types (mutation command payloads)')}
${inputTypesBlock(model)}

${H('Mutation payloads')}
${payloadsBlock(model)}

${H('Queries — read side')}
${queryBlock(model)}

${H('Mutations — write side')}
${mutationBlock(model)}
`;
}
