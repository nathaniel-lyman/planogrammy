import type { CommandResult, ShelfDistribution, WasmEngine } from './types';

export type MoveSource = 'inspector' | 'keyboard' | 'pointer';

export interface MoveShelfInput {
  versionId: string;
  shelfId: string;
  elevationSixteenths: number;
  expectedRevision: number;
}

export function moveShelf(engine: WasmEngine, input: MoveShelfInput, source: MoveSource): CommandResult {
  return engine.move_shelf(input.versionId, input.shelfId, input.elevationSixteenths, input.expectedRevision, `${source} move`);
}

export interface MovePlacementInput {
  versionId: string;
  placementId: string;
  targetShelfId: string;
  xSixteenths: number;
  expectedRevision: number;
}

export function movePlacement(engine: WasmEngine, input: MovePlacementInput, source: MoveSource): CommandResult {
  return engine.move_placement(input.versionId, input.placementId, input.targetShelfId, input.xSixteenths, input.expectedRevision, `${source} move placement`);
}

export type DistributionSource = 'inspector' | 'webmcp';

export function distributeShelf(
  engine: WasmEngine,
  input: { versionId: string; shelfId: string; distribution: ShelfDistribution; expectedRevision: number; reason?: string },
  source: DistributionSource,
): CommandResult {
  const reason = input.reason ?? `${source} ${input.distribution.replaceAll('_', ' ')} distribution`;
  if (source === 'webmcp') {
    return engine.distribute_shelf_as(input.versionId, input.shelfId, input.distribution, input.expectedRevision, 'webmcp', reason);
  }
  return engine.distribute_shelf(input.versionId, input.shelfId, input.distribution, input.expectedRevision, reason);
}

export type PlacementSource = 'catalog_button' | 'catalog_double_click' | 'catalog_drag';

export type AddPlacementSource = PlacementSource | 'webmcp';

export function addPlacement(engine: WasmEngine, input: { versionId: string; productId: string; shelfId: string; expectedRevision: number; reason?: string }, source: AddPlacementSource): CommandResult {
  if (source === 'webmcp') {
    return engine.add_placement_as(input.versionId, input.productId, input.shelfId, input.expectedRevision, 'webmcp', input.reason ?? 'WebMCP add product');
  }
  return engine.add_placement(input.versionId, input.productId, input.shelfId, input.expectedRevision, `${source} add product`);
}

export type RemovalSource = 'inspector' | 'keyboard';

export function removePlacement(engine: WasmEngine, input: { versionId: string; placementId: string; expectedRevision: number }, source: RemovalSource): CommandResult {
  return engine.remove_placement(input.versionId, input.placementId, input.expectedRevision, `${source} remove product`);
}

export type UndoSource = 'human' | 'webmcp';

export function undoChangeSet(engine: WasmEngine, input: { versionId: string; changeSetId: string; expectedRevision: number }, source: UndoSource): CommandResult {
  if (source === 'webmcp') return engine.undo_change_set_as(input.versionId, input.changeSetId, input.expectedRevision, 'webmcp');
  return engine.undo_change_set(input.versionId, input.changeSetId, input.expectedRevision);
}

export function resultError(result: CommandResult): string | undefined {
  if (result.status === 'validation_failed') return result.validation.issues[0]?.message ?? 'The change could not be applied.';
  if (result.status === 'revision_conflict') return `Revision conflict: expected ${result.expected_revision}, current ${result.current_revision}.`;
  if (result.status === 'not_found') return `${result.entity} ${result.id} was not found.`;
  if (result.status === 'forbidden' || result.status === 'invalid_command') return result.message;
}
