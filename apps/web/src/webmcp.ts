import { resultError } from './commands';
import { getPlanogramContext, getSection, searchProducts, toToolChangeSet, toToolOperation, toToolPlacement, toToolProduct } from './queries';
import type { PlanogramSession, SessionPreviewResult } from './session';
import type { CommandResult, PlacementChange, SalesAllocationRequest, SalesAllocationScope, Selection } from './types';

type JsonSchema = {
  type: 'object' | 'string' | 'integer' | 'array' | 'boolean';
  description?: string;
  properties?: Record<string, JsonSchema>;
  required?: string[];
  additionalProperties?: boolean;
  items?: JsonSchema;
  oneOf?: JsonSchema[];
  enum?: string[];
  minItems?: number;
  maxItems?: number;
  minimum?: number;
  maximum?: number;
  maxLength?: number;
};

interface SiteToolExecutionContext {
  signal?: AbortSignal;
}

interface SiteToolDefinition {
  name: string;
  title: string;
  description: string;
  inputSchema: JsonSchema;
  annotations?: { readOnlyHint?: boolean };
  execute: (args: unknown, context?: SiteToolExecutionContext) => unknown | Promise<unknown>;
}

interface ModelContext {
  registerTool: (tool: SiteToolDefinition) => unknown | Promise<unknown>;
  unregisterTool?: (name: string) => unknown | Promise<unknown>;
}

declare global {
  interface Document {
    modelContext?: ModelContext;
  }
}

export interface WebMcpRegistration {
  status: 'ready' | 'unsupported' | 'error';
  registeredNames: string[];
  error?: string;
  unregister?: () => Promise<void>;
}

interface ToolError {
  status: 'error';
  code: 'invalid_input' | 'not_found' | 'unsupported' | 'cancelled';
  message: string;
  revision?: number;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isToolError(value: unknown): value is ToolError {
  return isRecord(value) && value.status === 'error' && typeof value.message === 'string';
}

function validateArguments(args: unknown, allowedKeys: readonly string[], revision: number): ToolError | undefined {
  // Some model-context implementations pass undefined for a no-argument tool.
  if (args === undefined) return undefined;
  if (!isRecord(args)) {
    return { status: 'error', code: 'invalid_input', message: 'Tool arguments must be a JSON object.', revision };
  }
  const unknownKey = Object.keys(args).find(key => !allowedKeys.includes(key));
  if (unknownKey) {
    return { status: 'error', code: 'invalid_input', message: `Unknown argument: ${unknownKey}.`, revision };
  }
  return undefined;
}

function readRequiredString(args: unknown, key: string, maxLength: number, revision: number): string | ToolError {
  const value = isRecord(args) ? args[key] : undefined;
  if (typeof value !== 'string' || !value.trim()) {
    return { status: 'error', code: 'invalid_input', message: `${key} must be a non-empty string.`, revision };
  }
  const trimmed = value.trim();
  if (trimmed.length > maxLength) {
    return { status: 'error', code: 'invalid_input', message: `${key} must be at most ${maxLength} characters.`, revision };
  }
  return trimmed;
}

function readOptionalString(args: unknown, key: string, maxLength: number, revision: number): string | undefined | ToolError {
  const value = isRecord(args) ? args[key] : undefined;
  if (value === undefined) return undefined;
  if (typeof value !== 'string') {
    return { status: 'error', code: 'invalid_input', message: `${key} must be a string.`, revision };
  }
  const trimmed = value.trim();
  if (trimmed.length > maxLength) {
    return { status: 'error', code: 'invalid_input', message: `${key} must be at most ${maxLength} characters.`, revision };
  }
  return trimmed || undefined;
}

function readRequiredRevision(args: unknown, revision: number): number | ToolError {
  const value = isRecord(args) ? args.expected_revision : undefined;
  if (typeof value !== 'number' || !Number.isInteger(value) || value < 0 || value > 4_294_967_295) {
    return { status: 'error', code: 'invalid_input', message: 'expected_revision must be an integer from 0 to 4294967295.', revision };
  }
  return value;
}

function readRequiredSequence(args: Record<string, unknown>, key: string, revision: number): number | ToolError {
  const value = args[key];
  if (typeof value !== 'number' || !Number.isInteger(value) || !Number.isSafeInteger(value) || value < 0 || value > 10_000) {
    return { status: 'error', code: 'invalid_input', message: `${key} must be an integer sequence from 0 to 10000.`, revision };
  }
  return value;
}

function readOptionalFacing(args: Record<string, unknown>, key: string, revision: number): number | undefined | ToolError {
  const value = args[key];
  if (value === undefined) return undefined;
  if (typeof value !== 'number' || !Number.isInteger(value) || value < 1 || value > 100) {
    return { status: 'error', code: 'invalid_input', message: `${key} must be an integer from 1 to 100.`, revision };
  }
  return value;
}

function readPlacementChanges(args: unknown, revision: number): PlacementChange[] | ToolError {
  const raw = isRecord(args) ? args.operations : undefined;
  if (!Array.isArray(raw) || raw.length < 1 || raw.length > 200) {
    return { status: 'error', code: 'invalid_input', message: 'operations must contain between 1 and 200 changes.', revision };
  }
  const changes: PlacementChange[] = [];
  for (const [index, value] of raw.entries()) {
    if (!isRecord(value) || typeof value.kind !== 'string') {
      return { status: 'error', code: 'invalid_input', message: `operations[${index}] must include a change kind.`, revision };
    }
    const allowed = value.kind === 'add'
      ? ['kind', 'product_id', 'shelf_id', 'sequence', 'facings_x', 'facings_y', 'facings_z']
      : value.kind === 'move'
        ? ['kind', 'placement_id', 'shelf_id', 'sequence']
        : value.kind === 'remove'
          ? ['kind', 'placement_id']
          : [];
    if (!allowed.length) {
      return { status: 'error', code: 'invalid_input', message: `operations[${index}].kind must be add, move, or remove.`, revision };
    }
    const unknownKey = Object.keys(value).find(key => !allowed.includes(key));
    if (unknownKey) {
      return { status: 'error', code: 'invalid_input', message: `Unknown argument: operations[${index}].${unknownKey}.`, revision };
    }
    if (value.kind === 'add') {
      const productId = readRequiredString(value, 'product_id', 120, revision);
      const shelfId = readRequiredString(value, 'shelf_id', 120, revision);
      const sequence = readRequiredSequence(value, 'sequence', revision);
      const facingsX = readOptionalFacing(value, 'facings_x', revision);
      const facingsY = readOptionalFacing(value, 'facings_y', revision);
      const facingsZ = readOptionalFacing(value, 'facings_z', revision);
      for (const result of [productId, shelfId, sequence, facingsX, facingsY, facingsZ]) {
        if (isToolError(result)) return result;
      }
      const change: Extract<PlacementChange, { kind: 'add' }> = {
        kind: 'add',
        product_id: productId as string,
        shelf_id: shelfId as string,
        sequence: sequence as number,
      };
      if (typeof facingsX === 'number') change.facings_x = facingsX;
      if (typeof facingsY === 'number') change.facings_y = facingsY;
      if (typeof facingsZ === 'number') change.facings_z = facingsZ;
      changes.push(change);
    } else if (value.kind === 'move') {
      const placementId = readRequiredString(value, 'placement_id', 120, revision);
      const shelfId = readRequiredString(value, 'shelf_id', 120, revision);
      const sequence = readRequiredSequence(value, 'sequence', revision);
      for (const result of [placementId, shelfId, sequence]) {
        if (isToolError(result)) return result;
      }
      changes.push({ kind: 'move', placement_id: placementId as string, shelf_id: shelfId as string, sequence: sequence as number });
    } else {
      const placementId = readRequiredString(value, 'placement_id', 120, revision);
      if (typeof placementId !== 'string') return placementId;
      changes.push({ kind: 'remove', placement_id: placementId });
    }
  }
  return changes;
}

function readSalesAllocationRequest(args: unknown, revision: number): SalesAllocationRequest | ToolError {
  const record = isRecord(args) ? args : {};
  const rawScope = record.scope;
  if (!isRecord(rawScope) || (rawScope.kind !== 'shelf' && rawScope.kind !== 'bay')) {
    return { status: 'error', code: 'invalid_input', message: 'scope must identify a shelf or bay.', revision };
  }
  const idKey = rawScope.kind === 'shelf' ? 'shelf_id' : 'section_id';
  const unexpected = Object.keys(rawScope).find(key => key !== 'kind' && key !== idKey);
  if (unexpected) {
    return { status: 'error', code: 'invalid_input', message: `Unknown argument: scope.${unexpected}.`, revision };
  }
  const id = readRequiredString(rawScope, idKey, 120, revision);
  if (typeof id !== 'string') return id;
  const scope: SalesAllocationScope = rawScope.kind === 'shelf'
    ? { kind: 'shelf', shelf_id: id }
    : { kind: 'bay', section_id: id };
  if (record.basis !== 'revenue' && record.basis !== 'units') {
    return { status: 'error', code: 'invalid_input', message: 'basis must be revenue or units.', revision };
  }
  if (record.target !== 'space' && record.target !== 'facings') {
    return { status: 'error', code: 'invalid_input', message: 'target must be space or facings.', revision };
  }
  const minimum = readOptionalFacing(record, 'min_facings', revision);
  const maximum = readOptionalFacing(record, 'max_facings', revision);
  if (isToolError(minimum)) return minimum;
  if (isToolError(maximum)) return maximum;
  if (minimum === undefined || maximum === undefined) {
    return { status: 'error', code: 'invalid_input', message: 'min_facings and max_facings are required integers from 1 to 100.', revision };
  }
  if (minimum > maximum) {
    return { status: 'error', code: 'invalid_input', message: 'min_facings must not exceed max_facings.', revision };
  }
  return { scope, basis: record.basis, target: record.target, min_facings: minimum, max_facings: maximum };
}

function cancelled(revision: number): ToolError {
  return { status: 'error', code: 'cancelled', message: 'The site-tool request was cancelled.', revision };
}

function requestWasCancelled(context?: SiteToolExecutionContext): boolean {
  return context?.signal?.aborted === true;
}

function mutationResult(result: CommandResult, session: PlanogramSession): Record<string, unknown> {
  if (result.status === 'applied') {
    const context = session.context();
    const placements = result.affected_ids
      .map(id => context.placements.find(candidate => candidate.id === id))
      .filter((candidate): candidate is NonNullable<typeof candidate> => candidate !== undefined);
    return {
      status: result.status,
      revision: result.revision,
      change_set: toToolChangeSet(result.change_set),
      affected_ids: result.affected_ids,
      placements: placements.map(toToolPlacement),
      placement: placements[0] ? toToolPlacement(placements[0]) : null,
      removed_placement_ids: result.scene_patch.removed_placement_ids,
      validation: result.validation,
    };
  }
  if (result.status === 'validation_failed') {
    return { status: result.status, revision: result.revision, validation: result.validation };
  }
  if (result.status === 'revision_conflict') {
    return {
      status: result.status,
      expected_revision: result.expected_revision,
      current_revision: result.current_revision,
    };
  }
  return {
    status: result.status,
    revision: session.context().revision,
    message: resultError(result) ?? 'The command could not be applied.',
  };
}

function previewResult(result: SessionPreviewResult, session: PlanogramSession): Record<string, unknown> {
  if (result.status === 'ready') {
    return {
      status: result.status,
      revision: result.revision,
      proposal_id: result.proposal_id,
      reason: result.reason,
      affected_ids: result.affected_ids,
      operations: result.operations.map(toToolOperation),
      validation: result.validation,
      ...(result.sales_allocation ? { sales_allocation: result.sales_allocation } : {}),
    };
  }
  if (result.status === 'validation_failed') {
    return { status: result.status, revision: result.revision, validation: result.validation };
  }
  if (result.status === 'revision_conflict') {
    return { status: result.status, expected_revision: result.expected_revision, current_revision: result.current_revision };
  }
  return {
    status: result.status,
    revision: session.context().revision,
    message: result.status === 'not_found' ? `${result.entity} ${result.id} was not found.` : result.message,
  };
}

function placementChangeSchema(): JsonSchema {
  return {
    type: 'object',
    oneOf: [
      {
        type: 'object',
        properties: {
          kind: { type: 'string', enum: ['add'] },
          product_id: { type: 'string', maxLength: 120 },
          shelf_id: { type: 'string', maxLength: 120 },
          sequence: { type: 'integer', minimum: 0, maximum: 10_000 },
          facings_x: { type: 'integer', minimum: 1, maximum: 100, description: 'Optional loose-product horizontal facings. Omit for a tray-configured product so Rust resolves its catalog preset.' },
          facings_y: { type: 'integer', minimum: 1, maximum: 100, description: 'Optional loose-product vertical facings. Omit for a tray-configured product so Rust resolves its catalog preset.' },
          facings_z: { type: 'integer', minimum: 1, maximum: 100, description: 'Optional loose-product depth facings. Omit for a tray-configured product so Rust resolves its catalog preset.' },
        },
        required: ['kind', 'product_id', 'shelf_id', 'sequence'],
        additionalProperties: false,
      },
      {
        type: 'object',
        properties: {
          kind: { type: 'string', enum: ['move'] },
          placement_id: { type: 'string', maxLength: 120 },
          shelf_id: { type: 'string', maxLength: 120 },
          sequence: { type: 'integer', minimum: 0, maximum: 10_000 },
        },
        required: ['kind', 'placement_id', 'shelf_id', 'sequence'],
        additionalProperties: false,
      },
      {
        type: 'object',
        properties: {
          kind: { type: 'string', enum: ['remove'] },
          placement_id: { type: 'string', maxLength: 120 },
        },
        required: ['kind', 'placement_id'],
        additionalProperties: false,
      },
    ],
  };
}

function schemas(): SiteToolDefinition[] {
  const noArguments: JsonSchema = { type: 'object', properties: {}, required: [], additionalProperties: false };
  return [
    {
      name: 'planogram.get_planogram_context',
      title: 'Get open planogram context',
      description: 'Reads the currently open draft planogram, its revision, selection, fixture dimensions, and summary counts.',
      inputSchema: noArguments,
      annotations: { readOnlyHint: true },
      execute: () => undefined,
    },
    {
      name: 'planogram.search_products',
      title: 'Search products',
      description: 'Searches the product catalog currently loaded in the open planogram by text, UPC, brand, category, or loose/tray stocking mode. Results include exact logistics, performance, dimensions, and optional shelf-ready tray data.',
      inputSchema: {
        type: 'object',
        properties: {
          query: { type: 'string', maxLength: 120 },
          upc: { type: 'string', maxLength: 32 },
          brand: { type: 'string', maxLength: 80 },
          category: { type: 'string', maxLength: 80 },
          stocking_mode: { type: 'string', enum: ['loose', 'tray'] },
          limit: { type: 'integer', minimum: 1, maximum: 50 },
        },
        required: [],
        additionalProperties: false,
      },
      annotations: { readOnlyHint: true },
      execute: () => undefined,
    },
    {
      name: 'planogram.get_product',
      title: 'Get product details',
      description: 'Reads one loaded catalog product, including exact dimensions, net weight, casepack, performance data and source period, plus optional shelf-ready tray geometry and preset facings.',
      inputSchema: {
        type: 'object',
        properties: { product_id: { type: 'string', maxLength: 120 } },
        required: ['product_id'],
        additionalProperties: false,
      },
      annotations: { readOnlyHint: true },
      execute: () => undefined,
    },
    {
      name: 'planogram.get_section',
      title: 'Get section layout',
      description: 'Reads one fixture section, its shelf bounds, largest horizontally placeable width after the required gap and x-grid, and current placements with Rust-derived display width, height, required depth, stocking mode, and stocked-unit count, plus simulated days of supply (stocked units × 7 ÷ assumed weekly units, summed across the fixture) and its band for each SKU in the section.',
      inputSchema: {
        type: 'object',
        properties: { section_id: { type: 'string', maxLength: 120 } },
        required: ['section_id'],
        additionalProperties: false,
      },
      annotations: { readOnlyHint: true },
      execute: () => undefined,
    },
    {
      name: 'planogram.validate_planogram',
      title: 'Validate open planogram',
      description: 'Runs the Rust domain engine’s whole-plan structural validation against the currently open draft. Returns the current revision and structured issues without changing geometry, history, or revision.',
      inputSchema: noArguments,
      annotations: { readOnlyHint: true },
      execute: () => undefined,
    },
    {
      name: 'planogram.add_product',
      title: 'Add product to shelf',
      description: 'Adds one catalog product to a named shelf and validates it physically. The product joins an existing block of the same SKU on that shelf or starts a new block at the right end; Rust then re-spaces the shelf so each same-SKU block stays at the 1/8-inch minimum gap and the blocks are spaced evenly, recording every resulting move in the same change set. Tray-configured products use their catalog preset facings, units-deep count, and loaded tray envelope resolved by Rust. This changes the open draft, never publishes it, and returns a change set for review or undo.',
      inputSchema: {
        type: 'object',
        properties: {
          product_id: { type: 'string', maxLength: 120 },
          shelf_id: { type: 'string', maxLength: 120 },
          expected_revision: { type: 'integer', minimum: 0 },
          reason: { type: 'string', maxLength: 240 },
        },
        required: ['product_id', 'shelf_id', 'expected_revision'],
        additionalProperties: false,
      },
      execute: () => undefined,
    },
    {
      name: 'planogram.distribute_shelf',
      title: 'Distribute products on shelf',
      description: 'Reflows every product on one shelf as packed left, a centered group, space between, or space evenly. Neighboring placements of the same SKU form one block that always keeps the 1/8-inch minimum gap; spacing goes between blocks. Rust preserves the current left-to-right order, resolves final coordinates on the 1/8-inch grid, enforces the 1/8-inch minimum product gap, and records one atomic change set.',
      inputSchema: {
        type: 'object',
        properties: {
          shelf_id: { type: 'string', maxLength: 120 },
          distribution: { type: 'string', enum: ['packed_left', 'centered', 'space_between', 'space_evenly'] },
          expected_revision: { type: 'integer', minimum: 0 },
          reason: { type: 'string', maxLength: 240 },
        },
        required: ['shelf_id', 'distribution', 'expected_revision'],
        additionalProperties: false,
      },
      execute: () => undefined,
    },
    {
      name: 'planogram.undo_change_set',
      title: 'Undo planogram change',
      description: 'Reverses the latest active change set in the open draft using the current expected revision and records a compensating change set. Read latest_undoable_change_set_id from planogram.get_planogram_context; consecutive undos walk backward through active history.',
      inputSchema: {
        type: 'object',
        properties: {
          change_set_id: { type: 'string', maxLength: 120 },
          expected_revision: { type: 'integer', minimum: 0 },
        },
        required: ['change_set_id', 'expected_revision'],
        additionalProperties: false,
      },
      execute: () => undefined,
    },
    {
      name: 'planogram.preview_shelf_allocation',
      title: 'Preview shelf-facing allocation',
      description: 'Builds a read-only Rust-resolved shelf allocation proposal. The fill_evenly strategy maximizes loose-product horizontal facings, balances facing counts as evenly as capacity permits, preserves stable current left-to-right order, keeps loaded trays at their fixed catalog presets, and distributes residual shelf slack evenly. Rust validates the complete result without changing the draft.',
      inputSchema: {
        type: 'object',
        properties: {
          shelf_id: { type: 'string', maxLength: 120 },
          strategy: { type: 'string', enum: ['fill_evenly'] },
          expected_revision: { type: 'integer', minimum: 0 },
          reason: { type: 'string', maxLength: 240 },
        },
        required: ['shelf_id', 'strategy', 'expected_revision'],
        additionalProperties: false,
      },
      annotations: { readOnlyHint: true },
      execute: () => undefined,
    },
    {
      name: 'planogram.preview_sales_allocation',
      title: 'Preview allocation by sales contribution',
      description: 'Previews a Rust-resolved allocation using synthetic revenue or unit contribution to target display-space or horizontal-facing shares on one shelf or bay. Retains represented SKUs, shelf assignments, stable order, loaded-tray presets, vertical and depth facings; min/max horizontal facings bound each loose placement. Demand is counted once per represented SKU, including fixed trays. Bay scope includes occupied adjustable shelves only. Rust validates physical fit and returns per-SKU before/after shares and warnings. This does not change the draft or forecast sales uplift; review before applying.',
      inputSchema: {
        type: 'object',
        properties: {
          scope: {
            type: 'object',
            oneOf: [
              { type: 'object', properties: { kind: { type: 'string', enum: ['shelf'] }, shelf_id: { type: 'string', maxLength: 120 } }, required: ['kind', 'shelf_id'], additionalProperties: false },
              { type: 'object', properties: { kind: { type: 'string', enum: ['bay'] }, section_id: { type: 'string', maxLength: 120 } }, required: ['kind', 'section_id'], additionalProperties: false },
            ],
          },
          basis: { type: 'string', enum: ['revenue', 'units'] },
          target: { type: 'string', enum: ['space', 'facings'] },
          min_facings: { type: 'integer', minimum: 1, maximum: 100 },
          max_facings: { type: 'integer', minimum: 1, maximum: 100 },
          expected_revision: { type: 'integer', minimum: 0, maximum: 4_294_967_295 },
          reason: { type: 'string', maxLength: 240 },
        },
        required: ['scope', 'basis', 'target', 'min_facings', 'max_facings', 'expected_revision'],
        additionalProperties: false,
      },
      annotations: { readOnlyHint: true },
      execute: () => undefined,
    },
    {
      name: 'planogram.preview_changes',
      title: 'Preview generic planogram changes',
      description: 'Builds a read-only proposal from generic add, move, and remove placement operations. The model expresses shelf assignment and zero-based sequence (include move operations for existing items that should participate in the requested order); omit facings for tray-configured products so Rust resolves the catalog preset and loaded tray envelope. Rust resolves physical x positions on the 1/8-inch grid and checks fit, the 1/8-inch minimum gap, overlap, shelf capacity, and all other planogram constraints without changing the draft.',
      inputSchema: {
        type: 'object',
        properties: {
          expected_revision: { type: 'integer', minimum: 0 },
          operations: { type: 'array', minItems: 1, maxItems: 200, items: placementChangeSchema() },
          reason: { type: 'string', maxLength: 240 },
        },
        required: ['expected_revision', 'operations'],
        additionalProperties: false,
      },
      annotations: { readOnlyHint: true },
      execute: () => undefined,
    },
    {
      name: 'planogram.apply_changes',
      title: 'Apply reviewed planogram proposal',
      description: 'Applies one reviewed proposal previously returned by planogram.preview_changes, planogram.preview_shelf_allocation, or planogram.preview_sales_allocation. The retained semantic intent is recomputed and revalidated in the same open document at the supplied revision, then commits as one atomic WebMCP change set or changes nothing. A proposal can be applied only once; repeat a preview after edits or document changes.',
      inputSchema: {
        type: 'object',
        properties: {
          proposal_id: { type: 'string', maxLength: 120 },
          expected_revision: { type: 'integer', minimum: 0 },
        },
        required: ['proposal_id', 'expected_revision'],
        additionalProperties: false,
      },
      execute: () => undefined,
    },
    {
      name: 'planogram.set_facings',
      title: 'Set product facings',
      description: 'Sets the horizontal (facings_x), vertical (facings_y), and depth (facings_z) facing counts of one loose placement; omitted counts stay unchanged. Rust then re-spaces the shelf: same-SKU blocks stay at the 1/8-inch minimum gap and blocks are spaced evenly, and every resulting move is recorded with the facing change. Rust validates shelf bounds, vertical clearance, shelf depth, and the gap, then records one atomic change set or changes nothing. Loaded trays keep their catalog preset and reject any other facings.',
      inputSchema: {
        type: 'object',
        properties: {
          placement_id: { type: 'string', maxLength: 120 },
          facings_x: { type: 'integer', minimum: 1, maximum: 100 },
          facings_y: { type: 'integer', minimum: 1, maximum: 100 },
          facings_z: { type: 'integer', minimum: 1, maximum: 100 },
          expected_revision: { type: 'integer', minimum: 0 },
          reason: { type: 'string', maxLength: 240 },
        },
        required: ['placement_id', 'expected_revision'],
        additionalProperties: false,
      },
      execute: () => undefined,
    },
  ];
}

function bindTools(session: PlanogramSession, getSelection: () => Selection | undefined): SiteToolDefinition[] {
  const tools = schemas();
  const [contextTool, searchTool, productTool, sectionTool, validationTool, addTool, distributeTool, undoTool, allocationPreviewTool, salesPreviewTool, previewTool, applyTool, facingsTool] = tools;
  contextTool.execute = (_args, executionContext) => {
    const context = session.context();
    if (requestWasCancelled(executionContext)) return cancelled(context.revision);
    const argumentError = validateArguments(_args, [], context.revision);
    if (argumentError) return argumentError;
    return getPlanogramContext(context, getSelection());
  };
  searchTool.execute = (args, executionContext) => {
    const context = session.context();
    if (requestWasCancelled(executionContext)) return cancelled(context.revision);
    const argumentError = validateArguments(args, ['query', 'upc', 'brand', 'category', 'stocking_mode', 'limit'], context.revision);
    if (argumentError) return argumentError;
    const query = readOptionalString(args, 'query', 120, context.revision);
    const upc = readOptionalString(args, 'upc', 32, context.revision);
    const brand = readOptionalString(args, 'brand', 80, context.revision);
    const category = readOptionalString(args, 'category', 80, context.revision);
    const stockingMode = readOptionalString(args, 'stocking_mode', 20, context.revision);
    for (const value of [query, upc, brand, category, stockingMode]) {
      if (isToolError(value)) return value;
    }
    if (stockingMode !== undefined && stockingMode !== 'loose' && stockingMode !== 'tray') {
      return { status: 'error', code: 'invalid_input', message: 'stocking_mode must be loose or tray.', revision: context.revision } satisfies ToolError;
    }
    const limit = isRecord(args) ? args.limit : undefined;
    if (limit !== undefined && (typeof limit !== 'number' || !Number.isInteger(limit) || limit < 1 || limit > 50)) {
      return { status: 'error', code: 'invalid_input', message: 'limit must be an integer from 1 to 50.', revision: context.revision } satisfies ToolError;
    }
    return { status: 'ok', revision: context.revision, products: searchProducts(context.products, { query: query as string | undefined, upc: upc as string | undefined, brand: brand as string | undefined, category: category as string | undefined, stocking_mode: stockingMode as 'loose' | 'tray' | undefined, limit: limit as number | undefined }).map(toToolProduct) };
  };
  productTool.execute = (args, executionContext) => {
    const context = session.context();
    if (requestWasCancelled(executionContext)) return cancelled(context.revision);
    const argumentError = validateArguments(args, ['product_id'], context.revision);
    if (argumentError) return argumentError;
    const productId = readRequiredString(args, 'product_id', 120, context.revision);
    if (typeof productId !== 'string') return productId;
    const product = context.products.find(candidate => candidate.id === productId);
    if (!product) return { status: 'error', code: 'not_found', message: `Product ${productId} was not found.`, revision: context.revision } satisfies ToolError;
    return { status: 'ok', revision: context.revision, product: toToolProduct(product) };
  };
  sectionTool.execute = (args, executionContext) => {
    const context = session.context();
    if (requestWasCancelled(executionContext)) return cancelled(context.revision);
    const argumentError = validateArguments(args, ['section_id'], context.revision);
    if (argumentError) return argumentError;
    const sectionId = readRequiredString(args, 'section_id', 120, context.revision);
    if (typeof sectionId !== 'string') return sectionId;
    const section = getSection(context, sectionId);
    if (!section) return { status: 'error', code: 'not_found', message: `Section ${sectionId} was not found.`, revision: context.revision } satisfies ToolError;
    return { ...section, revision: context.revision };
  };
  validationTool.execute = (args, executionContext) => {
    const context = session.context();
    if (requestWasCancelled(executionContext)) return cancelled(context.revision);
    const argumentError = validateArguments(args, [], context.revision);
    if (argumentError) return argumentError;
    return { status: 'ok', ...session.validatePlanogram() };
  };
  addTool.execute = (args, executionContext) => {
    const context = session.context();
    if (requestWasCancelled(executionContext)) return cancelled(context.revision);
    const argumentError = validateArguments(args, ['product_id', 'shelf_id', 'expected_revision', 'reason'], context.revision);
    if (argumentError) return argumentError;
    const productId = readRequiredString(args, 'product_id', 120, context.revision);
    if (typeof productId !== 'string') return productId;
    const shelfId = readRequiredString(args, 'shelf_id', 120, context.revision);
    if (typeof shelfId !== 'string') return shelfId;
    const expectedRevision = readRequiredRevision(args, context.revision);
    if (typeof expectedRevision !== 'number') return expectedRevision;
    const reason = readOptionalString(args, 'reason', 240, context.revision);
    if (typeof reason !== 'string' && reason !== undefined) return reason;
    return mutationResult(session.addPlacement({ versionId: context.version_id, productId, shelfId, expectedRevision, reason }, 'webmcp'), session);
  };
  distributeTool.execute = (args, executionContext) => {
    const context = session.context();
    if (requestWasCancelled(executionContext)) return cancelled(context.revision);
    const argumentError = validateArguments(args, ['shelf_id', 'distribution', 'expected_revision', 'reason'], context.revision);
    if (argumentError) return argumentError;
    const shelfId = readRequiredString(args, 'shelf_id', 120, context.revision);
    if (typeof shelfId !== 'string') return shelfId;
    const distribution = readRequiredString(args, 'distribution', 40, context.revision);
    if (typeof distribution !== 'string') return distribution;
    if (!['packed_left', 'centered', 'space_between', 'space_evenly'].includes(distribution)) {
      return { status: 'error', code: 'invalid_input', message: 'distribution must be packed_left, centered, space_between, or space_evenly.', revision: context.revision } satisfies ToolError;
    }
    const expectedRevision = readRequiredRevision(args, context.revision);
    if (typeof expectedRevision !== 'number') return expectedRevision;
    const reason = readOptionalString(args, 'reason', 240, context.revision);
    if (typeof reason !== 'string' && reason !== undefined) return reason;
    return mutationResult(session.distributeShelf({
      versionId: context.version_id,
      shelfId,
      distribution: distribution as 'packed_left' | 'centered' | 'space_between' | 'space_evenly',
      expectedRevision,
      reason,
    }, 'webmcp'), session);
  };
  allocationPreviewTool.execute = (args, executionContext) => {
    const context = session.context();
    if (requestWasCancelled(executionContext)) return cancelled(context.revision);
    const argumentError = validateArguments(args, ['shelf_id', 'strategy', 'expected_revision', 'reason'], context.revision);
    if (argumentError) return argumentError;
    const shelfId = readRequiredString(args, 'shelf_id', 120, context.revision);
    if (typeof shelfId !== 'string') return shelfId;
    const strategy = readRequiredString(args, 'strategy', 40, context.revision);
    if (typeof strategy !== 'string') return strategy;
    if (strategy !== 'fill_evenly') {
      return { status: 'error', code: 'invalid_input', message: 'strategy must be fill_evenly.', revision: context.revision } satisfies ToolError;
    }
    const expectedRevision = readRequiredRevision(args, context.revision);
    if (typeof expectedRevision !== 'number') return expectedRevision;
    const reason = readOptionalString(args, 'reason', 240, context.revision);
    if (typeof reason !== 'string' && reason !== undefined) return reason;
    return previewResult(session.previewShelfAllocation({
      versionId: context.version_id,
      shelfId,
      strategy,
      expectedRevision,
      reason,
    }), session);
  };
  salesPreviewTool.execute = (args, executionContext) => {
    const context = session.context();
    if (requestWasCancelled(executionContext)) return cancelled(context.revision);
    const argumentError = validateArguments(args, ['scope', 'basis', 'target', 'min_facings', 'max_facings', 'expected_revision', 'reason'], context.revision);
    if (argumentError) return argumentError;
    const request = readSalesAllocationRequest(args, context.revision);
    if (isToolError(request)) return request;
    const expectedRevision = readRequiredRevision(args, context.revision);
    if (typeof expectedRevision !== 'number') return expectedRevision;
    const reason = readOptionalString(args, 'reason', 240, context.revision);
    if (typeof reason !== 'string' && reason !== undefined) return reason;
    return previewResult(session.previewSalesAllocation({ versionId: context.version_id, request, expectedRevision, reason }), session);
  };
  previewTool.execute = (args, executionContext) => {
    const context = session.context();
    if (requestWasCancelled(executionContext)) return cancelled(context.revision);
    const argumentError = validateArguments(args, ['expected_revision', 'operations', 'reason'], context.revision);
    if (argumentError) return argumentError;
    const expectedRevision = readRequiredRevision(args, context.revision);
    if (typeof expectedRevision !== 'number') return expectedRevision;
    const operations = readPlacementChanges(args, context.revision);
    if (isToolError(operations)) return operations;
    const reason = readOptionalString(args, 'reason', 240, context.revision);
    if (typeof reason !== 'string' && reason !== undefined) return reason;
    return previewResult(session.previewChanges({ versionId: context.version_id, expectedRevision, operations, reason }), session);
  };
  applyTool.execute = (args, executionContext) => {
    const context = session.context();
    if (requestWasCancelled(executionContext)) return cancelled(context.revision);
    const argumentError = validateArguments(args, ['proposal_id', 'expected_revision'], context.revision);
    if (argumentError) return argumentError;
    const proposalId = readRequiredString(args, 'proposal_id', 120, context.revision);
    if (typeof proposalId !== 'string') return proposalId;
    const expectedRevision = readRequiredRevision(args, context.revision);
    if (typeof expectedRevision !== 'number') return expectedRevision;
    return mutationResult(session.applyChanges({ versionId: context.version_id, proposalId, expectedRevision }, 'webmcp'), session);
  };
  facingsTool.execute = (args, executionContext) => {
    const context = session.context();
    if (requestWasCancelled(executionContext)) return cancelled(context.revision);
    const argumentError = validateArguments(args, ['placement_id', 'facings_x', 'facings_y', 'facings_z', 'expected_revision', 'reason'], context.revision);
    if (argumentError) return argumentError;
    const placementId = readRequiredString(args, 'placement_id', 120, context.revision);
    if (typeof placementId !== 'string') return placementId;
    const record = isRecord(args) ? args : {};
    const facingsX = readOptionalFacing(record, 'facings_x', context.revision);
    const facingsY = readOptionalFacing(record, 'facings_y', context.revision);
    const facingsZ = readOptionalFacing(record, 'facings_z', context.revision);
    for (const value of [facingsX, facingsY, facingsZ]) {
      if (isToolError(value)) return value;
    }
    const expectedRevision = readRequiredRevision(args, context.revision);
    if (typeof expectedRevision !== 'number') return expectedRevision;
    const reason = readOptionalString(args, 'reason', 240, context.revision);
    if (typeof reason !== 'string' && reason !== undefined) return reason;
    return mutationResult(session.setFacings({
      versionId: context.version_id,
      placementId,
      facingsX: facingsX as number | undefined,
      facingsY: facingsY as number | undefined,
      facingsZ: facingsZ as number | undefined,
      expectedRevision,
      reason,
    }, 'webmcp'), session);
  };
  undoTool.execute = (args, executionContext) => {
    const context = session.context();
    if (requestWasCancelled(executionContext)) return cancelled(context.revision);
    const argumentError = validateArguments(args, ['change_set_id', 'expected_revision'], context.revision);
    if (argumentError) return argumentError;
    const changeSetId = readRequiredString(args, 'change_set_id', 120, context.revision);
    if (typeof changeSetId !== 'string') return changeSetId;
    const expectedRevision = readRequiredRevision(args, context.revision);
    if (typeof expectedRevision !== 'number') return expectedRevision;
    return mutationResult(session.undoChangeSet({ versionId: context.version_id, changeSetId, expectedRevision }, 'webmcp'), session);
  };
  return tools;
}

export async function registerPlanogramWebMcp(session: PlanogramSession, getSelection: () => Selection | undefined): Promise<WebMcpRegistration> {
  const modelContext = document.modelContext;
  if (!modelContext || typeof modelContext.registerTool !== 'function') {
    return { status: 'unsupported', registeredNames: [] };
  }
  const tools = bindTools(session, getSelection);
  const registeredNames: string[] = [];
  try {
    for (const tool of tools) {
      await modelContext.registerTool(tool);
      registeredNames.push(tool.name);
    }
  } catch (cause) {
    if (modelContext.unregisterTool) {
      for (const name of registeredNames) await modelContext.unregisterTool(name);
    }
    return { status: 'error', registeredNames: [], error: cause instanceof Error ? cause.message : String(cause) };
  }
  return {
    status: 'ready',
    registeredNames,
    unregister: async () => {
      if (!modelContext.unregisterTool) return;
      for (const name of registeredNames) await modelContext.unregisterTool(name);
    },
  };
}
