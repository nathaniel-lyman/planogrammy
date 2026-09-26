import { describe, expect, it, vi } from 'vitest';
import { addPlacement, distributeShelf, movePlacement, moveShelf, removePlacement, setFacings } from './commands';
import { appliedResult } from './testFixtures';
import type { WasmEngine } from './types';

// These tests only check what reaches the Rust command; the result shape is irrelevant.
function engineWith<Name extends keyof WasmEngine>(name: Name) {
  const command = vi.fn(() => appliedResult(1));
  return { command, engine: { [name]: command } as unknown as WasmEngine };
}

describe('one semantic shelf command path', () => {
  it('preserves exact sixteenths for inspector, keyboard, and pointer completion', () => {
    const { command, engine } = engineWith('move_shelf');
    for (const source of ['inspector', 'keyboard', 'pointer'] as const) moveShelf(engine, { versionId: 'v1', shelfId: 'shelf_01', elevationSixteenths: 488, expectedRevision: 0 }, source);
    expect(command.mock.calls.map(call => (call as unknown[]).slice(0, 4))).toEqual(Array(3).fill(['v1', 'shelf_01', 488, 0]));
  });
});

describe('semantic product placement', () => {
  it('routes inspector, keyboard, and pointer completion through one move command', () => {
    const { command, engine } = engineWith('move_placement');
    for (const source of ['inspector', 'keyboard', 'pointer'] as const) {
      movePlacement(engine, { versionId: 'v1', placementId: 'placement_0001', targetShelfId: 'shelf_02', xSixteenths: 402, expectedRevision: 7 }, source);
    }
    expect(command.mock.calls).toEqual(['inspector', 'keyboard', 'pointer'].map(source => ['v1', 'placement_0001', 'shelf_02', 402, 7, `${source} move placement`]));
  });

  it('passes product, shelf, and revision to the Rust command', () => {
    const { command, engine } = engineWith('add_placement');
    addPlacement(engine, { versionId: 'v1', productId: 'jif_creamy_16', shelfId: 'shelf_01', expectedRevision: 7 }, 'catalog_drag');
    expect(command).toHaveBeenCalledWith('v1', 'jif_creamy_16', 'shelf_01', 7, 'catalog_drag add product');
  });

  it('passes placement identity and revision to the Rust removal command', () => {
    const { command, engine } = engineWith('remove_placement');
    removePlacement(engine, { versionId: 'v1', placementId: 'placement_0001', expectedRevision: 7 }, 'keyboard');
    expect(command).toHaveBeenCalledWith('v1', 'placement_0001', 7, 'keyboard remove product');
  });

  it('routes inspector and keyboard facing intent through one Rust command, leaving omitted counts to Rust', () => {
    const { command, engine } = engineWith('set_facings');
    setFacings(engine, { versionId: 'v1', placementId: 'placement_0001', facingsX: 3, facingsY: 2, facingsZ: 1, expectedRevision: 7 }, 'inspector');
    setFacings(engine, { versionId: 'v1', placementId: 'placement_0001', facingsX: 4, expectedRevision: 8 }, 'keyboard');
    expect(command.mock.calls).toEqual([
      ['v1', 'placement_0001', 3, 2, 1, 7, 'inspector set facings'],
      ['v1', 'placement_0001', 4, undefined, undefined, 8, 'keyboard set facings'],
    ]);
  });

  it('routes shelf distribution intent without calculating coordinates in TypeScript', () => {
    const { command, engine } = engineWith('distribute_shelf');
    distributeShelf(engine, { versionId: 'v1', shelfId: 'shelf_01', distribution: 'space_evenly', expectedRevision: 7 }, 'inspector');
    expect(command).toHaveBeenCalledWith('v1', 'shelf_01', 'space_evenly', 7, 'inspector space evenly distribution');
  });
});
