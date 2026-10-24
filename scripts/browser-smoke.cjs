// Run only against a server using disposable prototype data.
const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const { randomUUID } = require('node:crypto');

const baseURL = process.env.COMIC_TEST_URL || 'http://127.0.0.1:8080';
const output = 'artifacts';
fs.mkdirSync(output, { recursive: true });

(async () => {
  const browser = await chromium.launch({
    executablePath: process.env.COMIC_CHROMIUM === 'bundled' ? undefined : (process.env.COMIC_CHROMIUM || '/usr/bin/chromium'),
    headless: true,
    args: ['--no-sandbox'],
  });
  const errors = [];
  const context = await browser.newContext({
    viewport: { width: 1440, height: 1000 },
    httpCredentials: { username: 'comic', password: process.env.COMIC_TEST_PASSWORD || '', send: 'always' },
    extraHTTPHeaders: { Origin: new URL(baseURL).origin },
  });
  const page = await context.newPage();
  page.on('pageerror', error => errors.push(error.message));
  // Dedicated project checks verify cancellation. Here explicit reloads recover
  // disposable work after upload/save failures, including ambiguous outcomes.
  page.on('dialog', dialog => dialog.type() === 'beforeunload' ? dialog.accept() : dialog.dismiss());
  const read = async () => {
    const response = await context.request.get(`${baseURL}/api/workspace`);
    assert(response.ok());
    return response.json();
  };
  const saved = async () => {
    await page.waitForFunction(() => document.querySelector('[role=status]')?.textContent.trim() === 'Saved');
  };
  const waitForData = async predicate => {
    for (let attempt = 0; attempt < 100; attempt++) {
      const snapshot = await read();
      if (predicate(snapshot)) return snapshot;
      await page.waitForTimeout(100);
    }
    throw new Error('Timed out waiting for saved workspace data');
  };
  try {
    await page.goto(baseURL);
    await page.getByRole('heading', { name: 'The story takes shape.' }).waitFor();
    await saved();
    await page.screenshot({ path: `${output}/desktop.png`, fullPage: true });

    await page.getByRole('button', { name: 'Story overview', exact: false }).click();
    await page.getByRole('heading', { name: 'Your story, unfolding.' }).waitFor();
    assert.equal(await page.locator('.story-card').count(), 4);
    assert(await page.locator('.archive-art').evaluate(async image => { await image.decode(); return image.naturalWidth > 0; }), 'Bundled overview artwork failed to load');
    await page.screenshot({ path: `${output}/overview-desktop.png`, fullPage: true });
    await page.locator('.story-card').first().click();
    const editor = page.getByRole('complementary', { name: 'Panel editor' });
    assert.equal(await editor.getByLabel('Page position').inputValue(), '0');
    await editor.getByLabel('Title', { exact: true }).fill('Wick listens');
    await editor.getByLabel('Action & composition').fill('Wick pauses beneath the trees. A branch cracks nearby.');
    await waitForData(s => s.workspace.panels[0].title === 'Wick listens');
    await saved();
    await page.reload();
    await saved();
    assert.equal(await editor.getByLabel('Title', { exact: true }).inputValue(), 'Wick listens');

    const png = Buffer.from(await page.evaluate(() => {
      const canvas = document.createElement('canvas');
      canvas.width = 360; canvas.height = 240;
      const ctx = canvas.getContext('2d');
      ctx.fillStyle = '#526952'; ctx.fillRect(0, 0, 360, 240);
      ctx.fillStyle = '#cad6b5'; ctx.font = '24px serif'; ctx.fillText('Sample panel', 35, 125);
      return canvas.toDataURL('image/png').split(',')[1];
    }), 'base64');
    // A failed server upload must retain HTTP evidence and one retry UUID,
    // without reflecting an arbitrary provider/proxy response into the UI.
    const beforeFailedUpload = await read();
    const failedUploadIds = [];
    await page.route('**/api/projects/*/panels/*/images', route => {
      failedUploadIds.push(route.request().headers()['x-comic-upload-id']);
      return route.fulfill({ status: 503, body: 'provider-secret-sentinel' });
    });
    await editor.locator('input[type=file]').setInputFiles({ name: 'unavailable.png', mimeType: 'image/png', buffer: png });
    await page.getByRole('alert').waitFor();
    const uploadFailure = await page.getByRole('alert').innerText();
    assert.match(uploadFailure, /HTTP 503/);
    assert.equal(failedUploadIds.length, 3);
    assert(failedUploadIds[0]);
    assert(failedUploadIds.every(id => id === failedUploadIds[0]));
    assert(uploadFailure.includes(failedUploadIds[0]));
    assert(!uploadFailure.includes('provider-secret-sentinel'));
    assert.deepEqual(await read(), beforeFailedUpload);
    await page.unroute('**/api/projects/*/panels/*/images');
    await page.reload();
    await saved();

    for (let id = 1; id <= 4; id++) {
      await page.locator('.panel-card').nth(id - 1).click();
      await editor.locator('input[type=file]').setInputFiles({ name: `panel-${id}.png`, mimeType: 'image/png', buffer: png });
      await waitForData(s => s.workspace.panels[id - 1].images.length === 1);
      await saved();
    }
    const beforeReplacement = await read();
    await page.locator('.panel-card').nth(2).click();
    await editor.locator('input[type=file]').setInputFiles({ name: 'replacement.png', mimeType: 'image/png', buffer: png });
    const afterReplacement = await waitForData(s => s.workspace.panels[2].images.length === 2);
    for (const index of [0, 1, 3]) {
      assert.deepEqual(afterReplacement.workspace.panels[index], beforeReplacement.workspace.panels[index]);
    }
    await saved();
    assert.equal(await editor.getByLabel('Selected image').inputValue(), afterReplacement.workspace.panels[2].selected_image);
    const firstImage = afterReplacement.workspace.panels[2].images[0].id;
    await editor.getByRole('button', { name: 'Select image alternative 1', exact: true }).click();
    assert.equal(await editor.getByLabel('Selected image').inputValue(), firstImage);
    await waitForData(s => s.workspace.panels[2].selected_image === firstImage);
    await saved();
    await editor.getByLabel('Horizontal crop').evaluate(input => {
      input.value = '25';
      input.dispatchEvent(new Event('input', { bubbles: true }));
    });
    await waitForData(s => s.workspace.panels[2].crop_x === 25);
    await saved();

    await page.getByRole('combobox', { name: 'Page layout' }).selectOption('wide');
    await waitForData(s => s.workspace.chapters[0].pages[0].layout === 'Wide');
    assert.equal(await page.locator('.page-panel').count(), 3);
    assert.match(await page.locator('.panel-card').nth(3).innerText(), /Unplaced/);
    assert.equal((await read()).workspace.panels.length, 4);
    await page.getByRole('combobox', { name: 'Page layout' }).selectOption('grid');
    await waitForData(s => s.workspace.chapters[0].pages[0].layout === 'Grid');
    await page.locator('.panel-card').nth(3).click();
    await editor.getByLabel('Page position').selectOption('3');
    await waitForData(s => s.workspace.chapters[0].pages[0].slots[3] === 4);
    await saved();
    await page.getByRole('combobox', { name: 'Page layout' }).selectOption('full');
    await waitForData(s => s.workspace.chapters[0].pages[0].layout === 'Full');
    assert.equal(await page.locator('.page-panel').count(), 1);
    assert.equal((await read()).workspace.panels.length, 4);
    await page.getByRole('combobox', { name: 'Page layout' }).selectOption('grid');
    await waitForData(s => s.workspace.chapters[0].pages[0].layout === 'Grid');
    for (let id = 2; id <= 4; id++) {
      await page.locator('.panel-card').nth(id - 1).click();
      await editor.getByLabel('Page position').selectOption(String(id - 1));
      await waitForData(s => s.workspace.chapters[0].pages[0].slots[id - 1] === id);
    }
    await saved();

    const unchanged = await read();
    const invalid = await context.request.post(`${baseURL}/api/panels/1/images`, {
      headers: { 'x-comic-upload-id': randomUUID(), 'x-comic-revision': String(unchanged.revision), 'content-type': 'application/octet-stream' },
      data: Buffer.from('invalid image bytes'),
    });
    assert.equal(invalid.status(), 400);
    assert.deepEqual(await read(), unchanged);
    const oversized = await context.request.post(`${baseURL}/api/panels/1/images`, {
      headers: { 'x-comic-upload-id': randomUUID(), 'x-comic-revision': String(unchanged.revision), 'content-type': 'application/octet-stream' },
      data: Buffer.alloc(20_000_001),
    });
    assert.equal(oversized.status(), 400);
    assert.deepEqual(await read(), unchanged);
    for (const mime of ['image/jpeg', 'image/webp']) {
      const bytes = Buffer.from(await page.evaluate(mime => {
        const canvas = document.createElement('canvas');
        canvas.width = 16; canvas.height = 16;
        canvas.getContext('2d').fillRect(0, 0, 16, 16);
        return canvas.toDataURL(mime).split(',')[1];
      }, mime), 'base64');
      const current = await read();
      const upload = await context.request.post(`${baseURL}/api/panels/4/images`, {
        headers: { 'x-comic-upload-id': randomUUID(), 'x-comic-revision': String(current.revision), 'content-type': mime },
        data: bytes,
      });
      assert(upload.ok(), `${mime} upload failed: ${await upload.text()}`);
      const result = await upload.json();
      const image = result.workspace.panels[3].images.at(-1);
      const stored = await context.request.get(`${baseURL}${image.url}`);
      assert.deepEqual(await stored.body(), bytes, 'Stored original image changed');
    }

    await page.setViewportSize({ width: 390, height: 844 });
    await page.reload();
    await saved();
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'Mobile page has horizontal overflow');
    await editor.getByLabel('Dialogue').fill('Who goes there?');
    await waitForData(s => s.workspace.panels[0].dialogue === 'Who goes there?');
    await saved();
    await page.screenshot({ path: `${output}/phone.png`, fullPage: true });
    await page.getByRole('button', { name: 'Story overview', exact: false }).click();
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'Mobile overview overflows');
    assert.match(await page.locator('.story-card').first().innerText(), /Wick listens/);
    await page.screenshot({ path: `${output}/overview-phone.png`, fullPage: true });
    await page.getByRole('navigation', { name: 'Workspace views' }).getByRole('button', { name: 'Page composition', exact: false }).click();
    assert.equal(await editor.getByLabel('Dialogue').inputValue(), 'Who goes there?');

    await page.route('**/api/projects/*', route => route.abort());
    await editor.getByLabel('Dialogue').fill('Keep this edit when saving fails.');
    await page.getByRole('alert').waitFor();
    assert.equal(await editor.getByLabel('Dialogue').inputValue(), 'Keep this edit when saving fails.');
    await page.unroute('**/api/projects/*');
    await page.getByRole('button', { name: 'Retry', exact: true }).click();
    await waitForData(s => s.workspace.panels[0].dialogue === 'Keep this edit when saving fails.');
    await saved();

    const second = await context.newPage();
    await second.goto(baseURL);
    await second.getByRole('heading', { name: 'The story takes shape.' }).waitFor();
    await second.waitForFunction(() => document.querySelector('[role=status]')?.textContent.trim() === 'Saved');
    await editor.getByLabel('Title', { exact: true }).fill('Latest phone edit');
    await waitForData(s => s.workspace.panels[0].title === 'Latest phone edit');
    await saved();
    await second.getByRole('complementary', { name: 'Panel editor' }).getByLabel('Title', { exact: true }).fill('Stale desktop edit');
    await second.getByRole('alert').waitFor();
    assert.equal((await read()).workspace.panels[0].title, 'Latest phone edit');
    assert.equal(await second.getByRole('complementary', { name: 'Panel editor' }).getByLabel('Title', { exact: true }).inputValue(), 'Stale desktop edit');
    await second.close();

    assert.deepEqual(errors, [], 'Browser runtime errors');
    console.log('PASS: autosave/reload, PNG/JPEG/WebP import, size/format rejection, independent image replacement, alternatives, crop, layouts, mobile overflow, save failure/retry, upload failure diagnostics, and stale-write conflict');
  } finally {
    await browser.close();
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
