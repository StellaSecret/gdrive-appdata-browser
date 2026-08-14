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

  test('preset quick-sign-in buttons (if configured) are accessible and well-formed', async ({
    page,
  }) => {
    // Deliberately does NOT assume a fixed number of presets: whether any
    // of the GOOGLE_WEB_CLIENT_ID_* secrets are set depends on which
    // environment built this artifact (local/fork builds have none; a
    // real deploy may have all three), and that can change over time as
    // secrets get added. Instead this asserts the invariant that holds
    // either way - if presets exist, each one must be a real, accessible
    // button, not just decoration.
    await page.goto('./');
    const presetButtons = page.locator('.preset-avatar');
    const count = await presetButtons.count();

    if (count === 0) {
      // No presets configured in this build - the manual Client ID field
      // (covered by the other tests above) is the only path, and the
      // "Quick sign-in" group shouldn't render at all in that case.
      await expect(page.locator('.presets')).toHaveCount(0);
      return;
    }

    await expect(page.getByText('Quick sign-in:')).toBeVisible();
    for (let i = 0; i < count; i++) {
      const ariaLabel = await presetButtons.nth(i).getAttribute('aria-label');
      expect(ariaLabel).toMatch(/^Sign in with .+'s Client ID$/);
    }
  });
});
