import { defineConfig } from '@playwright/test';

// In CI, the pipeline builds the app once with its production base_path
// (e.g. /repo-name/) already baked in, then nests the build under that
// same path locally so these smoke tests exercise the exact artifact that
// gets deployed - no separate/duplicate build for testing vs. deploying.
//
// Locally (no env vars set), this falls back to serving ../docs at the
// root, matching a plain `dx bundle --platform web --release` with no
// base_path configured.
const baseURL = process.env.PW_BASE_URL || 'http://127.0.0.1:4173/';
const serveDir = process.env.PW_SERVE_DIR || '../docs';

export default defineConfig({
  testDir: './tests',
  timeout: 30_000,
  fullyParallel: true,
  reporter: [['html', { open: 'never' }], ['list']],
  use: {
    baseURL,
    trace: 'retain-on-failure',
  },
  webServer: {
    command: `python3 -m http.server 4173 --directory ${serveDir}`,
    url: baseURL,
    reuseExistingServer: !process.env.CI,
    timeout: 30_000,
  },
});
