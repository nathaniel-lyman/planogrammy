export type ShelfKind = 'base_deck' | 'adjustable';

export interface Shelf {
  id: string;
  section_id: string;
  kind: ShelfKind;
  width: number;
  depth: number;
  elevation: number;
}

export interface Section {
  id: string;
  fixture_id: string;
  sequence: number;
  width: number;
  height: number;
  shelves: Shelf[];
}

export interface Fixture {
  id: string;
  name: string;
  width: number;
  height: number;
  depth: number;
  sections: Section[];
}

export interface EngineContext {
  document_revision?: number;
  scenario?: ScenarioView | null;
  version_id: string;
  version_status: 'draft' | 'proposed' | 'published' | 'archived';
  revision: number;
  fixture: Fixture;
  products: Product[];
  placements: Placement[];
  latest_change_set_id?: string;
  latest_undoable_change_set_id?: string;
}

export interface ProductPerformance {
  sales_per_store_per_week_cents: number;
  units_per_store_per_week_milliunits: number;
  gross_margin_basis_points: number;
  source: string;
  period: string;
}

export interface TrayConfiguration {
  facings_x: number;
  units_deep: number;
  outer_width_sixteenths: number;
  outer_height_sixteenths: number;
  outer_depth_sixteenths: number;
  front_lip_height_sixteenths: number;
}

export interface Product {
  id: string;
  upc: string;
  brand: string;
  description: string;
  size_oz: string;
  category: string;
  dimensions: { width: number; height: number; depth: number; source: string; confidence: string };
  net_weight_ounces_hundredths: number;
  casepack_quantity: number;
  performance: ProductPerformance;
  tray?: TrayConfiguration | null;
  color: [number, number, number];
  lid_color: [number, number, number];
}

export type StockingMode = 'loose' | 'tray';

export interface Placement {
  id: string;
  product_id: string;
  shelf_id: string;
  x: number;
  stocking_mode: StockingMode;
  facings_x: number;
  facings_y: number;
  facings_z: number;
  stocked_unit_count: number;
  geometry: {
    display_width: number;
    display_height: number;
    required_depth: number;
  };
  tray_front_lip_height?: number | null;
}

export interface PlacementSceneNode {
  id: string;
  product_id: string;
  shelf_id: string;
  x: number;
  width: number;
  height: number;
  required_depth: number;
  stocking_mode: StockingMode;
  stocked_unit_count: number;
  facings_x: number;
  facings_y: number;
  facings_z: number;
  tray_front_lip_height?: number | null;
  color: [number, number, number];
  lid_color: [number, number, number];
}

export interface RenderScene {
  revision: number;
  fixture_id: string;
  width: number;
  height: number;
  shelves: Array<{ id: string; kind: ShelfKind; width: number; depth: number; elevation: number }>;
  placements: PlacementSceneNode[];
}

/** Requested facing counts; an omitted count keeps the placement's current value in Rust. */
export interface FacingsRequest {
  facingsX?: number;
  facingsY?: number;
  facingsZ?: number;
}

export type ShelfDistribution = 'packed_left' | 'centered' | 'space_between' | 'space_evenly';
export type ShelfAllocationStrategy = 'fill_evenly';

export type SalesAllocationScope =
  | { kind: 'shelf'; shelf_id: string }
  | { kind: 'bay'; section_id: string };
export type SalesAllocationBasis = 'revenue' | 'units';
export type SalesAllocationTarget = 'space' | 'facings';

/** Semantic intent only; Rust resolves all physical positions and facing counts. */
export interface SalesAllocationRequest {
  scope: SalesAllocationScope;
  basis: SalesAllocationBasis;
  target: SalesAllocationTarget;
  min_facings: number;
  max_facings: number;
}

export interface SalesAllocationRow {
  product_id: string;
  contribution_basis_points: number;
  before_facings: number;
  after_facings: number;
  before_space_sixteenths: number;
  after_space_sixteenths: number;
  before_share_basis_points: number;
  after_share_basis_points: number;
}

export interface SalesAllocationReport {
  basis: SalesAllocationBasis;
  target: SalesAllocationTarget;
  scope: SalesAllocationScope;
  product_count: number;
  shelf_count: number;
  fixed_tray_count: number;
  zero_weight_sku_count: number;
  period: string;
  source: string;
  rows: SalesAllocationRow[];
  warnings: string[];
}

export interface ChangeSet {
  id: string;
  actor: string;
  reason: string;
  base_revision: number;
  resulting_revision: number;
  operations: unknown[];
  compensates?: string;
}

export type Selection =
  | { kind: 'shelf'; id: string }
  | { kind: 'placement'; id: string };

export type HitTarget =
  | { kind: 'shelf'; id: string }
  | { kind: 'placement'; id: string; shelf_id: string };

export interface ValidationIssue { code: string; message: string; shelf_id?: string }

export interface PlanogramValidationResult {
  revision: number;
  valid: boolean;
  validation: { issues: ValidationIssue[] };
}

export type CommandResult =
  | { status: 'applied'; revision: number; change_set: ChangeSet; affected_ids: string[]; validation: { issues: ValidationIssue[] }; scene_patch: { revision: number; shelves: unknown[]; placements: unknown[]; removed_placement_ids: string[] } }
  | { status: 'validation_failed'; revision: number; validation: { issues: ValidationIssue[] } }
  | { status: 'revision_conflict'; expected_revision: number; current_revision: number }
  | { status: 'not_found'; entity: string; id: string }
  | { status: 'forbidden' | 'invalid_command'; message: string };

export type PlacementChange =
  | { kind: 'add'; product_id: string; shelf_id: string; sequence: number; facings_x?: number; facings_y?: number; facings_z?: number }
  | { kind: 'move'; placement_id: string; shelf_id: string; sequence: number }
  | { kind: 'remove'; placement_id: string };

export type PreviewResult =
  | { status: 'ready'; revision: number; operations: unknown[]; affected_ids: string[]; validation: { issues: ValidationIssue[] }; preview_scene: RenderScene; sales_allocation?: SalesAllocationReport }
  | { status: 'validation_failed'; revision: number; validation: { issues: ValidationIssue[] } }
  | { status: 'revision_conflict'; expected_revision: number; current_revision: number }
  | { status: 'not_found'; entity: string; id: string }
  | { status: 'forbidden' | 'invalid_command'; message: string };

export interface WasmEngine {
  start_cereal(seed:number, expectedDocumentRevision:number):void;
  select_alternative(index:number, expectedDocumentRevision:number):void;
  duplicate_alternative(expectedDocumentRevision:number):void;
  focus_bay(shelfId:string):void;
  export_bay(name: string): string;
  inspect_bay(json: string): string;
  restore_bay(json: string, expectedRevision: number): string;
  initialize_renderer(canvasId: string): Promise<void>;
  context(): EngineContext;
  validate_planogram(): PlanogramValidationResult;
  move_shelf(versionId: string, shelfId: string, elevationSixteenths: number, expectedRevision: number, reason: string): CommandResult;
  move_placement(versionId: string, placementId: string, targetShelfId: string, xSixteenths: number, expectedRevision: number, reason: string): CommandResult;
  set_facings(versionId: string, placementId: string, facingsX: number | undefined, facingsY: number | undefined, facingsZ: number | undefined, expectedRevision: number, reason: string): CommandResult;
  set_facings_as(versionId: string, placementId: string, facingsX: number | undefined, facingsY: number | undefined, facingsZ: number | undefined, expectedRevision: number, actor: string, reason: string): CommandResult;
  distribute_shelf(versionId: string, shelfId: string, distribution: ShelfDistribution, expectedRevision: number, reason: string): CommandResult;
  distribute_shelf_as(versionId: string, shelfId: string, distribution: ShelfDistribution, expectedRevision: number, actor: string, reason: string): CommandResult;
  add_placement(versionId: string, productId: string, shelfId: string, expectedRevision: number, reason: string): CommandResult;
  add_placement_as(versionId: string, productId: string, shelfId: string, expectedRevision: number, actor: string, reason: string): CommandResult;
  remove_placement(versionId: string, placementId: string, expectedRevision: number, reason: string): CommandResult;
  preview_changes(versionId: string, expectedRevision: number, changes: PlacementChange[]): PreviewResult;
  preview_shelf_allocation(versionId: string, shelfId: string, strategy: ShelfAllocationStrategy, expectedRevision: number): PreviewResult;
  preview_sales_allocation(versionId: string, request: SalesAllocationRequest, expectedRevision: number): PreviewResult;
  clear_proposal_preview(): void;
  apply_changes_as(versionId: string, expectedRevision: number, changes: PlacementChange[], actor: string, reason: string): CommandResult;
  apply_shelf_allocation_as(versionId: string, shelfId: string, strategy: ShelfAllocationStrategy, expectedRevision: number, actor: string, reason: string): CommandResult;
  apply_sales_allocation_as(versionId: string, request: SalesAllocationRequest, expectedRevision: number, actor: string, reason: string): CommandResult;
  undo_change_set(versionId: string, changeSetId: string, expectedRevision: number): CommandResult;
  undo_change_set_as(versionId: string, changeSetId: string, expectedRevision: number, actor: string): CommandResult;
  resize(width: number, height: number): void;
  hit_test(x: number, y: number): HitTarget | undefined;
  select_shelf(shelfId: string): void;
  select_placement(placementId: string): void;
  clear_selection(): void;
  begin_drag(shelfId: string, pointerY: number): boolean;
  preview_drag(pointerY: number): number | undefined;
  finish_drag(): [string, number] | undefined;
  cancel_drag(): void;
  zoom_by(factor: number): void;
  pan_by(dx: number, dy: number): void;
  fit_fixture(): void;
}

export interface ScenarioMetrics {
 bay_count:number; distinct_sku_count:number; expected_sku_count:number; unplaced_product_ids:string[]; capacity_units:number; weekly_demand_milliunits:number; stocked_weekly_demand_milliunits:number; aggregate_days_supply_millidays:number|null; replenishment_turnovers_per_week_milli:number; below_seven_days_sku_count:number; validation_issue_count:number; within_six_bay_limit:boolean;
}
export interface ScenarioView { seed:number; active:number|null; alternatives:string[]; comparison:{assumptions:Record<string,string|number>;baseline:ScenarioMetrics;current:ScenarioMetrics;products:Array<{product_id:string;description:string;weekly_demand_milliunits:number;baseline_capacity_units:number;current_capacity_units:number;baseline_days_supply_millidays:number|null;current_days_supply_millidays:number|null}>} }
