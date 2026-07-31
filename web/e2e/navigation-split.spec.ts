import { expect, test } from "@playwright/test";
import { mockC6 } from "./fixtures";

test("administrator switches between C6 Hub and C6 Admin without a reload", async ({ page, isMobile }) => {
  test.skip(isMobile, "The context switch has a dedicated mobile regression.");
  await mockC6(page);
  await page.goto("/");

  await expect(page.getByRole("navigation", { name: "C6 Hub" })).toBeVisible();
  await page.getByRole("link", { name: "Admin", exact: true }).click();
  await expect(page).toHaveURL(/\/admin$/);
  await expect(page.getByRole("navigation", { name: "C6 Admin" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "This C6 server" })).toBeVisible();

  await page.getByRole("link", { name: "Access & invitations" }).click();
  await expect(page).toHaveURL(/\/admin\/access$/);
  await expect(page.getByRole("heading", { name: "Access & invitations" })).toBeVisible();
  await page.getByRole("link", { name: "Hub" }).click();
  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByRole("heading", { name: "Your small software" })).toBeVisible();
});

test("workspace owner cannot enter C6 Admin without installation authority", async ({ page, isMobile }) => {
  test.skip(isMobile, "Authority separation is viewport-independent.");
  await mockC6(page, { serverAdministrator: false });
  await page.goto("/admin/access");

  await expect(page.getByRole("heading", { name: "C6 Admin is restricted" })).toBeVisible();
  await expect(page.getByRole("link", { name: "Admin", exact: true })).toHaveCount(0);
  await expect(page.getByRole("heading", { name: "Access & invitations" })).toHaveCount(0);
  await expect(page.getByText(/Workspace roles do not grant installation administration/)).toBeVisible();
});

test("administrator without a workspace can open C6 Admin", async ({ page, isMobile }) => {
  test.skip(isMobile, "Empty-workspace routing is viewport-independent.");
  await mockC6(page, { noWorkspaces: true });
  await page.goto("/admin");

  await expect(page.getByRole("heading", { name: "This C6 server" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Name your C6 workspace" })).toHaveCount(0);
});

test("mobile navigation follows the selected C6 surface and closes after navigation", async ({ page, isMobile }) => {
  test.skip(!isMobile, "Mobile drawer behavior is checked on the mobile project.");
  await mockC6(page);
  await page.goto("/");

  await page.getByRole("link", { name: "Admin", exact: true }).click();
  await expect(page.getByRole("heading", { name: "This C6 server" })).toBeVisible();
  await page.getByRole("button", { name: "Open navigation" }).click();
  await expect(page.getByRole("navigation", { name: "C6 Admin" })).toBeVisible();
  await page.getByRole("link", { name: "Access & invitations" }).click();
  await expect(page).toHaveURL(/\/admin\/access$/);
  await expect(page.locator(".sidebar")).not.toHaveClass(/open/);

  await page.getByRole("link", { name: "Hub" }).click();
  await expect(page.getByRole("heading", { name: "Your small software" })).toBeVisible();
  await page.getByRole("button", { name: "Open navigation" }).click();
  await expect(page.getByRole("navigation", { name: "C6 Hub" })).toBeVisible();
});
