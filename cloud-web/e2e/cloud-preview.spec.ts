import { expect, test } from "@playwright/test";

test("claims the preview and reaches the workspace directory", async ({ page }) => {
  await page.goto("/claim?preview=1");
  await expect(page.getByRole("heading", { name: /give small software/i })).toBeVisible();
  await expect(page.getByText("Preview data")).toBeVisible();
  await page.getByRole("button", { name: "Enter preview" }).press("Enter");

  await expect(page).toHaveURL(/\/app\?preview=1$/);
  await expect(page.getByRole("heading", { name: "Workspaces" })).toBeVisible();
  await expect(page.getByText("route-7fk2.relay.cresix.com").first()).toBeVisible();
});

test("creates a workspace without implying local authorization", async ({ page }) => {
  await page.goto("/app/workspaces/new?preview=1");
  await expect(page.getByText(/does not grant access to any local c6 data/i)).toBeVisible();
  await page.getByLabel("Namespace").fill("tiny-team");
  await page.getByLabel("Workspace name").fill("Tiny team tools");
  await page.getByRole("button", { name: "Create workspace" }).click();
  await expect(page).toHaveURL(/\/app\?preview=1$/);
});

test("reveals an installation credential exactly on the completion step", async ({ page }) => {
  await page.goto("/app/installations/new?preview=1");
  await expect(page.getByTestId("connector-credential")).toHaveCount(0);
  await page.getByLabel("Installation label").fill("Kitchen laptop");
  await page.getByLabel("Local server ID").fill("c479b58f-6a46-4f4d-b855-e57b49b775b8");
  await page.getByRole("button", { name: "Register installation" }).click();

  await expect(page.getByRole("heading", { name: "Save the connector credential" })).toBeVisible();
  await expect(page.getByText("This is the only reveal.")).toBeVisible();
  await expect(page.getByTestId("connector-credential")).toContainText("c6c_preview_");
});

test("binds by immutable local workspace UUID", async ({ page }) => {
  await page.goto("/app/workspaces/workspace-new/bind?preview=1");
  await expect(page.getByText(/does not synchronize cloud membership/i)).toBeVisible();
  await page
    .getByLabel("Local workspace UUID")
    .fill("35a2c849-9f0b-4682-80d7-b79a25960584");
  await page.getByRole("button", { name: "Bind workspace" }).click();
  await expect(page).toHaveURL(/\/app\?preview=1$/);
});

test("connected directory makes the cross-origin transition explicit", async ({ page }) => {
  await page.goto("/paper-street/weeknote?preview=1&state=connected");
  await expect(page.getByRole("heading", { name: "Weeknote" })).toBeVisible();
  await expect(page.getByText(/leaving cresix.com/i)).toBeVisible();
  await expect(page.getByRole("link", { name: /open on c6/i })).toHaveAttribute(
    "href",
    "https://route-7fk2.relay.cresix.com/projects/weeknote",
  );
});

for (const state of ["offline", "revoked"] as const) {
  test(`${state} directory never offers an installation link`, async ({ page }) => {
    await page.goto(`/paper-street/weeknote?preview=1&state=${state}`);
    await expect(page.getByText(state, { exact: true })).toBeVisible();
    await expect(page.getByRole("link", { name: /open on c6/i })).toHaveCount(0);
    await expect(
      page.getByRole("heading", {
        name: state === "offline" ? /installation is offline/i : /route was revoked/i,
      }),
    ).toBeVisible();
  });
}

test("keyboard focus reaches the primary action", async ({ page }) => {
  await page.goto("/paper-street/weeknote?preview=1");
  await page.keyboard.press("Tab");
  await page.keyboard.press("Tab");
  await page.keyboard.press("Tab");
  await page.keyboard.press("Tab");
  await expect(page.getByRole("link", { name: /open on c6/i })).toBeFocused();
});

test("mobile workspace rows do not overflow the viewport", async ({ page }) => {
  await page.goto("/app?preview=1");
  const dimensions = await page.evaluate(() => ({
    scroll: document.documentElement.scrollWidth,
    client: document.documentElement.clientWidth,
  }));
  expect(dimensions.scroll).toBeLessThanOrEqual(dimensions.client);
  await expect(page.getByRole("link", { name: "paper-street" })).toBeVisible();
});
