import { defineConfig, devices } from '@playwright/test';

/**
 * Email against a full local stack whose Google is `crates/gmail_emulator`
 * and whose Microsoft is `crates/outlook_emulator`. Run through
 * `just local-e2e-email`, which starts the stack without Doppler (so the
 * emulators stand in for Google and Microsoft) and sets
 * `LOCAL_EMAIL_E2E_APP_URL` (the stack's own frontend), `GMAIL_EMULATOR_URL`
 * and `OUTLOOK_EMULATOR_URL` (each emulator's sign-in page and control API).
 * Specs sign in with Google or Microsoft as one of an emulator's seeded
 * users.
 */
const appURL = process.env.LOCAL_EMAIL_E2E_APP_URL;
if (
  !appURL ||
  !process.env.GMAIL_EMULATOR_URL ||
  !process.env.OUTLOOK_EMULATOR_URL
) {
  throw new Error(
    'LOCAL_EMAIL_E2E_APP_URL, GMAIL_EMULATOR_URL and OUTLOOK_EMULATOR_URL are required. Run `just local-e2e-email`.'
  );
}

export default defineConfig({
  testDir: './email-live',
  testMatch: '**/*.live.ts',
  fullyParallel: false,
  workers: 1,
  retries: 0,
  timeout: 120_000,
  expect: { timeout: 30_000 },
  reporter: [['list']],
  use: {
    ...devices['Desktop Chrome'],
    baseURL: `${appURL.replace(/\/$/, '')}/`,
    ignoreHTTPSErrors: true,
    launchOptions: {
      executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH,
    },
    trace: 'retain-on-failure',
  },
});
