// Disposable PostgreSQL/S3 only. Covers v3 panel/page and reference interactions.
const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const { randomUUID, createHash } = require('node:crypto');
const baseURL = process.env.COMIC_TEST_URL || 'http://127.0.0.1:8080';
let png;
(async () => {
  const browser = await chromium.launch({ executablePath: process.env.COMIC_CHROMIUM === 'bundled' ? undefined : (process.env.COMIC_CHROMIUM || '/usr/bin/chromium'), headless: true, args: ['--no-sandbox'] });
  const context = await browser.newContext({ viewport: { width: 1440, height: 1000 }, httpCredentials: { username: 'comic', password: process.env.COMIC_TEST_PASSWORD || '', send: 'always' }, extraHTTPHeaders: { Origin: new URL(baseURL).origin } });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', e => errors.push(e.message));
  const button = name => page.getByRole('button', { name, exact: true });
  const saved = () => page.waitForFunction(() => document.querySelector('[role=status]')?.textContent.trim() === 'Saved');
  const read = async id => { const r = await context.request.get(`${baseURL}/api/projects/${id}`); assert(r.ok()); return r.json(); };
  const waitData = async (id, predicate) => {
    for (let i = 0; i < 100; i++) { const s = await read(id); if (predicate(s.workspace)) { await saved(); return s; } await page.waitForTimeout(100); }
    throw Error('Timed out waiting for panel/page/reference persistence');
  };
  const view = name => page.getByRole('navigation', { name: 'Workspace views' }).getByRole('button', { name, exact: true }).click();
  const selectPanel = title => page.getByRole('navigation', { name: 'Chapter panels' }).getByRole('button').filter({ hasText: title }).click();
  const confirm = () => page.getByRole('dialog', { name: 'Panel and page confirmation' }).getByRole('button', { name: 'Confirm', exact: true }).click();
  const cancel = () => page.getByRole('dialog', { name: 'Panel and page confirmation' }).getByRole('button', { name: 'Cancel', exact: true }).click();
  const createProject = async name => {
    await button('New project').click(); await page.getByLabel('New project name', { exact: true }).fill(name); await button('Create project').click();
    await page.getByLabel('Project title', { exact: true }).waitFor(); await saved(); return new URL(page.url()).searchParams.get('project');
  };
  try {
    if (process.argv.includes('--verify-retention')) {
      const before = JSON.parse(fs.readFileSync('artifacts/panels-references-retention.json'));
      assert.deepEqual(await read(before.snapshot.project_id), before.snapshot);
      for (const url of [before.referenceURL, ...before.originalImages.map(i => i.url)]) {
        const response = await context.request.get(`${baseURL}${url}`);
        assert(response.ok(), `Retained original ${url}`);
        assert.equal(createHash('sha256').update(await response.body()).digest('hex'), before.referenceDigest);
      }
      console.log('PASS: fresh runtime retains v3 pages/planning/assets and removed reference/panel original bytes');
      return;
    }
    await page.goto(`${baseURL}/?view=projects`);
    const id = await createProject(`Panels references ${randomUUID().slice(0, 8)}`);
    png = Buffer.from(await page.evaluate(() => { const canvas = document.createElement('canvas'); canvas.width = 16; canvas.height = 16; const ctx = canvas.getContext('2d'); ctx.fillStyle = '#a45b32'; ctx.fillRect(0, 0, 16, 16); return canvas.toDataURL('image/png').split(',')[1]; }), 'base64');
    let snapshot = await read(id);
    const source = snapshot.workspace.chapters[0].id;
    const panelID = snapshot.workspace.panels[0].id;
    await page.getByLabel('Title', { exact: true }).fill('Bridge approach');
    await page.getByLabel('Framing & composition', { exact: true }).fill('Wide shot from the bridge');
    await page.getByLabel('Mood', { exact: true }).fill('Uneasy');
    await page.getByLabel('Production notes', { exact: true }).fill('Retain original alternatives');
    await waitData(id, w => w.panels[0].production_notes === 'Retain original alternatives');
    const editor = page.getByRole('complementary', { name: 'Panel editor' });
    for (let i = 0; i < 2; i++) {
      await editor.locator('input[type=file]').setInputFiles({ name: `original-${i}.png`, mimeType: 'image/png', buffer: png });
      await waitData(id, w => w.panels[0].images.length === i + 1);
    }
    const originalPanel = (await read(id)).workspace.panels[0];
    await view('Asset library');
    await page.getByLabel('Asset type', { exact: true }).selectOption('Character');
    await button('Create character').click();
    await page.getByLabel('Asset name', { exact: true }).fill('Bridge keeper');
    await waitData(id, w => w.assets[0]?.name === 'Bridge keeper');
    await page.getByLabel('Import reference image', { exact: true }).setInputFiles({ name: 'keeper.png', mimeType: 'image/png', buffer: png });
    snapshot = await waitData(id, w => w.assets[0]?.reference_images.length === 1);
    const asset = snapshot.workspace.assets[0];
    await page.getByLabel('Reference label', { exact: true }).fill('Keeper silhouette');
    await page.getByLabel('Reference description', { exact: true }).fill('The same shared reference in every panel');
    await waitData(id, w => w.assets[0].reference_images[0].description === 'The same shared reference in every panel');
    await button('Open reference Keeper silhouette').click();
    const lightbox = page.getByRole('dialog', { name: 'Reference image: Keeper silhouette' });
    assert(await lightbox.isVisible());
    await lightbox.getByRole('img').evaluate(image => image.decode());
    await button('Close reference image').click();
    const referenceURL = asset.reference_images[0].url;
    const response = await context.request.get(`${baseURL}${referenceURL}`);
    assert(response.ok()); assert.deepEqual(await response.body(), png);
    const referenceDigest = createHash('sha256').update(png).digest('hex');
    await view('Page composition');
    await page.getByRole('checkbox', { name: 'Bridge keeper', exact: true }).check();
    await waitData(id, w => w.panels.find(p => p.id === panelID).assets.characters.includes(asset.id));
    const shared = page.locator('details').filter({ has: page.getByText('Bridge keeper — shared definition', { exact: true }) });
    await shared.locator('summary').click();
    await shared.getByRole('button', { name: 'Open reference Keeper silhouette', exact: true }).click();
    assert(await lightbox.isVisible()); await button('Close reference image').click();
    await button('New chapter').click();
    await page.getByLabel('Chapter title', { exact: true }).fill('Destination');
    snapshot = await waitData(id, w => w.chapters.some(c => c.title === 'Destination'));
    const destination = snapshot.workspace.chapters.find(c => c.title === 'Destination').id;
    await button('Open chapter Chapter 1').click();
    await selectPanel('Bridge approach');
    await page.getByLabel('Move panel to chapter', { exact: true }).selectOption(String(destination));
    await waitData(id, w => w.chapters.find(c => c.id === destination).panel_ids.includes(panelID));
    await button('Open chapter Destination').click(); await selectPanel('Bridge approach');
    await button('New page').click(); await waitData(id, w => w.chapters.find(c => c.id === destination).pages.length === 2);
    // Select by visible text: page IDs are stable while displayed numbers change.
    await page.getByLabel('Panel placement', { exact: true }).selectOption({ label: 'Page 2 · Position 1' });
    snapshot = await waitData(id, w => w.chapters.find(c => c.id === destination).pages[1].slots[0] === panelID);
    const secondPage = snapshot.workspace.chapters.find(c => c.id === destination).pages[1].id;
    await page.getByRole('region', { name: 'Page management' }).getByRole('listitem').nth(1).getByRole('button', { name: 'Earlier', exact: true }).click();
    await waitData(id, w => w.chapters.find(c => c.id === destination).pages[0].id === secondPage);
    await page.reload(); await saved(); await button('Open chapter Destination').click(); await selectPanel('Bridge approach');
    assert.equal(await page.getByLabel('Mood', { exact: true }).inputValue(), 'Uneasy');
    snapshot = await read(id);
    const retainedPanel = snapshot.workspace.panels.find(p => p.id === panelID);
    assert.deepEqual(retainedPanel.images, originalPanel.images);
    assert.equal(retainedPanel.selected_image, originalPanel.selected_image);
    await button('Delete panel').click(); await cancel();
    assert.deepEqual((await read(id)).workspace, snapshot.workspace);
    await button('Remove page 1').click(); await cancel();
    assert.deepEqual((await read(id)).workspace, snapshot.workspace);
    await button('Remove page 1').click(); await confirm();
    await waitData(id, w => w.chapters.find(c => c.id === destination).pages.length === 1);
    await button('Remove page 1').click(); await confirm();
    await waitData(id, w => w.chapters.find(c => c.id === destination).pages.length === 0);
    assert(await page.getByText('No pages. Panels remain editable and unplaced. Create a page to compose them.', { exact: true }).isVisible());
    await button('Delete panel').click(); await confirm();
    snapshot = await waitData(id, w => !w.panels.some(p => p.id === panelID));
    assert.equal(snapshot.workspace.assets[0].reference_images.length, 1);
    await view('Asset library'); await button('Bridge keeper').click();
    await button('Remove reference').click(); await button('Cancel reference removal').click();
    assert.equal((await read(id)).workspace.assets[0].reference_images.length, 1);
    await button('Remove reference').click(); await button('Confirm remove reference').click();
    await waitData(id, w => w.assets[0].reference_images.length === 0);
    await page.setViewportSize({ width: 390, height: 844 });
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'Phone reference gallery overflows');
    fs.mkdirSync('artifacts', { recursive: true }); await page.screenshot({ path: 'artifacts/panels-references-phone.png', fullPage: true });
    await view('Page composition');
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'Phone panel/page controls overflow');
    await button('New page').click(); await waitData(id, w => w.chapters.find(c => c.id === destination).pages.length === 1);
    const final = await read(id);
    fs.writeFileSync('artifacts/panels-references-retention.json', JSON.stringify({ snapshot: final, referenceURL, referenceDigest, originalImages: originalPanel.images, source }, null, 2));
    assert.deepEqual(errors, [], 'Browser runtime errors');
    console.log('PASS: planning fields, alternative images, shared references/larger views, panel move/delete/cancel, page creation/reorder/removal/cancel/zero pages, reload and phone controls');
  } catch (error) {
    fs.mkdirSync('artifacts', { recursive: true }); console.error('Panel/reference runtime errors:', errors);
    console.error(await page.locator('body').innerText().catch(() => 'Unavailable'));
    await page.screenshot({ path: 'artifacts/panels-references-failure.png', fullPage: true }).catch(() => {}); throw error;
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
