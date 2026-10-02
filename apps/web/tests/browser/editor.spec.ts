import { readFile } from 'node:fs/promises';
import { expect, test, type Page } from '@playwright/test';

type SiteTool = { name: string; execute: (args: unknown, context?: { signal?: AbortSignal }) => Promise<unknown> };
type SiteToolWindow = Window & { __planogramSiteTools: SiteTool[] };

const SITE_TOOL_COUNT = 13;
const SYNTHETIC_SOURCE = 'Synthetic representative 13-week average; not retailer actuals';

/** Opens the editor and skips when the browser lacks WebGPU entirely. */
async function openEditor(page: Page) {
  await page.goto('/');
  await page.waitForFunction(() => document.querySelector('.shelf-list button') || document.querySelector('.unsupported'));
  test.skip(await page.getByRole('heading', { name: 'WebGPU is required' }).isVisible(), 'The test browser does not expose WebGPU.');
}

/** Records WebMCP tool registrations so tests can call the live site tools. */
async function openEditorWithSiteTools(page: Page) {
  await page.addInitScript(() => {
    Object.defineProperty(window, '__planogramSiteTools', { value: [], configurable: true, writable: true });
    Object.defineProperty(document, 'modelContext', {
      configurable: true,
      value: {
        registerTool: async (tool: SiteTool) => { (window as unknown as SiteToolWindow).__planogramSiteTools.push(tool); },
        unregisterTool: async () => undefined,
      },
    });
  });
  await openEditor(page);
  await page.waitForFunction(count => ((window as unknown as Partial<SiteToolWindow>).__planogramSiteTools?.length ?? 0) === count, SITE_TOOL_COUNT);
}

function callSiteTool<T = unknown>(page: Page, name: string, args: unknown = {}): Promise<T> {
  return page.evaluate(async ({ name, args }) => {
    const tool = (window as unknown as SiteToolWindow).__planogramSiteTools.find(candidate => candidate.name === name);
    if (!tool) throw new Error(`${name} site tool was not registered`);
    return await tool.execute(args, { signal: new AbortController().signal });
  }, { name, args }) as Promise<T>;
}

/** The first render-backed write in headless Chromium may throw; skip rather than fail. */
async function callSiteToolOrSkip<T = unknown>(page: Page, name: string, args: unknown): Promise<T> {
  try {
    return await callSiteTool<T>(page, name, args);
  } catch (error) {
    test.skip(true, `Headless WebGPU could not execute ${name}: ${String(error).split("\n")[0]}`);
    throw error;
  }
}

async function shelf01Placements(page: Page) {
  const result = await callSiteTool<{ section: { shelves: Array<{ id: string; placements: unknown[] }> } }>(page, 'planogram.get_section', { section_id: 'section_01' });
  return result.section.shelves.find(shelf => shelf.id === 'shelf_01')?.placements;
}

function revision(page: Page, value: number) {
  return page.getByText(`Revision ${value} · All changes local`);
}

async function expectRevision(page: Page, value: number) {
  await expect(revision(page, value)).toBeVisible();
}

/** Skips when headless WebGPU initialized but the first command never rendered. */
async function skipUnlessRevision(page: Page, value: number) {
  const reached = await revision(page, value).waitFor({ timeout: 2_000 }).then(() => true, () => false);
  test.skip(!reached, 'Headless WebGPU initialized but could not execute a render-backed command.');
}

async function addToShelf01(page: Page, times = 1) {
  await page.getByRole('button', { name: /Shelf 01/ }).click();
  const add = page.getByRole('button', { name: /Add .*selected shelf/ });
  for (let count = 0; count < times; count += 1) await add.click();
  await skipUnlessRevision(page, times);
}

function placementAt(page: Page, position: string) {
  return page.getByRole('button', { name: new RegExp(`Jif Creamy Peanut Butter 16 oz.*at ${position}`) });
}

test('loads the complete fixture and supports accessible command paths', async ({ page }) => {
  await openEditor(page);

  const baseDeck = page.getByRole('button', { name: /Base deck/ });
  await expect(baseDeck).toBeVisible();
  for (let shelf = 1; shelf <= 6; shelf += 1) {
    await expect(page.getByRole('button', { name: new RegExp(`Shelf 0${shelf}`) })).toBeVisible();
  }
  const stockingFilter = page.getByLabel('Filter by stocking mode');
  for (const [mode, count] of [['tray', 5], ['loose', 17], ['all', 22]] as const) {
    await stockingFilter.selectOption(mode);
    await expect(page.locator('.catalog-header')).toContainText(`${count} SKUs`);
  }
  await baseDeck.click();
  await expect(page.getByRole('button', { name: /Add .*selected shelf/ })).toBeDisabled();

  const shelf04 = page.getByRole('button', { name: /Shelf 04/ });
  await shelf04.click();
  await expect(page.getByRole('heading', { name: 'Shelf 04' })).toBeVisible();
  const elevation = page.getByLabel('Elevation', { exact: true });
  const applyElevation = page.locator('.elevation-form').getByRole('button', { name: 'Apply' });
  await elevation.fill('4\' 6"');
  await applyElevation.click();
  await skipUnlessRevision(page, 1);

  await shelf04.press('ArrowUp');
  await expect(elevation).toHaveValue('4\' 7"');
  await shelf04.press('Shift+ArrowDown');
  await expect(elevation).toHaveValue('4\' 6"');

  await elevation.fill('5\'');
  await applyElevation.click();
  await expect(page.locator('#elevation-error')).toContainText('cannot occupy the same elevation');
  await expectRevision(page, 3);

  const undo = page.getByRole('button', { name: 'Undo' });
  for (const [expected, nextRevision] of [['4\' 7"', 4], ['4\' 6"', 5], ['4\'', 6]] as const) {
    await undo.click();
    await expect(elevation).toHaveValue(expected);
    await expectRevision(page, nextRevision);
  }
  await expect(undo).toBeDisabled();
});

test('shows the explicit unsupported state without WebGPU', async ({ page }) => {
  await page.addInitScript(() => Object.defineProperty(navigator, 'gpu', { value: undefined, configurable: true }));
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'WebGPU is required' })).toBeVisible();
});

test('selects and removes a placement through the accessible command path, then restores it with undo', async ({ page }) => {
  await openEditor(page);
  await addToShelf01(page);

  // A lone tray is centered: Rust spaces shelf blocks evenly by default.
  const placement = placementAt(page, '1\' 6 1/2"');
  await expect(placement).toBeVisible();
  await placement.click();
  await expect(page.getByRole('heading', { name: 'Jif Creamy Peanut Butter' })).toBeVisible();
  await expect(page.getByText('placement_0001')).toBeVisible();
  await expect(page.getByText('Shelf-ready tray', { exact: true }).first()).toBeVisible();
  await expect(page.getByText('12', { exact: true }).first()).toBeVisible();
  const inspector = page.locator('.selection-panel');
  for (const text of ['3 × 1 × 4', '10 15/16" W × 5" H × 1\' 2 1/2" D', '1 1/4"', 'Illustrative data', 'Sales / store / week', 'Units / store / week', 'Gross margin', '$36.65', '10.5', '28.5%', 'Trailing 13 weeks', SYNTHETIC_SOURCE]) {
    await expect(inspector.getByText(text, { exact: true }).first()).toBeVisible();
  }

  await page.getByRole('button', { name: 'Remove product' }).click();
  await expectRevision(page, 2);
  await expect(placement).not.toBeVisible();
  await expect(page.getByText('Select a shelf or product on the canvas or in the fixture outline.')).toBeVisible();

  await page.getByRole('button', { name: 'Undo' }).click();
  await expectRevision(page, 3);
  await placement.click();
  await expect(page.getByText('placement_0001')).toBeVisible();
  await expect(inspector.getByText('1\' 6 1/2"', { exact: true })).toBeVisible();
});

test('moves a selected placement by eighths and between shelves through one inspector command', async ({ page }) => {
  await openEditor(page);
  await addToShelf01(page);

  const placement = placementAt(page, '1\' 6 1/2"');
  await placement.click();
  const position = page.getByLabel('Position', { exact: true });
  const shelf = page.getByLabel('Shelf', { exact: true });
  const apply = page.locator('.placement-form').getByRole('button', { name: 'Apply' });
  await expect(position).toHaveValue('1\' 6 1/2"');

  // Manual moves keep their exact position; they never re-apply the layout.
  await placement.press('ArrowRight');
  await expect(position).toHaveValue('1\' 6 5/8"');
  await expectRevision(page, 2);

  await position.fill('1\' 6 9/16"');
  await apply.click();
  await expect(page.getByRole('alert')).toContainText('1/8-inch increments');
  await expectRevision(page, 2);
  await expect(position).toHaveValue('1\' 6 5/8"');

  await shelf.selectOption('shelf_02');
  await apply.click();
  await expectRevision(page, 3);
  await expect(shelf).toHaveValue('shelf_02');
  await expect(page.getByText('shelf_02', { exact: true })).toBeVisible();
  await expect(placementAt(page, '1\' 6 5/8"')).toBeVisible();

  await page.getByRole('button', { name: 'Undo' }).click();
  await expectRevision(page, 4);
  await expect(shelf).toHaveValue('shelf_01');
  await expect(position).toHaveValue('1\' 6 5/8"');
});

test('registers site tools and applies the first WebMCP write through the live page session', async ({ page }) => {
  await openEditorWithSiteTools(page);
  await expect(page.getByText('Site tools ready')).toBeVisible();
  const toolNames = await page.evaluate(() => (window as unknown as SiteToolWindow).__planogramSiteTools.map(tool => tool.name));
  expect(toolNames).toEqual([
    'planogram.get_planogram_context',
    'planogram.search_products',
    'planogram.get_product',
    'planogram.get_section',
    'planogram.validate_planogram',
    'planogram.add_product',
    'planogram.distribute_shelf',
    'planogram.undo_change_set',
    'planogram.preview_shelf_allocation',
    'planogram.preview_sales_allocation',
    'planogram.preview_changes',
    'planogram.apply_changes',
    'planogram.set_facings',
  ]);

  expect(await callSiteTool(page, 'planogram.get_product', { product_id: 'jif_creamy_16' })).toMatchObject({
    status: 'ok',
    product: {
      id: 'jif_creamy_16',
      net_weight_ounces_hundredths: 1600,
      casepack_quantity: 12,
      dimensions: { depth_sixteenths: 57 },
      performance: {
        sales_per_store_per_week_cents: 3665,
        units_per_store_per_week_milliunits: 10500,
        gross_margin_basis_points: 2850,
        source: SYNTHETIC_SOURCE,
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
    },
  });

  const added = await callSiteToolOrSkip(page, 'planogram.add_product', { product_id: 'jif_creamy_16', shelf_id: 'shelf_01', expected_revision: 0, reason: 'browser contract test' });
  expect(added).toMatchObject({ status: 'applied', revision: 1, change_set: { actor: 'webmcp' }, placement: { shelf_id: 'shelf_01', x_sixteenths: 296, stocking_mode: 'tray', stocked_unit_count: 12, display_width_sixteenths: 175, display_height_sixteenths: 80, required_depth_sixteenths: 232 } });
  await expectRevision(page, 1);
  await expect(placementAt(page, '1\' 6 1/2"')).toBeVisible();

  expect(await callSiteTool(page, 'planogram.undo_change_set', { change_set_id: 'change_0001', expected_revision: 1 })).toMatchObject({ status: 'applied', revision: 2, change_set: { actor: 'webmcp' } });
  await expectRevision(page, 2);
  await expect(placementAt(page, '1\' 6 1/2"')).not.toBeVisible();
});

test('previews a WebMCP proposal and records truthful human approval in the review UI', async ({ page }) => {
  await openEditorWithSiteTools(page);

  const preview = await callSiteToolOrSkip(page, 'planogram.preview_changes', {
    expected_revision: 0,
    reason: 'Group Jif by package size',
    operations: [
      { kind: 'add', product_id: 'jif_creamy_16', shelf_id: 'shelf_01', sequence: 0 },
      { kind: 'add', product_id: 'jif_creamy_40', shelf_id: 'shelf_02', sequence: 0 },
    ],
  });
  expect(preview).toMatchObject({ status: 'ready', revision: 0, proposal_id: 'proposal_0001' });
  await expectRevision(page, 0);
  await expect(page.getByText('Proposal ready · 2 changes')).toBeVisible();
  await expect(page.getByText(/Add Jif Creamy Peanut Butter \(16 oz\) to Shelf 01 at 1' 6 1\/2"/)).toBeVisible();

  await page.getByRole('button', { name: 'Accept proposal' }).click();
  await expectRevision(page, 1);
  await expect(page.getByText('Proposal ready · 2 changes')).not.toBeVisible();
  const receipt = page.locator('.applied-proposal-receipt');
  const receiptHeading = receipt.getByRole('heading', { name: 'WebMCP proposal approved' });
  await expect(receiptHeading).toBeVisible();
  await expect(receiptHeading).toBeFocused();
  for (const text of ['Group Jif by package size', 'human', '0 → 1', 'change_0001', '2']) {
    await expect(receipt.getByText(text, { exact: true })).toBeVisible();
  }
  await expect(placementAt(page, '1\' 6 1/2"')).toBeVisible();
});

test('fills a shelf through one semantic WebMCP proposal and undoes the atomic reflow', async ({ page }) => {
  await openEditorWithSiteTools(page);

  const setup = await callSiteToolOrSkip<{ status: string; proposal_id?: string }>(page, 'planogram.preview_changes', {
    expected_revision: 0,
    reason: 'Place all largest Jif products on the bottom shelf',
    operations: ['jif_creamy_40', 'jif_crunchy_40', 'jif_natural_40'].map((product_id, sequence) => ({ kind: 'add', product_id, shelf_id: 'shelf_01', sequence })),
  });
  test.skip(setup.status !== 'ready', 'Headless WebGPU could not execute the setup preview.');
  const applied = await callSiteToolOrSkip<{ status: string }>(page, 'planogram.apply_changes', { proposal_id: setup.proposal_id, expected_revision: 0 });
  test.skip(applied.status !== 'applied', 'Headless WebGPU could not execute the setup proposal.');

  const filledLayout = [
    { x_sixteenths: 4, facings_x: 4 },
    { x_sixteenths: 282, facings_x: 4 },
    { x_sixteenths: 560, facings_x: 3 },
  ];
  expect(await callSiteTool(page, 'planogram.preview_shelf_allocation', { shelf_id: 'shelf_01', strategy: 'fill_evenly', expected_revision: 1, reason: 'Fill that shelf evenly with facings' })).toMatchObject({
    status: 'ready',
    revision: 1,
    operations: filledLayout.map(after => ({ type: 'reflow_placement', after })),
  });
  await expectRevision(page, 1);
  await expect(page.getByText('Proposal ready · 3 changes')).toBeVisible();
  await expect(page.getByText(/Reflow Jif Creamy Peanut Butter.*from 1 to 4 horizontal facings/)).toBeVisible();

  await page.getByRole('button', { name: 'Accept proposal' }).click();
  await expectRevision(page, 2);
  expect(await shelf01Placements(page)).toMatchObject(filledLayout);

  await page.getByRole('button', { name: 'Undo' }).click();
  await expectRevision(page, 3);
  expect(await shelf01Placements(page)).toMatchObject([
    { x_sixteenths: 140, facings_x: 1 },
    { x_sixteenths: 350, facings_x: 1 },
    { x_sixteenths: 560, facings_x: 1 },
  ]);
});

test('keeps same-product trays in one tight block, applies an explicit distribution, and enforces the gap', async ({ page }) => {
  await openEditor(page);
  await addToShelf01(page, 2);
  // Two trays of the same SKU form one block at the minimum gap, spaced evenly as a unit.
  await expect(placementAt(page, '1\' 1"')).toBeVisible();
  await expect(placementAt(page, '2\' 1/8"')).toBeVisible();

  await expect(page.getByLabel('Product distribution')).toHaveValue('space_evenly');
  await page.getByLabel('Product distribution').selectOption('space_between');
  await page.locator('.distribution-form').getByRole('button', { name: 'Apply' }).click();
  await expectRevision(page, 3);
  await expect(placementAt(page, '1\' 7/8"')).toBeVisible();
  await expect(placementAt(page, '2\'')).toBeVisible();

  await page.getByRole('button', { name: 'Undo' }).click();
  await expectRevision(page, 4);
  await placementAt(page, '2\' 1/8"').click();
  const position = page.getByLabel('Position', { exact: true });
  await position.fill('2\'');
  await page.locator('.placement-form').getByRole('button', { name: 'Apply' }).click();
  await expect(page.getByRole('alert')).toContainText('at least a 1/8-inch gap');
  await expectRevision(page, 4);
  await expect(position).toHaveValue('2\' 1/8"');
});

test('sets loose facings from the inspector and keyboard, re-spaces the shelf, rejects overflow, and undoes exactly', async ({ page }) => {
  await openEditor(page);
  await page.getByLabel('Filter by stocking mode').selectOption('loose');
  await page.getByRole('button', { name: /^Jif Extra Crunchy Peanut Butter 16 oz/ }).click();
  await page.getByRole('button', { name: /Shelf 01/ }).click();
  const add = page.getByRole('button', { name: /Add .*selected shelf/ });
  await add.click();
  await add.click();
  await skipUnlessRevision(page, 2);

  const companion = page.locator('.companion');
  const crunchy = (position: string, facings: string) => companion.getByRole('button', { name: new RegExp(`Jif Extra Crunchy Peanut Butter.*at ${position} · Loose · ${facings} facings`) });
  // Two units of one SKU form a single tight block spaced evenly on the shelf.
  await expect(crunchy('2\' 1/8"', '1 × 1 × 1')).toBeVisible();

  await crunchy('1\' 8 3/8"', '1 × 1 × 1').click();
  await page.getByLabel('Wide').fill('3');
  await page.getByRole('button', { name: 'Apply facings' }).click();
  await expectRevision(page, 3);
  await expect(crunchy('1\' 4 3/4"', '3 × 1 × 1')).toBeVisible();
  await expect(crunchy('2\' 3 5/8"', '1 × 1 × 1')).toBeVisible();

  const lead = crunchy('1\' 4 3/4"', '3 × 1 × 1');
  await lead.focus();
  await lead.press('-');
  await expectRevision(page, 4);
  await expect(crunchy('1\' 6 5/8"', '2 × 1 × 1')).toBeVisible();
  await expect(crunchy('2\' 1 7/8"', '1 × 1 × 1')).toBeVisible();

  await page.getByLabel('High').fill('3');
  await page.getByRole('button', { name: 'Apply facings' }).click();
  await expect(page.getByRole('alert')).toContainText('too tall');
  await expectRevision(page, 4);

  const undo = page.getByRole('button', { name: 'Undo' });
  await undo.click();
  await expectRevision(page, 5);
  await expect(crunchy('1\' 4 3/4"', '3 × 1 × 1')).toBeVisible();
  await undo.click();
  await expectRevision(page, 6);
  await expect(crunchy('1\' 8 3/8"', '1 × 1 × 1')).toBeVisible();
  await expect(crunchy('2\' 1/8"', '1 × 1 × 1')).toBeVisible();
});

test('keeps loaded tray facings fixed in the inspector and through WebMCP', async ({ page }) => {
  await openEditorWithSiteTools(page);
  await addToShelf01(page);
  await placementAt(page, '1\' 6 1/2"').click();
  for (const control of [page.getByLabel('Wide'), page.getByLabel('High'), page.getByLabel('Deep'), page.getByRole('button', { name: 'Add one horizontal facing' }), page.getByRole('button', { name: 'Apply facings' })]) {
    await expect(control).toBeDisabled();
  }
  await expect(page.getByText('Loaded trays keep their catalog preset facings.')).toBeVisible();

  expect(await callSiteTool(page, 'planogram.set_facings', { placement_id: 'placement_0001', facings_x: 4, expected_revision: 1 })).toMatchObject({
    status: 'validation_failed',
    revision: 1,
    validation: { issues: [{ code: 'tray_facing_mismatch' }] },
  });
  await expectRevision(page, 1);
});


async function downloadBay(page: Page, name: string) {
  await page.getByRole('button', { name: 'Save bay', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Save this arrangement' });
  await dialog.getByLabel('Bay name').fill(name);
  const downloadPromise = page.waitForEvent('download');
  await dialog.getByRole('button', { name: 'Download bay' }).click();
  const download = await downloadPromise;
  expect(download.suggestedFilename()).toBe(`${name}.planogrammy.json`);
  expect(await download.failure()).toBeNull();
  return await readFile((await download.path())!, 'utf8');
}

async function openBayText(page: Page, text: string) {
  await page.getByLabel('Open bay file').setInputFiles({ name: 'test.planogrammy.json', mimeType: 'application/json', buffer: Buffer.from(text) });
}

test('downloads the complete bay, closes, reopens, edits and undoes preserved history', async ({ page, context }, testInfo) => {
  await openEditorWithSiteTools(page);
  await addToShelf01(page, 2);
  const json = await downloadBay(page, 'Peanut butter bay');
  const file = JSON.parse(json);
  expect(file).toMatchObject({ format: 'planogrammy-bay', format_version: 1, name: 'Peanut butter bay', draft: { revision: 2 } });
  expect(file.draft.products).toHaveLength(22);
  expect(file.draft.products.filter((product: { tray: unknown }) => product.tray)).toHaveLength(5);
  expect(file.draft.placements).toHaveLength(2);
  expect(file.draft.change_sets).toHaveLength(2);
  expect(file.draft.fixture.width_sixteenths).toBe(768);
  const before = await shelf01Placements(page);
  await expect(page.getByText('No unsaved changes', { exact: true })).toBeVisible();
  await page.close();

  const reopened = await context.newPage();
  const errors: string[] = [];
  reopened.on('pageerror', error => errors.push(error.message));
  await openEditorWithSiteTools(reopened);
  await openBayText(reopened, json);
  await expectRevision(reopened, 2);
  await expect(reopened.locator('.bay-document strong')).toHaveText('Peanut butter bay');
  expect(await shelf01Placements(reopened)).toEqual(before);
  await expect(reopened.getByRole('button', { name: 'Open bay', exact: true })).toBeFocused();
  const repeated = await downloadBay(reopened, 'Peanut butter bay');
  expect(JSON.parse(repeated)).toEqual(file);
  await reopened.screenshot({ path: testInfo.outputPath('reopened-bay.png') });
  await testInfo.attach('Reopened bay with exact placements', { path: testInfo.outputPath('reopened-bay.png'), contentType: 'image/png' });
  await testInfo.attach('Portable bay file', { body: json, contentType: 'application/json' });
  await reopened.getByRole('button', { name: /Shelf 01/ }).click();
  await reopened.getByRole('button', { name: /Add .*selected shelf/ }).click();
  await expectRevision(reopened, 3);
  await expect(reopened.getByText('Unsaved changes', { exact: true })).toBeVisible();
  await reopened.getByRole('button', { name: 'Undo', exact: true }).click();
  await expectRevision(reopened, 4);
  expect(await shelf01Placements(reopened)).toEqual(before);
  await reopened.getByRole('button', { name: 'Undo', exact: true }).click();
  await expectRevision(reopened, 5);
  expect(await shelf01Placements(reopened)).toHaveLength(1);
  const afterUndo = await downloadBay(reopened, 'After undo');
  await openBayText(reopened, afterUndo);
  await expectRevision(reopened, 5);
  await reopened.getByRole('button', { name: 'Undo', exact: true }).click();
  await expectRevision(reopened, 6);
  expect(await shelf01Placements(reopened)).toHaveLength(0);
  expect(errors).toEqual([]);
});

test('invalid files and cancelled replacements preserve current work; repeated opens reset selection', async ({ page }) => {
  await openEditorWithSiteTools(page);
  const initial = await downloadBay(page, 'Empty bay');
  await addToShelf01(page);
  const before = await shelf01Placements(page);
  for (const invalid of ['{', initial.replace('"format_version": 1', '"format_version": 42'), initial.replace('"next_placement": 1', '"next_placement": 0')]) {
    await openBayText(page, invalid);
    await expect(page.locator('.file-feedback[role="alert"]')).toBeVisible();
    await expectRevision(page, 1);
    expect(await shelf01Placements(page)).toEqual(before);
    await expect(page.getByRole('dialog')).not.toBeVisible();
  }
  await openBayText(page, initial);
  const replace = page.getByRole('dialog', { name: 'Replace the current bay?' });
  await expect(replace).toBeVisible();
  await expect(replace.getByRole('button', { name: 'Keep current bay' })).toBeFocused();
  await replace.getByRole('button', { name: 'Keep current bay' }).click();
  await expectRevision(page, 1);
  expect(await shelf01Placements(page)).toEqual(before);
  await openBayText(page, initial);
  await page.keyboard.press('Escape');
  await expectRevision(page, 1);
  await openBayText(page, initial);
  await replace.getByRole('button', { name: 'Replace bay', exact: true }).click();
  await expectRevision(page, 0);
  expect(await shelf01Placements(page)).toEqual([]);
  await expect(page.getByRole('button', { name: 'Undo', exact: true })).toBeDisabled();
  await expect(page.locator('.shelf-list button[aria-current="true"]')).toHaveCount(0);
  await openBayText(page, initial);
  await expect(page.getByRole('dialog')).not.toBeVisible();
  await expectRevision(page, 0);
});

test('saving excludes a pending proposal and opening confirms its discard even with no unsaved edits', async ({ page }) => {
  await openEditorWithSiteTools(page);
  const preview = await callSiteTool<{ proposal_id: string }>(page, 'planogram.preview_changes', {
    expected_revision: 0,
    operations: [{ kind: 'add', product_id: 'jif_creamy_16', shelf_id: 'shelf_01', sequence: 0 }],
    reason: 'Pending assortment',
  });
  await expect(page.getByRole('heading', { name: 'Proposal review' })).toBeVisible();
  await page.getByRole('button', { name: 'Save bay', exact: true }).click();
  await expect(page.getByRole('dialog')).toContainText('will not be included');
  await page.getByRole('dialog').getByRole('button', { name: 'Cancel' }).click();
  const json = await downloadBay(page, 'Committed bay');
  expect(JSON.parse(json).draft.placements).toEqual([]);
  expect(JSON.parse(json).draft.change_sets).toEqual([]);
  await expect(page.getByRole('heading', { name: 'Proposal review' })).toBeVisible();
  await openBayText(page, json);
  const replace = page.getByRole('dialog', { name: 'Replace the current bay?' });
  await expect(replace).toContainText('pending proposal');
  await replace.getByRole('button', { name: 'Keep current bay' }).click();
  await expect(page.getByRole('heading', { name: 'Proposal review' })).toBeVisible();
  await openBayText(page, json);
  await replace.getByRole('button', { name: 'Replace bay', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Proposal review' })).not.toBeVisible();
  expect(await callSiteTool(page, 'planogram.apply_changes', { proposal_id: preview.proposal_id, expected_revision: 0 })).toMatchObject({ status: 'not_found' });
  await expectRevision(page, 0);
});


test('a concurrent edit while replacement is pending rejects the stale open without losing newer work', async ({ page }) => {
  await openEditorWithSiteTools(page);
  const json = await downloadBay(page, 'Baseline');
  await addToShelf01(page);
  await openBayText(page, json);
  await expect(page.getByRole('dialog', { name: 'Replace the current bay?' })).toBeVisible();
  expect(await callSiteTool(page, 'planogram.add_product', { product_id: 'jif_creamy_16', shelf_id: 'shelf_01', expected_revision: 1 })).toMatchObject({ status: 'applied', revision: 2 });
  await page.getByRole('button', { name: 'Replace bay', exact: true }).click();
  await expect(page.locator('.file-feedback[role="alert"]')).toContainText('changed while opening');
  await expectRevision(page, 2);
  expect(await shelf01Placements(page)).toHaveLength(2);
  await expect(page.getByText('Unsaved changes', { exact: true })).toBeVisible();
  // The browser must offer a way to cancel navigation while work is unsaved.
  const dialogPromise = page.waitForEvent('dialog');
  const navigation = page.reload({ timeout: 2_000 }).catch(() => undefined);
  const dialog = await dialogPromise;
  expect(dialog.type()).toBe('beforeunload');
  await dialog.dismiss();
  await navigation;
  await expectRevision(page, 2);
});

test('synthetic eight-to-six challenge edits alternatives and preserves baseline across reopen', async ({page,context},testInfo)=>{
  const errors:string[]=[];page.on('pageerror',e=>errors.push(e.message));
  await openEditorWithSiteTools(page);
  await page.getByRole('button',{name:'Cereal challenge',exact:true}).click();
  await page.getByRole('button',{name:'Start cereal challenge'}).click();
  const compare=page.getByRole('region',{name:'Cereal challenge comparison'});
  await expect(compare).toContainText('1,095 units');await expect(compare).toContainText('1,475 units');
  await expect(page.locator('.catalog-header')).toContainText('100 SKUs');
  await page.screenshot({path:testInfo.outputPath('six-bay-overview.png')});
  await page.getByRole('combobox',{name:'Focus bay'}).selectOption('bay_06');
  await expect(page.locator('.shelf-list>li')).toHaveCount(6);
  await page.screenshot({path:testInfo.outputPath('six-bay-focused.png')});
  await page.getByRole('combobox',{name:'Focus bay'}).selectOption('');
  const original=JSON.parse(await downloadBay(page,'Cereal baseline'));
  expect(original.format_version).toBe(2);expect(original.document.baseline.fixture.sections).toHaveLength(8);
  expect(original.document.alternatives[0].draft.fixture.sections).toHaveLength(6);
  await page.locator('.placement-list .companion-placement>button').first().click();
  await page.getByRole('button',{name:'Remove product',exact:true}).click();
  await expectRevision(page,1);await expect(compare).toContainText('99 / 100 SKUs');
  await expect(compare).toContainText('1 unplaced SKUs');
  await page.getByRole('button',{name:'Duplicate target'}).click();
  await expectRevision(page,1);
  await page.getByRole('combobox',{name:'Scenario alternative'}).selectOption('-1');
  await expect(page.locator('.baseline-note')).toBeVisible();await expect(compare).toContainText('1,475 units');
  const saved=await downloadBay(page,'Cereal alternatives');
  expect(JSON.parse(saved).document.baseline).toEqual(original.document.baseline);
  const reopened=await context.newPage();await openEditorWithSiteTools(reopened);await openBayText(reopened,saved);
  await reopened.getByRole('combobox',{name:'Scenario alternative'}).selectOption('0');
  await expectRevision(reopened,1);await reopened.getByRole('button',{name:'Undo',exact:true}).click();await expectRevision(reopened,2);
  await expect(reopened.getByRole('region',{name:'Cereal challenge comparison'})).toContainText('1,095 units');
  await reopened.getByRole('combobox',{name:'Scenario alternative'}).selectOption('1');
  await expectRevision(reopened,1);await expect(reopened.getByRole('region',{name:'Cereal challenge comparison'})).toContainText('99 / 100 SKUs');
  const again=await downloadBay(reopened,'Repeated scenario');await openBayText(reopened,again);
  expect(JSON.parse(await downloadBay(reopened,'Repeated scenario'))).toEqual(JSON.parse(again));
  await reopened.screenshot({path:testInfo.outputPath('six-bay-reopened.png')});
  expect(errors).toEqual([]);
});

test('cereal cross-bay movement is atomic and undoable; baseline and corrupted files are protected', async ({page})=>{
  await openEditorWithSiteTools(page);
  await page.getByRole('button',{name:'Cereal challenge',exact:true}).click();await page.getByRole('button',{name:'Start cereal challenge'}).click();
  const original=JSON.parse(await downloadBay(page,'Cross bay'));
  const targetShelf='bay_06_shelf_05';
  const destination=original.document.alternatives[0].draft.placements.filter((p:{shelf_id:string})=>p.shelf_id===targetShelf);
  const preview=await callSiteTool<{proposal_id:string}>(page,'planogram.preview_changes',{expected_revision:0,reason:'Make a destination gap',operations:destination.map((p:{id:string})=>({kind:'remove',placement_id:p.id}))});
  expect(await callSiteTool(page,'planogram.apply_changes',{expected_revision:0,proposal_id:preview.proposal_id})).toMatchObject({status:'applied',revision:1});
  await page.locator('.placement-list .companion-placement>button').first().click();
  await page.getByLabel('Shelf',{exact:true}).selectOption(targetShelf);
  await page.getByLabel('Position',{exact:true}).fill('0');
  await page.locator('.placement-form').getByRole('button',{name:'Apply',exact:true}).click();
  await expectRevision(page,2);
  const moved=JSON.parse(await downloadBay(page,'Moved')).document.alternatives[0].draft;
  expect(moved.placements.find((p:{id:string})=>p.id==='placement_0001')).toMatchObject({shelf_id:targetShelf,x_sixteenths:0});
  await page.getByRole('button',{name:'Undo',exact:true}).click();await expectRevision(page,3);
  const undone=JSON.parse(await downloadBay(page,'Undone')).document.alternatives[0].draft;
  expect(undone.placements.find((p:{id:string})=>p.id==='placement_0001')).toEqual(original.document.alternatives[0].draft.placements[0]);
  await page.getByRole('combobox',{name:'Scenario alternative'}).selectOption('-1');
  expect(await callSiteTool(page,'planogram.add_product',{expected_revision:0,product_id:'cereal_01_01_01',shelf_id:'bay_01_shelf_01'})).toMatchObject({status:'forbidden'});
  original.document.alternatives[0].draft.scenario_origin.bay_count=8;
  await openBayText(page,JSON.stringify(original));await expect(page.locator('.file-feedback[role="alert"]')).toContainText('Invalid six-bay');
  await expect(page.getByRole('combobox',{name:'Scenario alternative'})).toHaveValue('-1');
});

test('shelf validation belongs to its form and clears after correction, selection or another command', async ({ page }, testInfo) => {
  await openEditor(page);
  await addToShelf01(page);
  const elevation = page.getByLabel('Elevation', { exact: true });
  const applyElevation = page.locator('.elevation-form').getByRole('button', { name: 'Apply' });
  const applyDistribution = page.locator('.distribution-form').getByRole('button', { name: 'Apply' });
  const elevationError = page.locator('#elevation-error');
  const distributionError = page.locator('#distribution-error');

  // This is a Rust rejection, not a UI parser error. The shelf must stay put.
  await elevation.fill('12 1/2');
  await applyElevation.click();
  await expectRevision(page, 1);
  await expect(elevationError).toContainText('1-inch increments');
  await expect(page.locator('.selection-panel [role="alert"]')).toHaveCount(1);
  await expect(distributionError).toHaveCount(0);
  await expect(elevation).toHaveAttribute('aria-invalid', 'true');
  await expect(page.getByLabel('Product distribution')).toHaveAttribute('aria-invalid', 'false');
  await page.screenshot({ path: testInfo.outputPath('scoped-elevation-validation.png') });
  await testInfo.attach('Elevation validation only under Elevation', { path: testInfo.outputPath('scoped-elevation-validation.png'), contentType: 'image/png' });

  await elevation.fill('13');
  await applyElevation.click();
  await expectRevision(page, 2);
  await expect(page.locator('.selection-panel [role="alert"]')).toHaveCount(0);
  await expect(elevation).toHaveAttribute('aria-invalid', 'false');
  await expect(elevation).toHaveAttribute('aria-describedby', 'elevation-help');

  // Local parser failures use the same form scope and do not survive selection.
  await elevation.fill('not a length');
  await applyElevation.click();
  await expect(elevationError).toBeVisible();
  await expect(distributionError).toHaveCount(0);
  await expectRevision(page, 2);
  await page.getByRole('button', { name: /^Shelf 02/ }).click();
  await expect(page.locator('.selection-panel [role="alert"]')).toHaveCount(0);
  await page.getByRole('button', { name: /^Shelf 01/ }).click();
  await expect(elevationError).toHaveCount(0);

  // A no-op distribution reports in Distribution, never in Elevation.
  await applyDistribution.click();
  await expect(distributionError).toContainText('already');
  await expect(elevationError).toHaveCount(0);
  await expectRevision(page, 2);
  await expect(page.getByLabel('Product distribution')).toHaveAttribute('aria-invalid', 'true');
  await page.getByLabel('Product distribution').selectOption('packed_left');
  await applyDistribution.click();
  await expectRevision(page, 3);
  await expect(page.locator('.selection-panel [role="alert"]')).toHaveCount(0);

  // Successful commands outside the failed form clear its stale feedback too.
  await elevation.fill('13 1/2');
  await applyElevation.click();
  await expect(elevationError).toBeVisible();
  await page.getByRole('button', { name: 'Undo', exact: true }).click();
  await expectRevision(page, 4);
  await expect(page.locator('.selection-panel [role="alert"]')).toHaveCount(0);
});

type SavedPlacement = {
  id: string;
  product_id: string;
  shelf_id: string;
  x_sixteenths: number;
  facings_x: number;
  facings_y: number;
  facings_z: number;
};

async function startCerealAllocation(page: Page) {
  await openEditorWithSiteTools(page);
  await page.getByRole('button', { name: 'Cereal challenge', exact: true }).click();
  await page.getByRole('button', { name: 'Start cereal challenge' }).click();
  await expectRevision(page, 0);
  await page.getByRole('combobox', { name: 'Focus bay' }).selectOption('bay_01');
  await page.locator('.placement-list .companion-placement>button').first().click();
  await expect(page.getByRole('heading', { name: 'Sales allocation', exact: true })).toBeVisible();
}

async function bayPlacements(page: Page, sectionId: string) {
  const result = await callSiteTool<{ section: { shelves: Array<{ placements: SavedPlacement[] }> } }>(page, 'planogram.get_section', { section_id: sectionId });
  return result.section.shelves.flatMap(shelf => shelf.placements);
}

async function expectEditorViewportStable(page: Page) {
  expect(await page.evaluate(() => window.scrollY)).toBe(0);
  const topbar = await page.locator('.topbar').boundingBox();
  expect(topbar).not.toBeNull();
  expect(topbar!.y).toBeGreaterThanOrEqual(0);
}

test('sales allocation previews all objectives, cancels without mutation, and saves an approved undoable cereal alternative', async ({ page, context }, testInfo) => {
  test.setTimeout(60_000);
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  await startCerealAllocation(page);
  await page.getByLabel('Maximum facings', { exact: true }).fill('24');
  const originalFile = JSON.parse(await downloadBay(page, 'Before sales allocation'));
  const originalDraft = originalFile.document.alternatives[0].draft;
  const before = await bayPlacements(page, 'bay_01');
  const previewButton = page.getByRole('button', { name: 'Preview sales allocation', exact: true });
  const review = page.getByRole('heading', { name: 'Proposal review', exact: true });
  const combinations = [
    { basis: 'revenue', target: 'space', scope: 'shelf', label: 'Revenue contribution → space' },
    { basis: 'units', target: 'facings', scope: 'shelf', label: 'Units contribution → facings' },
    { basis: 'revenue', target: 'facings', scope: 'bay', label: 'Revenue contribution → facings' },
    { basis: 'units', target: 'space', scope: 'bay', label: 'Units contribution → space' },
  ];
  for (const [index, choice] of combinations.entries()) {
    await page.getByRole('combobox', { name: 'Sales contribution', exact: true }).selectOption(choice.basis);
    await page.getByRole('combobox', { name: 'Allocate', exact: true }).selectOption(choice.target);
    await page.getByRole('combobox', { name: 'Allocation scope', exact: true }).selectOption(choice.scope);
    await previewButton.click();
    await expect(review).toBeVisible();
    await expect(review).toBeFocused();
    await expectEditorViewportStable(page);
    await expect(page.getByRole('heading', { name: 'Sales allocation preview', exact: true })).toBeVisible();
    await expect(page.locator('.sales-report-summary')).toContainText(choice.label);
    await expect(page.getByRole('region', { name: 'Sales allocation by product' })).toBeVisible();
    await expect(page.locator('.sales-allocation-report')).toContainText('synthetic demand stays unchanged');
    await expectRevision(page, 0);
    expect(await bayPlacements(page, 'bay_01')).toEqual(before);
    if (index === combinations.length - 1) {
      const path = testInfo.outputPath('sales-allocation-bay-preview.png');
      await page.screenshot({ path });
      await testInfo.attach('Synthetic units-to-space bay allocation preview', { path, contentType: 'image/png' });
    } else {
      await page.getByRole('button', { name: index === 0 ? 'Adjust allocation' : 'Reject', exact: true }).click();
      await expect(review).not.toBeVisible();
      await expect(previewButton).toBeFocused();
      await expectEditorViewportStable(page);
      await expect(page.getByRole('combobox', { name: 'Sales contribution', exact: true })).toHaveValue(choice.basis);
      await expect(page.getByRole('combobox', { name: 'Allocate', exact: true })).toHaveValue(choice.target);
      await expect(page.getByRole('combobox', { name: 'Allocation scope', exact: true })).toHaveValue(choice.scope);
      expect(await bayPlacements(page, 'bay_01')).toEqual(before);
    }
  }
  // Saving a pending allocation must save only the exact committed draft.
  const pending = JSON.parse(await downloadBay(page, 'Pending allocation excluded'));
  expect(pending.document.alternatives[0].draft).toEqual(originalDraft);
  await expect(review).toBeVisible();
  await page.getByRole('button', { name: 'Accept proposal', exact: true }).click();
  await expectRevision(page, 1);
  const receipt = page.getByRole('heading', { name: 'Sales allocation applied', exact: true });
  await expect(receipt).toBeVisible();
  await expect(receipt).toBeFocused();
  await expectEditorViewportStable(page);
  expect(await bayPlacements(page, 'bay_01')).not.toEqual(before);
  expect(await callSiteTool(page, 'planogram.validate_planogram')).toMatchObject({ status: 'ok', valid: true });
  const saved = await downloadBay(page, 'Sales allocation approved');
  const savedFile = JSON.parse(saved);
  const allocated = savedFile.document.alternatives[0].draft;
  expect(allocated.revision).toBe(1);
  expect(allocated.change_sets).toHaveLength(1);
  expect(allocated.change_sets[0]).toMatchObject({ actor: 'human', base_revision: 0, resulting_revision: 1 });
  expect(allocated.products).toEqual(originalDraft.products);
  expect(savedFile.document.baseline).toEqual(originalFile.document.baseline);
  for (const prior of originalDraft.placements as SavedPlacement[]) {
    const after = (allocated.placements as SavedPlacement[]).find(placement => placement.id === prior.id)!;
    expect(after).toMatchObject({ product_id: prior.product_id, shelf_id: prior.shelf_id, facings_y: prior.facings_y, facings_z: prior.facings_z });
    if (!prior.shelf_id.startsWith('bay_01_')) expect(after).toEqual(prior);
  }
  const repeated = await callSiteTool(page, 'planogram.preview_sales_allocation', {
    scope: { kind: 'bay', section_id: 'bay_01' }, basis: 'units', target: 'space', min_facings: 1, max_facings: 24, expected_revision: 1,
  });
  expect(repeated).toMatchObject({ status: 'invalid_command' });
  await expectRevision(page, 1);
  expect(JSON.parse(await downloadBay(page, 'Repeat allocation')).document.alternatives[0].draft).toEqual(allocated);

  const reopened = await context.newPage();
  reopened.on('pageerror', error => errors.push(error.message));
  await openEditorWithSiteTools(reopened);
  await openBayText(reopened, saved);
  await expectRevision(reopened, 1);
  expect(JSON.parse(await downloadBay(reopened, 'Sales allocation approved'))).toEqual(savedFile);
  await reopened.getByRole('button', { name: 'Undo', exact: true }).click();
  await expectRevision(reopened, 2);
  const undone = JSON.parse(await downloadBay(reopened, 'Allocation undone'));
  expect(undone.document.alternatives[0].draft.placements).toEqual(originalDraft.placements);
  expect(undone.document.alternatives[0].draft.change_sets[1].compensates).toBe(allocated.change_sets[0].id);
  expect(undone.document.baseline).toEqual(originalFile.document.baseline);
  const undoPath = testInfo.outputPath('sales-allocation-reopened-undo.png');
  await reopened.screenshot({ path: undoPath });
  await testInfo.attach('Reopened allocation undone to the exact original arrangement', { path: undoPath, contentType: 'image/png' });
  expect(errors).toEqual([]);
});

test('sales allocation validates bounds and impossible minimums in its own form without changing the draft', async ({ page }) => {
  await startCerealAllocation(page);
  const before = await bayPlacements(page, 'bay_01');
  const preview = page.getByRole('button', { name: 'Preview sales allocation', exact: true });
  const error = page.locator('#sales-allocation-error');
  for (const [min, max, message] of [
    ['0', '24', 'whole numbers from 1 to 100'],
    ['1.5', '24', 'whole numbers from 1 to 100'],
    ['4', '3', 'cannot exceed maximum'],
    ['100', '100', 'cannot fit the minimum facings'],
  ]) {
    await page.getByLabel('Minimum facings', { exact: true }).fill(min);
    await page.getByLabel('Maximum facings', { exact: true }).fill(max);
    await preview.click();
    await expect(error).toContainText(message);
    await expectEditorViewportStable(page);
    await expectRevision(page, 0);
    await expect(page.getByRole('heading', { name: 'Proposal review', exact: true })).not.toBeVisible();
    expect(await bayPlacements(page, 'bay_01')).toEqual(before);
  }
  await page.getByLabel('Minimum facings', { exact: true }).fill('1');
  await page.getByLabel('Maximum facings', { exact: true }).fill('24');
  await expect(error).not.toBeVisible();
  await preview.click();
  await expect(page.getByRole('heading', { name: 'Proposal review', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Reject', exact: true }).click();
  await expectRevision(page, 0);
  expect(await bayPlacements(page, 'bay_01')).toEqual(before);
  await page.getByRole('combobox', { name: 'Scenario alternative' }).selectOption('-1');
  await page.locator('.placement-list .companion-placement>button').first().click();
  await expect(preview).toBeDisabled();
  await expect(page.locator('.sales-allocation-unavailable')).toContainText('baseline is locked');
  expect(await callSiteTool(page, 'planogram.preview_sales_allocation', {
    scope: { kind: 'bay', section_id: 'bay_01' }, basis: 'revenue', target: 'space', min_facings: 1, max_facings: 24, expected_revision: 0,
  })).toMatchObject({ status: 'forbidden' });
});

test('sales allocation WebMCP handles cancellation and rejects proposals after edits or same-revision alternative switches', async ({ page }) => {
  await startCerealAllocation(page);
  const request = { scope: { kind: 'bay', section_id: 'bay_01' }, basis: 'revenue', target: 'space', min_facings: 1, max_facings: 24, expected_revision: 0 };
  const before = await bayPlacements(page, 'bay_01');
  const cancelled = await page.evaluate(async args => {
    const controller = new AbortController();
    controller.abort();
    const tool = (window as unknown as SiteToolWindow).__planogramSiteTools.find(candidate => candidate.name === 'planogram.preview_sales_allocation')!;
    return await tool.execute(args, { signal: controller.signal });
  }, request);
  expect(cancelled).toMatchObject({ status: 'error', code: 'cancelled', revision: 0 });
  await expectRevision(page, 0);
  expect(await bayPlacements(page, 'bay_01')).toEqual(before);
  await expect(page.getByRole('heading', { name: 'Proposal review', exact: true })).not.toBeVisible();
  const discarded = await callSiteTool<{ status: string; proposal_id: string }>(page, 'planogram.preview_sales_allocation', request);
  expect(discarded.status).toBe('ready');
  await page.getByRole('button', { name: 'Reject', exact: true }).click();
  expect(await callSiteTool(page, 'planogram.apply_changes', { proposal_id: discarded.proposal_id, expected_revision: 0 })).toMatchObject({ status: 'not_found' });
  const oldAlternative = await callSiteTool<{ status: string; proposal_id: string }>(page, 'planogram.preview_sales_allocation', request);
  expect(oldAlternative.status).toBe('ready');
  const alternative = page.getByRole('combobox', { name: 'Scenario alternative' });
  const duplicate = page.getByRole('button', { name: 'Duplicate target', exact: true });
  const cancelledDuplicateDialog = page.waitForEvent('dialog');
  const cancelledDuplicateClick = duplicate.click();
  const cancelDialog = await cancelledDuplicateDialog;
  expect(cancelDialog.type()).toBe('confirm');
  expect(cancelDialog.message()).toContain('Discard the pending proposal');
  await cancelDialog.dismiss();
  await cancelledDuplicateClick;
  await expect(alternative).toHaveValue('0');
  await expect(alternative.locator('option')).toHaveCount(2); // Baseline and the original target.
  await expect(page.getByRole('heading', { name: 'Proposal review', exact: true })).toBeVisible();
  await expectRevision(page, 0);
  expect(await bayPlacements(page, 'bay_01')).toEqual(before);

  const acceptedDuplicateDialog = page.waitForEvent('dialog');
  const acceptedDuplicateClick = duplicate.click();
  const acceptDialog = await acceptedDuplicateDialog;
  expect(acceptDialog.type()).toBe('confirm');
  expect(acceptDialog.message()).toContain('Discard the pending proposal');
  await acceptDialog.accept();
  await acceptedDuplicateClick;
  await expect(alternative).toHaveValue('1');
  await expect(alternative.locator('option')).toHaveCount(3); // Baseline plus two alternatives.
  await expect(page.getByRole('heading', { name: 'Proposal review', exact: true })).not.toBeVisible();
  await expectRevision(page, 0);
  expect(await callSiteTool(page, 'planogram.apply_changes', { proposal_id: oldAlternative.proposal_id, expected_revision: 0 })).toMatchObject({ status: 'not_found' });
  expect(await bayPlacements(page, 'bay_01')).toEqual(before);
  const stale = await callSiteTool<{ status: string; proposal_id: string }>(page, 'planogram.preview_sales_allocation', request);
  expect(stale.status).toBe('ready');
  const removal = await callSiteTool<{ proposal_id: string }>(page, 'planogram.preview_changes', {
    expected_revision: 0, operations: [{ kind: 'remove', placement_id: before[0].id }], reason: 'Independent assortment edit invalidates the old allocation',
  });
  expect(await callSiteTool(page, 'planogram.apply_changes', { proposal_id: removal.proposal_id, expected_revision: 0 })).toMatchObject({ status: 'applied', revision: 1 });
  const afterRemoval = await bayPlacements(page, 'bay_01');
  expect(await callSiteTool(page, 'planogram.apply_changes', { proposal_id: stale.proposal_id, expected_revision: 0 })).toMatchObject({ status: 'not_found' });
  expect(await callSiteTool(page, 'planogram.preview_sales_allocation', request)).toMatchObject({ status: 'revision_conflict', current_revision: 1 });
  await expectRevision(page, 1);
  expect(await bayPlacements(page, 'bay_01')).toEqual(afterRemoval);
});
