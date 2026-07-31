import { expect, test } from "@playwright/test";
import { mockC6 } from "./fixtures";

test.beforeEach(async ({ page }) => { await mockC6(page); });

test("first owner claim explains and advances through local trust", async ({ page }) => {
  await page.goto("/claim");
  await expect(page.getByRole("heading", { name: "Claim this C6 server" })).toBeVisible();
  await expect(page.getByText(/public-key field is a placeholder/)).toBeVisible();
  await page.getByRole("textbox", { name: "Bootstrap token" }).fill("claim-token");
  await page.getByRole("textbox", { name: "Your name" }).fill("Neal");
  await page.getByRole("textbox", { name: "Device label" }).fill("Test browser");
  await expect(page.getByRole("button", { name: "Claim server" })).toBeEnabled();
});

test("pair token is scrubbed before approval and never rendered", async ({ page }) => {
  await page.goto("/join#token=one-time-secret-proof");
  await expect(page).toHaveURL(/\/join$/);
  await expect(page.getByText("one-time-secret-proof")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Join server" })).toBeDisabled();
  await page.getByRole("textbox", { name: "Your name" }).fill("Amy");
  await page.getByRole("textbox", { name: "Device label" }).fill("Test browser");
  const pairRequest = page.waitForRequest((request) => request.url().endsWith("/invites/redeem"));
  await page.getByRole("button", { name: "Join server" }).click();
  expect((await pairRequest).postDataJSON().token).toBe("one-time-secret-proof");
  await expect(page).toHaveURL("/");
});

test("administrator sees live peers and creates a scoped invite", async ({ page }) => {
  await page.addInitScript(() => Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: async () => undefined } }));
  await page.goto("/admin/access");
  await expect(page.getByText("Administrator-issued invitations and cookie sessions")).toBeVisible();
  await expect(page.getByText("Local peer record")).toBeVisible();
  await page.getByRole("button", { name: "Invite peer" }).click();
  await page.getByRole("combobox").selectOption("reader");
  await page.getByRole("button", { name: "Create invite" }).click();
  await expect(page.getByRole("status")).toContainText("expires in 24 hours");
  await expect(page.locator("body")).not.toContainText("/join#token=opaque");
  await expect(page.getByRole("button", { name: "Copy invitation" }).first()).toBeEnabled();
  await page.getByRole("button", { name: "Copy invitation" }).first().click();
  await expect(page.getByRole("status")).toContainText("Invitation copied");
  await page.evaluate(() => Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: async () => { throw new Error("denied"); } } }));
  await page.getByRole("button", { name: "Copy invitation" }).first().click();
  await expect(page.getByRole("status")).toContainText("could not copy");
  await expect(page.locator("body")).not.toContainText("/join#token=opaque");
});

test("rejected pairing codes show a useful error without exposing the token", async ({ page }) => {
  await page.route("**/api/v1/invites/redeem", (route) => route.fulfill({ status: 410, json: { error: { message: "Invitation has expired or was already used." } } }));
  await page.goto("/join#token=rejected-secret-proof");
  await page.getByRole("textbox", { name: "Your name" }).fill("Amy");
  await page.getByRole("textbox", { name: "Device label" }).fill("Test browser");
  await page.getByRole("button", { name: "Join server" }).click();
  await expect(page.getByRole("alert")).toContainText("Invitation has expired or was already used.");
  await expect(page.locator("body")).not.toContainText("rejected-secret-proof");
});

test("CSRF proof survives a reload because mutations read the cookie", async ({ context, page }) => {
  await context.addCookies([{ name: "c6_csrf", value: "reload-bound-proof", url: "http://127.0.0.1:4173", sameSite: "Strict" }]);
  await page.goto("/projects/weeknote/runs");
  await page.reload();
  const mutation = page.waitForRequest((request) => request.url().endsWith("/runs") && request.method() === "POST");
  await page.getByRole("button", { name: "Record run intent" }).click();
  expect((await mutation).headers()["x-c6-csrf"]).toBe("reload-bound-proof");
});
