import { describe, expect, it, vi } from 'vitest';
import { addPlacement, distributeShelf, movePlacement, moveShelf, removePlacement } from './commands';
import type { WasmEngine } from './types';

describe('one semantic shelf command path', () => {
  it('preserves exact sixteenths for inspector, keyboard, and pointer completion', () => {
    const move_shelf = vi.fn(() => ({ status: 'applied', revision: 1, change_set: { id: 'c1' }, affected_ids: ['shelf_01'], validation: { issues: [] }, scene_patch: { revision: 1, shelves: [{}] } }));
    const engine = { move_shelf } as unknown as WasmEngine;
    for (const source of ['inspector', 'keyboard', 'pointer'] as const) moveShelf(engine, { versionId: 'v1', shelfId: 'shelf_01', elevationSixteenths: 488, expectedRevision: 0 }, source);
    expect(move_shelf).toHaveBeenCalledTimes(3);
    expect(move_shelf.mock.calls.map(call => call.slice(0, 4))).toEqual(Array(3).fill(['v1', 'shelf_01', 488, 0]));
  });
});

describe('semantic product placement', () => {
  it('routes inspector, keyboard, and pointer completion through one move command', () => {
    const move_placement = vi.fn(() => ({ status: 'applied', revision: 1, change_set: { id: 'c1' }, affected_ids: ['placement_0001'], validation: { issues: [] }, scene_patch: { revision: 1, shelves: [], placements: [{}], removed_placement_ids: [] } }));
    const engine = { move_placement } as unknown as WasmEngine;
    for (const source of ['inspector', 'keyboard', 'pointer'] as const) {
      movePlacement(engine, {
        versionId: 'v1',
        placementId: 'placement_0001',
        targetShelfId: 'shelf_02',
        xSixteenths: 402,
        expectedRevision: 7,
      }, source);
    }
    expect(move_placement).toHaveBeenCalledTimes(3);
    expect(move_placement.mock.calls.map(call => call.slice(0, 5))).toEqual(Array(3).fill(['v1', 'placement_0001', 'shelf_02', 402, 7]));
    expect((move_placement.mock.calls as unknown[][]).map(call => call[5])).toEqual(['inspector move placement', 'keyboard move placement', 'pointer move placement']);
  });

  it('passes product, shelf, and revision to the Rust command', () => {
    const add_placement = vi.fn(() => ({ status: 'applied', revision: 1, change_set: { id: 'c1' }, affected_ids: ['placement_0001'], validation: { issues: [] }, scene_patch: { revision: 1, shelves: [], placements: [{}], removed_placement_ids: [] } }));
    const engine = { add_placement } as unknown as WasmEngine;
    addPlacement(engine, { versionId: 'v1', productId: 'jif_creamy_16', shelfId: 'shelf_01', expectedRevision: 7 }, 'catalog_drag');
    expect(add_placement).toHaveBeenCalledWith('v1', 'jif_creamy_16', 'shelf_01', 7, 'catalog_drag add product');
  });

  it('passes placement identity and revision to the Rust removal command', () => {
    const remove_placement = vi.fn(() => ({ status: 'applied', revision: 8, change_set: { id: 'c8' }, affected_ids: ['placement_0001'], validation: { issues: [] }, scene_patch: { revision: 8, shelves: [], placements: [], removed_placement_ids: ['placement_0001'] } }));
    const engine = { remove_placement } as unknown as WasmEngine;
    removePlacement(engine, { versionId: 'v1', placementId: 'placement_0001', expectedRevision: 7 }, 'keyboard');
    expect(remove_placement).toHaveBeenCalledWith('v1', 'placement_0001', 7, 'keyboard remove product');
  });

  it('routes shelf distribution intent without calculating coordinates in TypeScript', () => {
    const distribute_shelf = vi.fn(() => ({ status: 'applied', revision: 8, change_set: { id: 'c8' }, affected_ids: ['placement_0001', 'placement_0002'], validation: { issues: [] }, scene_patch: { revision: 8, shelves: [], placements: [{}, {}], removed_placement_ids: [] } }));
    const engine = { distribute_shelf } as unknown as WasmEngine;
    distributeShelf(engine, { versionId: 'v1', shelfId: 'shelf_01', distribution: 'space_evenly', expectedRevision: 7 }, 'inspector');
    expect(distribute_shelf).toHaveBeenCalledWith('v1', 'shelf_01', 'space_evenly', 7, 'inspector space evenly distribution');
  });
});
