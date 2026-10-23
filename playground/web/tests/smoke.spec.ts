// Smoke test (PLAN §15): Snip renders, an error dims the view and shows a
// diagnostic, and fixing it updates the view.
import { expect, test } from "@playwright/test";

test("edit, break, and fix a model", async ({ page }) => {
  await page.goto("/");

  // Snip renders as a tower with its 7 containers.
  const pane = page.locator(".viewer-pane");
  const stage = page.locator(".saxl-stage");
  await expect(pane).toHaveAttribute("data-state", "fresh");
  await expect(stage).toHaveAttribute("data-containers", "7");
  await expect(page.locator(".diags li.empty")).toHaveText("No problems.");

  // An undeclared relation is an error: diagnostics appear and the view dims.
  const editor = page.locator(".cm-content");
  await editor.click();
  await page.keyboard.press("ControlOrMeta+End");
  await page.keyboard.press("Enter");
  await page.keyboard.type("Foo -> Bar");
  await page.keyboard.press("Escape");
  await expect(page.locator(".diags li.error .code").first()).toHaveText("E002");
  await expect(pane).toHaveAttribute("data-state", "stale");
  await expect(page.locator(".stale-note")).toBeVisible();

  // Replacing the line with a new service fixes it, and the view updates.
  await page.keyboard.press("End");
  await page.keyboard.press("Shift+Home");
  await page.keyboard.type("service Audit");
  await page.keyboard.press("Escape");
  await expect(pane).toHaveAttribute("data-state", "fresh");
  await expect(stage).toHaveAttribute("data-containers", "8");
  await expect(page.locator(".diags li.error")).toHaveCount(0);
});

test("explain a diagnostic", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".viewer-pane")).toHaveAttribute("data-state", "fresh");
  await page.locator(".cm-content").click();
  await page.keyboard.press("ControlOrMeta+End");
  await page.keyboard.press("Enter");
  await page.keyboard.type("service Legacy");
  await page.keyboard.press("Escape");
  await page.locator(".diags li", { hasText: "W104" }).getByRole("button", { name: "Explain" }).click();
  await expect(page.locator("dialog.explain h2")).toHaveText("W104");
  await expect(page.locator("dialog.explain pre")).toContainText("# W104:");
});
