import { test, expect } from '@playwright/test';

// These are smoke tests only. They verify the WASM app boots and the
// pre-sign-in UI behaves correctly. They deliberately do NOT exercise real
// Google sign-in - that requires a live Google account and consent flow,
// which isn't something we can (or should) automate in public CI.

test.describe('appDataFolder Browser - smoke tests', () => {
  test('loads the page and shows the sign-in form', async ({ page }) => {
    await page.goto('./');
    await expect(page.locator('h1')).toHaveText('appDataFolder Browser');
    await expect(page.locator('#client-id')).toBeVisible();
    await expect(
      page.getByRole('button', { name: /sign in with google/i })
    ).toBeVisible();
  });

  test('shows a validation message when signing in without a Client ID', async ({
    page,
  }) => {
    await page.goto('./');
    await page.getByRole('button', { name: /sign in with google/i }).click();
    await expect(
      page.getByText('Enter your Google OAuth Client ID first.')
    ).toBeVisible();
  });

  test('accepts typed input into the Client ID field', async ({ page }) => {
    await page.goto('./');
    const input = page.locator('#client-id');
    await input.fill('123456789-abc.apps.googleusercontent.com');
    await expect(input).toHaveValue(
      '123456789-abc.apps.googleusercontent.com'
    );
  });

  test('remembers the Client ID across a page reload (localStorage)', async ({
    page,
  }) => {
    await page.goto('./');
    const input = page.locator('#client-id');
    await input.fill('999999999-persisted.apps.googleusercontent.com');
    // Trigger the sign-in click so the value gets saved to localStorage,
    // then reload - it should still show the validation error (no real
    // client exists) but the field should be pre-filled from storage.
    await page.getByRole('button', { name: /sign in with google/i }).click();
    await page.reload();
    await expect(page.locator('#client-id')).toHaveValue(
      '999999999-persisted.apps.googleusercontent.com'
    );
  });
});
