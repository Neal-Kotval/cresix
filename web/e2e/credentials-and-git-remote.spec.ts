import { expect, test } from "@playwright/test";
import { mockC6 } from "./fixtures";

test.beforeEach(async ({ page }) => { await mockC6(page); });

test("creates a scoped Git credential, reveals it once, and revokes metadata", async ({ page, context }) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.goto("/credentials");
  await expect(page.getByRole("heading", { name: "CLI & Git credentials" })).toBeVisible();
  await expect(page.getByText("Laptop CLI")).toBeVisible();

  await page.getByRole("radio", { name: /Git HTTPS/ }).check();
  await expect(page.locator(".scope-unavailable")).toContainText("git:write is unavailable");
  await page.getByLabel("Label").fill("Laptop Git");
  await page.getByRole("button", { name: "Create credential" }).click();

  const dialog = page.getByRole("dialog", { name: "Copy Laptop Git now" });
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: "Copy credential" }).click();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe("c6g_v1_public_once-only-secret");
  await dialog.getByRole("button", { name: "I have stored it" }).click();
  await expect(dialog).toBeHidden();
  await expect(page.locator("body")).not.toContainText("c6g_v1_public_once-only-secret");

  await page.getByRole("button", { name: "Revoke Laptop Git" }).click();
  await page.getByRole("button", { name: "Confirm revoke" }).click();
  await expect(page.getByText("Laptop Git")).toBeVisible();
  await expect(page.getByText("Revoked", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Revoke Laptop Git" })).toBeHidden();
});

test("presents the canonical clone URL without embedding a credential", async ({ page }) => {
  await page.goto("/projects/weeknote");
  await expect(page.getByText("https://c6.example/git/paper-street/weeknote.git")).toBeVisible();
  await expect(page.getByText("Fetch available")).toBeVisible();
  await expect(page.getByText("Push unavailable")).toBeVisible();
  await expect(page.locator("body")).not.toContainText("c6g_v1_");
  await expect(page.locator("body")).not.toHaveCSS("overflow-x", "scroll");
});
