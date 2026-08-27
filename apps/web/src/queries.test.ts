import { describe, expect, it } from 'vitest';
import { getSection, searchProducts, toToolPlacement, toToolProduct } from './queries';
import type { EngineContext, Placement, Product } from './types';

const trayProduct: Product = {
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
    source: 'Synthetic representative 13-week average; not retailer actuals',
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

const looseProduct: Product = {
  ...trayProduct,
  id: 'jif_creamy_40',
  upc: '051500720004',
  size_oz: '40 oz',
  tray: null,
};

function placement(overrides: Partial<Placement> = {}): Placement {
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

function contextWithPlacements(placements: Placement[], shelfWidth = 768): EngineContext {
  return {
    version_id: 'version_draft_01',
    version_status: 'draft',
    revision: 0,
    fixture: {
      id: 'fixture_standard_4ft',
      name: "4' Standard Bay",
      width: shelfWidth,
      height: 1344,
      depth: 352,
      sections: [{
        id: 'section_01',
        fixture_id: 'fixture_standard_4ft',
        sequence: 0,
        width: shelfWidth,
        height: 1344,
        shelves: [{ id: 'shelf_01', section_id: 'section_01', kind: 'adjustable', width: shelfWidth, depth: 256, elevation: 192 }],
      }],
    },
    products: [trayProduct, looseProduct],
    placements,
  };
}

function availableCapacity(placements: Placement[], shelfWidth = 768): number {
  const result = getSection(contextWithPlacements(placements, shelfWidth), 'section_01');
  if (!result) throw new Error('Expected section_01');
  return result.section.shelves[0].available_capacity_sixteenths;
}

describe('catalog query transport', () => {
  it('keeps exact metrics and explicitly names sixteenth-inch tray dimensions', () => {
    expect(toToolProduct(trayProduct)).toMatchObject({
      net_weight_ounces_hundredths: 1600,
      casepack_quantity: 12,
      performance: {
        sales_per_store_per_week_cents: 3665,
        units_per_store_per_week_milliunits: 10500,
        gross_margin_basis_points: 2850,
        source: 'Synthetic representative 13-week average; not retailer actuals',
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
  it('transports the resolved footprint and uses it for contiguous shelf capacity', () => {
    const trayPlacement = placement();
    const loosePlacement = placement({
      id: 'placement_0002',
      product_id: looseProduct.id,
      x: 200,
      stocking_mode: 'loose',
      facings_x: 1,
      facings_z: 1,
      stocked_unit_count: 1,
      geometry: { display_width: 57, display_height: 130, required_depth: 45 },
      tray_front_lip_height: null,
    });
    expect(toToolPlacement(trayPlacement)).toMatchObject({
      stocking_mode: 'tray',
      stocked_unit_count: 12,
      display_width_sixteenths: 175,
      display_height_sixteenths: 80,
      required_depth_sixteenths: 232,
      tray_front_lip_height_sixteenths: 20,
    });

    const context = contextWithPlacements([trayPlacement, loosePlacement]);
    expect(getSection(context, 'section_01')).toMatchObject({
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
    expect(availableCapacity([placement()])).toBe(590);
  });

  it('reports the exact placeable width between two Rust-derived footprints', () => {
    const rightPlacement = placement({
      id: 'placement_0002',
      product_id: looseProduct.id,
      x: 200,
      stocking_mode: 'loose',
      facings_x: 1,
      facings_z: 1,
      stocked_unit_count: 1,
      geometry: { display_width: 57, display_height: 130, required_depth: 45 },
      tray_front_lip_height: null,
    });

    expect(availableCapacity([placement(), rightPlacement], 257)).toBe(20);
  });
});
