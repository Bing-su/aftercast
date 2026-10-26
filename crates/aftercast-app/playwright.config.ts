import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests/desktop',
  workers: 1,
  timeout: 90_000,
  expect: { timeout: 15_000 },
  reporter: 'list',
});
