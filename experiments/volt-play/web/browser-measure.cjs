// Sayfa yükleme ölçümü: önce `node web/serve.mjs web/dist 8765`, sonra
//   OUT=results PW=$(npm root -g)/playwright node web/browser-measure.cjs
// CHROMIUM: isteğe bağlı tarayıcı yolu (yoksa Playwright'ın kendi tarayıcısı).
const { chromium } = require(process.env.PW || 'playwright');
(async () => {
  const opts = process.env.CHROMIUM ? { executablePath: process.env.CHROMIUM } : {};
  const browser = await chromium.launch(opts);
  const profiles = [
    ['throttle yok (yerel)', null],
    ['kablo/fiber 20 Mbps, 40 ms RTT', { latency: 40, downloadThroughput: 20e6 / 8, uploadThroughput: 5e6 / 8 }],
    ['4G 9 Mbps, 85 ms RTT', { latency: 85, downloadThroughput: 9e6 / 8, uploadThroughput: 1.5e6 / 8 }],
    ['yavaş 4G 1.6 Mbps, 150 ms RTT', { latency: 150, downloadThroughput: 1.6e6 / 8, uploadThroughput: 750e3 / 8 }],
  ];
  for (const [name, cond] of profiles) {
    const ctx = await browser.newContext();
    const page = await ctx.newPage();
    const errors = [];
    page.on('pageerror', (e) => errors.push(String(e)));
    page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
    const cdp = await ctx.newCDPSession(page);
    await cdp.send('Network.enable');
    await cdp.send('Network.setCacheDisabled', { cacheDisabled: true });
    if (cond) await cdp.send('Network.emulateNetworkConditions', { offline: false, ...cond });
    let wasmBytes = 0;
    cdp.on('Network.loadingFinished', (e) => { wasmBytes += e.encodedDataLength; });
    const t0 = Date.now();
    await page.goto('http://127.0.0.1:8765/', { waitUntil: 'load' });
    await page.waitForFunction(() => window.voltPlayReady === true, null, { timeout: 120000 });
    const ready = Date.now() - t0;
    const st = await page.evaluate(() => ({ load: window.voltPlayLoadMs, ms: window.voltPlayLast.ms, status: document.getElementById('status').textContent }));
    console.log(`${name.padEnd(32)} sayfa+wasm hazır ${String(ready).padStart(5)} ms, aktarılan ${(wasmBytes / 1024).toFixed(0)} KiB, init() ${st.load.toFixed(0)} ms, ilk derleme ${st.ms.toFixed(1)} ms ${errors.length ? 'HATA: ' + errors.join(' | ') : ''}`);
    if (!cond) {
      await page.setViewportSize({ width: 1280, height: 760 });
      await page.screenshot({ path: process.env.OUT + '/blinky.png' });
      await page.selectOption('#example', 'cdc_error');
      await page.waitForFunction(() => window.voltPlayLast.out.summary.errors === 1);
      await page.selectOption('#lang', 'tr');
      await page.waitForTimeout(200);
      await page.click('#diag-list details summary');
      await page.screenshot({ path: process.env.OUT + '/cdc_error_tr.png' });
      // Düzenleme: hatayı düzelt (sync köprüsü) ve yeniden derlenmeyi bekle.
      const fixed = await page.evaluate(() => {
        const ta = document.getElementById('src');
        ta.value = ta.value.replace('reg led_r : bool = false', 'let pressed_sync = sync(pressed_r, slow_clk)\n\n    reg led_r : bool = false').replace('led_r <= pressed_r', 'led_r <= pressed_sync');
        ta.dispatchEvent(new Event('input'));
        return true;
      });
      await page.waitForTimeout(600);
      const after = await page.evaluate(() => window.voltPlayLast.out.summary);
      console.log('  düzenleme sonrası (sync eklendi):', JSON.stringify(after), fixed);
      await page.screenshot({ path: process.env.OUT + '/cdc_fixed.png' });
    }
    await ctx.close();
  }
  await browser.close();
})();
