import { describe, expect, it } from 'vitest';
import { getPlanogramContext, getSection, searchProducts, toToolOperation, toToolPlacement, toToolProduct } from './queries';
import { PERFORMANCE_SOURCE, adjustableShelf, loosePlacement, looseProduct, makeContext, reflowOperation, trayPlacement, trayProduct } from './testFixtures';
import type { EngineContext, Placement } from './types';

function contextWithPlacements(placements: Placement[], shelfWidth = 768): EngineContext {
  return makeContext({ width: shelfWidth, shelves: [adjustableShelf('shelf_01', 192, shelfWidth)], products: [trayProduct, looseProduct], placements });
}

function availableCapacity(placements: Placement[], shelfWidth = 768): number {
  const result = getSection(contextWithPlacements(placements, shelfWidth), 'section_01');
  if (!result) throw new Error('Expected section_01');
  return result.section.shelves[0].available_capacity_sixteenths;
}

const rightLoosePlacement = loosePlacement({ x: 200 });

describe('catalog query transport', () => {
  it('passes Rust days of supply through for SKUs placed in the section only', () => {
    const supply = (product_id: string, days: number | null) => ({ product_id, stocked_units: 12, weekly_demand_milliunits: 10_500, days_supply_millidays: days, band: 'under_fourteen_days' as const });
    const context = { ...contextWithPlacements([trayPlacement()]), sku_supply: [supply(trayProduct.id, 8_000), supply('elsewhere_sku', 2_000)] };

    expect(getSection(context, 'section_01')?.section.sku_supply).toEqual([supply(trayProduct.id, 8_000)]);
  });

  it('distinguishes the audit-log tail from the next undoable change', () => {
    const context = {
      ...contextWithPlacements([]),
      revision: 3,
      latest_change_set_id: 'change_0003',
      latest_undoable_change_set_id: 'change_0001',
    };

    expect(getPlanogramContext(context, undefined).summary).toMatchObject({
      latest_change_set_id: 'change_0003',
      latest_undoable_change_set_id: 'change_0001',
    });
  });

  it('keeps exact metrics and explicitly names sixteenth-inch tray dimensions', () => {
    expect(toToolProduct(trayProduct)).toMatchObject({
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
    });
    expect(toToolProduct(looseProduct).tray).toBeNull();
  });

  it('filters tray and loose-stocked products without changing their catalog order', () => {
    const products = [trayProduct, looseProduct];
    expect(searchProducts(products, { stocking_mode: 'tray' }).map(product => product.id)).toEqual(['jif_creamy_16']);
    expect(searchProducts(products, { stocking_mode: 'loose' }).map(product => product.id)).toEqual(['jif_creamy_40']);
    expect(searchProducts(products, { query: 'shelf-ready' }).map(product => product.id)).toEqual(['jif_creamy_16']);
  });
});

describe('Rust-derived placement geometry in queries', () => {
  it('transports a Rust-resolved reflow with explicit before and after facings', () => {
    expect(toToolOperation(reflowOperation(loosePlacement({ id: 'placement_0001' }), { x: 4, facings_x: 4 }))).toEqual({
      type: 'reflow_placement',
      placement_id: 'placement_0001',
      before: { shelf_id: 'shelf_01', x_sixteenths: 0, facings_x: 1, facings_y: 1, facings_z: 1 },
      after: { shelf_id: 'shelf_01', x_sixteenths: 4, facings_x: 4, facings_y: 1, facings_z: 1 },
    });
  });

  it('transports a Rust facing change with explicit before and after counts', () => {
    const counts = (facings_x: number) => ({ facings_x, facings_y: 1, facings_z: 1 });
    expect(toToolOperation({ type: 'change_facings', placement_id: 'placement_0001', before: counts(1), after: counts(3) })).toEqual({
      type: 'change_facings',
      placement_id: 'placement_0001',
      before: counts(1),
      after: counts(3),
    });
    expect(toToolOperation({ type: 'change_facings', placement_id: 'placement_0001', before: { facings_x: 1 }, after: counts(3) })).toEqual({ type: 'unknown' });
  });

  it('transports the resolved footprint and uses it for contiguous shelf capacity', () => {
    expect(toToolPlacement(trayPlacement())).toMatchObject({
      stocking_mode: 'tray',
      stocked_unit_count: 12,
      display_width_sixteenths: 175,
      display_height_sixteenths: 80,
      required_depth_sixteenths: 232,
      tray_front_lip_height_sixteenths: 20,
    });

    expect(getSection(contextWithPlacements([trayPlacement(), rightLoosePlacement]), 'section_01')).toMatchObject({
      section: {
        shelves: [{
          available_capacity_sixteenths: 508,
          placements: [
            { id: 'placement_0001', display_width_sixteenths: 175 },
            { id: 'placement_0002', display_width_sixteenths: 57 },
          ],
        }],
      },
    });
  });

  it('reports the exact right-edge capacity after the gap and even x-grid alignment', () => {
    expect(availableCapacity([trayPlacement()])).toBe(590);
  });

  it('reports the exact placeable width between two Rust-derived footprints', () => {
    expect(availableCapacity([trayPlacement(), rightLoosePlacement], 257)).toBe(20);
  });
});
