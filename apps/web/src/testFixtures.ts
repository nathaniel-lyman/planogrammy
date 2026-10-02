import type { CommandResult, EngineContext, Placement, PreviewResult, Product, SalesAllocationRequest, Shelf } from './types';

// Shared Vitest data mirroring the Rust-owned representative catalog and fixture.
// Tests use these transport shapes as canned Wasm responses; they never replace Rust geometry.

export const VERSION_ID = 'version_draft_01';
export const PERFORMANCE_SOURCE = 'Synthetic representative 13-week average; not retailer actuals';

export const trayProduct: Product = {
  id: 'jif_creamy_16',
  upc: '051500255001',
  brand: 'Jif',
  description: 'Creamy Peanut Butter',
  size_oz: '16 oz',
  category: 'Peanut Butter',
  dimensions: { width: 57, height: 130, depth: 45, source: 'fixture', confidence: 'high' },
  net_weight_ounces_hundredths: 1600,
  casepack_quantity: 12,
  performance: {
    sales_per_store_per_week_cents: 3665,
    units_per_store_per_week_milliunits: 10500,
    gross_margin_basis_points: 2850,
    source: PERFORMANCE_SOURCE,
    period: 'Trailing 13 weeks',
  },
  tray: {
    facings_x: 3,
    units_deep: 4,
    outer_width_sixteenths: 175,
    outer_height_sixteenths: 80,
    outer_depth_sixteenths: 232,
    front_lip_height_sixteenths: 20,
  },
  color: [40, 100, 60],
  lid_color: [210, 30, 40],
};

export const looseProduct: Product = {
  ...trayProduct,
  id: 'jif_creamy_40',
  upc: '051500720004',
  size_oz: '40 oz',
  tray: null,
};

/** A loaded Jif 16 oz tray placement as Rust reports it. */
export function trayPlacement(overrides: Partial<Placement> = {}): Placement {
  return {
    id: 'placement_0001',
    product_id: trayProduct.id,
    shelf_id: 'shelf_01',
    x: 0,
    stocking_mode: 'tray',
    facings_x: 3,
    facings_y: 1,
    facings_z: 4,
    stocked_unit_count: 12,
    geometry: { display_width: 175, display_height: 80, required_depth: 232 },
    tray_front_lip_height: 20,
    ...overrides,
  };
}

/** A single-facing loose Jif 40 oz placement as Rust reports it. */
export function loosePlacement(overrides: Partial<Placement> = {}): Placement {
  return trayPlacement({
    id: 'placement_0002',
    product_id: looseProduct.id,
    stocking_mode: 'loose',
    facings_x: 1,
    facings_z: 1,
    stocked_unit_count: 1,
    geometry: { display_width: 57, display_height: 130, required_depth: 45 },
    tray_front_lip_height: null,
    ...overrides,
  });
}

export function adjustableShelf(id: string, elevation: number, width = 768): Shelf {
  return { id, section_id: 'section_01', kind: 'adjustable', width, depth: 256, elevation };
}

export const baseDeck: Shelf = { id: 'base_deck', section_id: 'section_01', kind: 'base_deck', width: 768, depth: 352, elevation: 0 };

/** A draft context with one section holding `shelves` (no section when omitted). */
export function makeContext({ shelves, width = 768, ...overrides }: Partial<EngineContext> & { shelves?: Shelf[]; width?: number } = {}): EngineContext {
  return {
    version_id: VERSION_ID,
    version_status: 'draft',
    revision: 0,
    fixture: {
      id: 'fixture_standard_4ft',
      name: "4' Standard Bay",
      width,
      height: 1344,
      depth: 352,
      sections: shelves ? [{ id: 'section_01', fixture_id: 'fixture_standard_4ft', sequence: 0, width, height: 1344, shelves }] : [],
    },
    products: [],
    placements: [],
    ...overrides,
  };
}

export function changeSetId(revision: number): string {
  return `change_${String(revision).padStart(4, '0')}`;
}

export function revisionConflict(expectedRevision: number, currentRevision: number): CommandResult & PreviewResult {
  return { status: 'revision_conflict', expected_revision: expectedRevision, current_revision: currentRevision };
}

export function appliedResult(
  revision: number,
  { actor = 'human', reason = '', operations = [], affectedIds = [] }: { actor?: string; reason?: string; operations?: unknown[]; affectedIds?: string[] } = {},
): Extract<CommandResult, { status: 'applied' }> {
  return {
    status: 'applied',
    revision,
    change_set: { id: changeSetId(revision), actor, reason, base_revision: revision - 1, resulting_revision: revision, operations },
    affected_ids: affectedIds,
    validation: { issues: [] },
    scene_patch: { revision, shelves: [], placements: affectedIds.map(() => ({})), removed_placement_ids: [] },
  };
}

export function readyPreview(revision: number, operations: unknown[] = [{ type: 'add_placement' }]): Extract<PreviewResult, { status: 'ready' }> {
  return {
    status: 'ready',
    revision,
    operations,
    affected_ids: [],
    validation: { issues: [] },
    preview_scene: { revision, fixture_id: 'fixture_standard_4ft', width: 768, height: 1344, shelves: [], placements: [] },
  };
}

export function salesAllocationRequest(overrides: Partial<SalesAllocationRequest> = {}): SalesAllocationRequest {
  return { scope: { kind: 'shelf', shelf_id: 'shelf_01' }, basis: 'revenue', target: 'facings', min_facings: 1, max_facings: 6, ...overrides };
}

/** Canned Rust report: a fixed tray and a loose SKU with equal synthetic demand. */
export function readySalesAllocationPreview(revision: number, request = salesAllocationRequest()): Extract<PreviewResult, { status: 'ready' }> {
  return {
    ...readyPreview(revision, [reflowOperation(loosePlacement(), { x: 300, facings_x: 3 })]),
    sales_allocation: {
      scope: { ...request.scope },
      basis: request.basis,
      target: request.target,
      product_count: 2,
      shelf_count: 1,
      fixed_tray_count: 1,
      zero_weight_sku_count: 0,
      period: 'Trailing 13 weeks',
      source: PERFORMANCE_SOURCE,
      rows: [
        { product_id: trayProduct.id, contribution_basis_points: 5000, before_facings: 3, after_facings: 3, before_space_sixteenths: 175, after_space_sixteenths: 175, before_share_basis_points: 7500, after_share_basis_points: 5000 },
        { product_id: looseProduct.id, contribution_basis_points: 5000, before_facings: 1, after_facings: 3, before_space_sixteenths: 57, after_space_sixteenths: 171, before_share_basis_points: 2500, after_share_basis_points: 5000 },
      ],
      warnings: ['Synthetic inputs describe allocation, not a forecast of sales uplift.'],
    },
  };
}

/** A Rust-shaped reflow operation from `before` to `after` configuration. */
export function reflowOperation(placement: Placement, after: Partial<Pick<Placement, 'shelf_id' | 'x' | 'facings_x'>>) {
  const configuration = (source: Placement) => ({ shelf_id: source.shelf_id, x: source.x, facings_x: source.facings_x, facings_y: source.facings_y, facings_z: source.facings_z });
  return { type: 'reflow_placement', placement_id: placement.id, before: configuration(placement), after: configuration({ ...placement, ...after }) };
}

/** Opaque transport text; only Rust reads or resolves the document contents. */
export const BAY_FILE_TEXT = '{"format":"planogrammy-bay","format_version":1,"name":"Peanut butter bay","draft":{}}';
