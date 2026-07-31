import { expect, test } from "@playwright/test";
import { mockC6 } from "./fixtures";

test.beforeEach(async ({ page }) => { await mockC6(page); });

test("deployment metadata states the hosting boundary", async ({ page }) => {
  await page.goto("/projects/weeknote/deployments");
  await expect(page.getByText("Metadata only in this release")).toBeVisible();
  await expect(page.getByRole("button", { name: "Hosting unavailable" })).toBeDisabled();
});

test("illustrative schedules stay inert while runs, logs, and secret metadata are available", async ({ page }) => {
  await page.goto("/projects/weeknote/jobs");
  const schedule = page.getByRole("button", { name: "Disable friday-notes" });
  await expect(schedule).toBeDisabled();
  await expect(page.getByText(/Illustrative job definitions/)).toBeVisible();
  await page.getByRole("link", { name: "Runs" }).click();
  await page.getByRole("button", { name: /friday-notes/ }).click();
  await expect(page.getByText("Illustrative trace")).toBeVisible();
  await expect(page.getByText(/No project code executed/)).toBeVisible();
  await page.getByRole("link", { name: "Secrets" }).click();
  await expect(page.getByText("OPENAI_API_KEY")).toBeVisible();
  await expect(page.locator("body")).not.toContainText("sk-");
});

test("project and server policies expose secure defaults", async ({ page }) => {
  await page.goto("/projects/weeknote/settings");
  await expect(page.getByText("Hosted application access")).toBeVisible();
  await expect(page.getByText("no dispatcher executes project code", { exact: false })).toBeVisible();
  await page.goto("/settings/server");
  await expect(page.getByText("Operator-managed network")).toBeVisible();
  await expect(page.getByText("Account recovery")).toBeVisible();
});

test("keyboard navigation has visible focus and semantic landmarks", async ({ page, browserName }) => {
  test.skip(browserName !== "chromium", "Focus rendering is checked in Chromium.");
  await page.goto("/");
  await expect(page.getByRole("navigation", { name: "Workspace" })).toBeVisible();
  await page.keyboard.press("Tab");
  const focused = page.locator(":focus-visible");
  await expect(focused).toBeVisible();
  const outline = await focused.evaluate((element) => getComputedStyle(element).outlineStyle);
  expect(outline).not.toBe("none");
});

test("mobile navigation preserves access to every workspace surface", async ({ page, isMobile }) => {
  test.skip(!isMobile, "Mobile navigation is checked on the mobile project.");
  await page.goto("/");
  await page.getByRole("button", { name: "Open navigation" }).click();
  await expect(page.getByRole("navigation", { name: "Workspace" })).toBeVisible();
  await page.getByRole("link", { name: "Trusted peers" }).click();
  await expect(page.getByRole("heading", { name: "Trusted peers" })).toBeVisible();
  await expect(page.locator("body")).not.toHaveCSS("overflow-x", "scroll");
});
