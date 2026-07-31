import { expect, test } from "@playwright/test";
import { mockC6 } from "./fixtures";

test.beforeEach(async ({ page }) => { await mockC6(page); });

test("workspace lists, filters, and opens small software", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Your small software" })).toBeVisible();
  await expect(page.locator(".project-main").filter({ hasText: "Weeknote" })).toBeVisible();
  await page.getByRole("textbox", { name: "Filter projects" }).fill("receipt");
  await expect(page.locator(".project-main").filter({ hasText: "Receipt Box" })).toBeVisible();
  await expect(page.locator(".project-main").filter({ hasText: "Weeknote" })).toBeHidden();
  await page.getByRole("textbox", { name: "Filter projects" }).fill("weeknote");
  await page.locator(".project-main").filter({ hasText: "Weeknote" }).click();
  await expect(page).toHaveURL(/projects\/weeknote$/);
  await expect(page.getByLabel("Project metadata lifecycle")).toContainText("Deploy records");
});

test("repository navigation preserves source history", async ({ page }) => {
  await page.goto("/projects/weeknote/files");
  await expect(page.getByRole("button", { name: /c6.toml/ })).toBeVisible();
  await page.getByRole("link", { name: "Branches" }).click();
  await expect(page.getByText("agent/friday-notes/42")).toBeVisible();
  await page.getByRole("link", { name: "Commits" }).click();
  await expect(page.getByText("Make summaries easier to scan")).toBeVisible();
  await page.getByRole("link", { name: /Pull requests/ }).click();
  await expect(page.getByText("Draft summaries from the week’s activity")).toBeVisible();
  await expect(page.getByRole("link", { name: "Preview" })).toBeVisible();
});

test("create and import flows keep private access as the default", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("link", { name: "New project" }).click();
  await expect(page.getByRole("radio", { name: /Workspace only/ })).toBeChecked();
  await page.getByLabel("Project name").fill("Release Notes");
  await expect(page.getByText("paper-street/release-notes")).toBeVisible();
  await page.getByRole("button", { name: /Create project/ }).click();
  await expect(page).toHaveURL(/projects\/release-notes/);
  await page.goto("/import");
  await expect(page.getByRole("heading", { name: "Git import is planned" })).toBeVisible();
  await expect(page.getByText("C6 does not accept repository URLs or access tokens yet.")).toBeVisible();
});

test("empty and loading states remain actionable", async ({ page }) => {
  await page.unroute("**/api/v1/**"); await mockC6(page, { empty: true, delay: 300 });
  await page.goto("/");
  await expect(page.getByLabel("Loading C6")).toBeVisible();
  await expect(page.getByText("0 projects")).toBeVisible();
  await expect(page.getByRole("link", { name: "New project" })).toBeVisible();
});
