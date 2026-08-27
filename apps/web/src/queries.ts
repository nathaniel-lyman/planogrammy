import type { ChangeSet, EngineContext, Placement, Product, Selection, Shelf, StockingMode } from './types';

export interface ToolProduct {
  id: string;
  upc: string;
  brand: string;
  description: string;
  size_oz: string;
  category: string;
  net_weight_ounces_hundredths: number;
  casepack_quantity: number;
  dimensions: {
    width_sixteenths: number;
    height_sixteenths: number;
    depth_sixteenths: number;
    source: string;
    confidence: string;
  };
  performance: {
    sales_per_store_per_week_cents: number;
    units_per_store_per_week_milliunits: number;
    gross_margin_basis_points: number;
    source: string;
    period: string;
  };
  tray: {
    facings_x: number;
    units_deep: number;
    outer_width_sixteenths: number;
    outer_height_sixteenths: number;
    outer_depth_sixteenths: number;
    front_lip_height_sixteenths: number;
  } | null;
}

export interface ToolPlacement {
  id: string;
  product_id: string;
  shelf_id: string;
  x_sixteenths: number;
  facings_x: number;
  facings_y: number;
  facings_z: number;
  stocking_mode: StockingMode;
  stocked_unit_count: number;
  display_width_sixteenths: number;
  display_height_sixteenths: number;
  required_depth_sixteenths: number;
  tray_front_lip_height_sixteenths: number | null;
}

interface ToolOperationPlacement {
  id: string;
  product_id: string;
  shelf_id: string;
  x_sixteenths: number;
  facings_x: number;
  facings_y: number;
  facings_z: number;
}

export interface ToolChangeSet {
  id: string;
  actor: string;
  reason: string;
  base_revision: number;
  resulting_revision: number;
  operations: Array<Record<string, unknown>>;
  compensates: string | null;
}

export interface PlanogramContextToolResult {
  status: 'ok';
  version_id: string;
  version_status: EngineContext['version_status'];
  revision: number;
  selection: Selection | null;
  fixture: {
    id: string;
    name: string;
    width_sixteenths: number;
    height_sixteenths: number;
    depth_sixteenths: number;
  };
  summary: {
    section_count: number;
    shelf_count: number;
    adjustable_shelf_count: number;
    product_count: number;
    placement_count: number;
    latest_change_set_id: string | null;
  };
}

export interface SectionToolResult {
  status: 'ok';
  section: {
    id: string;
    fixture_id: string;
    sequence: number;
    width_sixteenths: number;
    height_sixteenths: number;
    shelves: Array<{
      id: string;
      kind: Shelf['kind'];
      width_sixteenths: number;
      depth_sixteenths: number;
      elevation_sixteenths: number;
      vertical_clearance_sixteenths: number;
      available_capacity_sixteenths: number;
      placements: ToolPlacement[];
    }>;
  };
}

export function toToolProduct(product: Product): ToolProduct {
  return {
    id: product.id,
    upc: product.upc,
    brand: product.brand,
    description: product.description,
    size_oz: product.size_oz,
    category: product.category,
    net_weight_ounces_hundredths: product.net_weight_ounces_hundredths,
    casepack_quantity: product.casepack_quantity,
    dimensions: {
      width_sixteenths: product.dimensions.width,
      height_sixteenths: product.dimensions.height,
      depth_sixteenths: product.dimensions.depth,
      source: product.dimensions.source,
      confidence: product.dimensions.confidence,
    },
    performance: { ...product.performance },
    tray: product.tray ? {
      facings_x: product.tray.facings_x,
      units_deep: product.tray.units_deep,
      outer_width_sixteenths: product.tray.outer_width_sixteenths,
      outer_height_sixteenths: product.tray.outer_height_sixteenths,
      outer_depth_sixteenths: product.tray.outer_depth_sixteenths,
      front_lip_height_sixteenths: product.tray.front_lip_height_sixteenths,
    } : null,
  };
}

export function toToolPlacement(placement: Placement): ToolPlacement {
  return {
    id: placement.id,
    product_id: placement.product_id,
    shelf_id: placement.shelf_id,
    x_sixteenths: placement.x,
    facings_x: placement.facings_x,
    facings_y: placement.facings_y,
    facings_z: placement.facings_z,
    stocking_mode: placement.stocking_mode,
    stocked_unit_count: placement.stocked_unit_count,
    display_width_sixteenths: placement.geometry.display_width,
    display_height_sixteenths: placement.geometry.display_height,
    required_depth_sixteenths: placement.geometry.required_depth,
    tray_front_lip_height_sixteenths: placement.tray_front_lip_height ?? null,
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isIntegerNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isInteger(value);
}

function serializedPlacement(value: unknown): ToolOperationPlacement | undefined {
  if (!isRecord(value)) return undefined;
  const id = value.id;
  const productId = value.product_id;
  const shelfId = value.shelf_id;
  const x = value.x;
  const facingsX = value.facings_x;
  const facingsY = value.facings_y;
  const facingsZ = value.facings_z;
  if (typeof id !== 'string' || typeof productId !== 'string' || typeof shelfId !== 'string') return undefined;
  if (!isIntegerNumber(x) || !isIntegerNumber(facingsX) || !isIntegerNumber(facingsY) || !isIntegerNumber(facingsZ)) return undefined;
  return { id, product_id: productId, shelf_id: shelfId, x_sixteenths: x, facings_x: facingsX, facings_y: facingsY, facings_z: facingsZ };
}

function serializedLocation(value: unknown): { shelf_id: string; x_sixteenths: number } | undefined {
  if (!isRecord(value) || typeof value.shelf_id !== 'string' || !isIntegerNumber(value.x)) return undefined;
  return { shelf_id: value.shelf_id, x_sixteenths: value.x };
}

export function toToolOperation(value: unknown): Record<string, unknown> {
  if (!isRecord(value) || typeof value.type !== 'string') return { type: 'unknown' };
  switch (value.type) {
    case 'move_shelf':
      if (typeof value.shelf_id === 'string' && isIntegerNumber(value.before) && isIntegerNumber(value.after)) {
        return { type: value.type, shelf_id: value.shelf_id, before_elevation_sixteenths: value.before, after_elevation_sixteenths: value.after };
      }
      break;
    case 'move_placement': {
      const before = serializedLocation(value.before);
      const after = serializedLocation(value.after);
      if (typeof value.placement_id === 'string' && before && after) return { type: value.type, placement_id: value.placement_id, before, after };
      break;
    }
    case 'add_placement': {
      const placement = serializedPlacement(value.placement);
      if (placement) return { type: value.type, placement };
      break;
    }
    case 'remove_placement': {
      const placement = serializedPlacement(value.placement);
      if (placement) return { type: value.type, placement };
      break;
    }
    default:
      break;
  }
  return { type: 'unknown' };
}

export function toToolChangeSet(changeSet: ChangeSet): ToolChangeSet {
  return {
    id: changeSet.id,
    actor: changeSet.actor,
    reason: changeSet.reason,
    base_revision: changeSet.base_revision,
    resulting_revision: changeSet.resulting_revision,
    operations: changeSet.operations.map(toToolOperation),
    compensates: changeSet.compensates ?? null,
  };
}

export function getPlanogramContext(context: EngineContext, selection: Selection | undefined): PlanogramContextToolResult {
  const shelves = context.fixture.sections.flatMap(section => section.shelves);
  return {
    status: 'ok',
    version_id: context.version_id,
    version_status: context.version_status,
    revision: context.revision,
    selection: selection ?? null,
    fixture: {
      id: context.fixture.id,
      name: context.fixture.name,
      width_sixteenths: context.fixture.width,
      height_sixteenths: context.fixture.height,
      depth_sixteenths: context.fixture.depth,
    },
    summary: {
      section_count: context.fixture.sections.length,
      shelf_count: shelves.length,
      adjustable_shelf_count: shelves.filter(shelf => shelf.kind === 'adjustable').length,
      product_count: context.products.length,
      placement_count: context.placements.length,
      latest_change_set_id: context.latest_change_set_id ?? null,
    },
  };
}

export interface ProductSearchInput {
  query?: string;
  upc?: string;
  brand?: string;
  category?: string;
  stocking_mode?: StockingMode;
  limit?: number;
}

export function searchProducts(products: Product[], input: ProductSearchInput = {}): Product[] {
  const query = input.query?.trim().toLowerCase() ?? '';
  const upc = input.upc?.trim().toLowerCase() ?? '';
  const brand = input.brand?.trim().toLowerCase() ?? '';
  const category = input.category?.trim().toLowerCase() ?? '';
  const stockingMode = input.stocking_mode;
  const queryTerms = query.split(/\s+/).filter(Boolean);
  const limit = input.limit ?? 25;
  return products
    .filter(product => {
      const haystack = `${product.id} ${product.upc} ${product.brand} ${product.description} ${product.size_oz} ${product.category} ${product.performance.source} ${product.performance.period} ${product.tray ? 'shelf-ready tray' : 'loose'}`.toLowerCase();
      return (queryTerms.length === 0 || queryTerms.every(term => haystack.includes(term)))
        && (!upc || product.upc.toLowerCase() === upc)
        && (!brand || product.brand.toLowerCase() === brand)
        && (!category || product.category.toLowerCase() === category)
        && (!stockingMode || (stockingMode === 'tray' ? Boolean(product.tray) : !product.tray));
    })
    .slice(0, Math.max(1, Math.min(50, limit)))
}

const MIN_PLACEMENT_GAP_SIXTEENTHS = 2;
const PLACEMENT_X_GRID_SIXTEENTHS = 2;

function firstAlignedPlacementXAfter(end: number): number {
  const minimumX = end + MIN_PLACEMENT_GAP_SIXTEENTHS;
  return Math.ceil(minimumX / PLACEMENT_X_GRID_SIXTEENTHS) * PLACEMENT_X_GRID_SIXTEENTHS;
}

function maxPlaceableWidth(shelf: Shelf, placements: Placement[]): number {
  const occupied = placements
    .map(placement => ({
      start: placement.x,
      end: placement.x + placement.geometry.display_width,
    }))
    .sort((left, right) => left.start - right.start);

  if (occupied.length === 0) return shelf.width;

  let previousEnd: number | undefined;
  let largestCapacity = 0;
  for (const segment of occupied) {
    const availableStart = previousEnd === undefined ? 0 : firstAlignedPlacementXAfter(previousEnd);
    const availableEnd = segment.start - MIN_PLACEMENT_GAP_SIXTEENTHS;
    largestCapacity = Math.max(largestCapacity, availableEnd - availableStart);
    previousEnd = Math.max(previousEnd ?? segment.end, segment.end);
  }

  const trailingStart = firstAlignedPlacementXAfter(previousEnd ?? 0);
  return Math.max(0, largestCapacity, shelf.width - trailingStart);
}

export function getSection(context: EngineContext, sectionId: string): SectionToolResult | undefined {
  const section = context.fixture.sections.find(candidate => candidate.id === sectionId);
  if (!section) return undefined;
  return {
    status: 'ok',
    section: {
      id: section.id,
      fixture_id: context.fixture.id,
      sequence: section.sequence ?? 0,
      width_sixteenths: section.width ?? context.fixture.width,
      height_sixteenths: section.height ?? context.fixture.height,
      shelves: section.shelves.map(shelf => {
        const nextElevation = section.shelves
          .map(candidate => candidate.elevation)
          .filter(elevation => elevation > shelf.elevation)
          .sort((left, right) => left - right)[0] ?? section.height ?? context.fixture.height;
        const shelfPlacements = context.placements.filter(placement => placement.shelf_id === shelf.id);
        return {
          id: shelf.id,
          kind: shelf.kind,
          width_sixteenths: shelf.width,
          depth_sixteenths: shelf.depth,
          elevation_sixteenths: shelf.elevation,
          vertical_clearance_sixteenths: nextElevation - shelf.elevation,
          available_capacity_sixteenths: maxPlaceableWidth(shelf, shelfPlacements),
          placements: shelfPlacements.map(toToolPlacement),
        };
      }),
    },
  };
}
