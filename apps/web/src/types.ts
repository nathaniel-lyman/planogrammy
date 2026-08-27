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
  version_id: string;
  version_status: 'draft' | 'proposed' | 'published' | 'archived';
  revision: number;
  fixture: Fixture;
  products: Product[];
  placements: Placement[];
  latest_change_set_id?: string;
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

export type ShelfDistribution = 'packed_left' | 'centered' | 'space_between' | 'space_evenly';

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
  | { status: 'ready'; revision: number; operations: unknown[]; affected_ids: string[]; validation: { issues: ValidationIssue[] }; preview_scene: RenderScene }
  | { status: 'validation_failed'; revision: number; validation: { issues: ValidationIssue[] } }
  | { status: 'revision_conflict'; expected_revision: number; current_revision: number }
  | { status: 'not_found'; entity: string; id: string }
  | { status: 'forbidden' | 'invalid_command'; message: string };

export interface WasmEngine {
  initialize_renderer(canvasId: string): Promise<void>;
  context(): EngineContext;
  validate_planogram(): PlanogramValidationResult;
  move_shelf(versionId: string, shelfId: string, elevationSixteenths: number, expectedRevision: number, reason: string): CommandResult;
  move_placement(versionId: string, placementId: string, targetShelfId: string, xSixteenths: number, expectedRevision: number, reason: string): CommandResult;
  distribute_shelf(versionId: string, shelfId: string, distribution: ShelfDistribution, expectedRevision: number, reason: string): CommandResult;
  distribute_shelf_as(versionId: string, shelfId: string, distribution: ShelfDistribution, expectedRevision: number, actor: string, reason: string): CommandResult;
  add_placement(versionId: string, productId: string, shelfId: string, expectedRevision: number, reason: string): CommandResult;
  add_placement_as(versionId: string, productId: string, shelfId: string, expectedRevision: number, actor: string, reason: string): CommandResult;
  remove_placement(versionId: string, placementId: string, expectedRevision: number, reason: string): CommandResult;
  preview_changes(versionId: string, expectedRevision: number, changes: PlacementChange[]): PreviewResult;
  clear_proposal_preview(): void;
  apply_changes_as(versionId: string, expectedRevision: number, changes: PlacementChange[], actor: string, reason: string): CommandResult;
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
