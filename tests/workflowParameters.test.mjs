import assert from "node:assert/strict";
import { test } from "node:test";
import { registerHooks } from "node:module";
import { readFileSync, existsSync } from "node:fs";
import ts from "typescript";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { descriptorProblem } from "../src/workflowDraft.ts";

// Use the existing compiler for JSX; no production or test runtime dependency.
const hooks = registerHooks({
  resolve(specifier, context, next) {
    if (specifier.startsWith(".") && context.parentURL) {
      for (const ext of [".ts", ".tsx"]) {
        const url = new URL(specifier + ext, context.parentURL);
        if (existsSync(url)) return next(url.href, context);
      }
    }
    return next(specifier, context);
  },
  load(url, context, next) {
    if (url.endsWith(".tsx")) return { format: "module", shortCircuit: true,
      source: ts.transpileModule(readFileSync(new URL(url), "utf8"), { compilerOptions: { jsx: ts.JsxEmit.ReactJSX, module: ts.ModuleKind.ESNext } }).outputText };
    return next(url, context);
  },
});
const { WorkflowParameters } = await import("../src/components/WorkflowParameters.tsx");
const { SavedWorkflows } = await import("../src/components/SavedWorkflows.tsx");
hooks.deregister();

test("a new registry action renders all existing control types without custom action code", () => {
  const descriptor = { id: "test.future_registration", workflow_compatible: true, schema_version: 7,
    terminal: false, confirmation: "none", parameters: [
      { key: "title", label: "Title", type: "text", max_length: 80, required: true },
      { key: "amount", label: "Amount", type: "number", min: 0, max: 100, integer: true, required: true },
      { key: "enabled", label: "Enabled", type: "boolean", required: true },
      { key: "mode", label: "Mode", type: "choice", options: ["alpha", "beta"], required: true },
      { key: "app", label: "App", type: "application", required: true },
      { key: "file", label: "File", type: "file", required: true },
      { key: "folder", label: "Folder", type: "folder", required: false },
    ] };
  assert.equal(descriptorProblem(descriptor), "");
  const markup = renderToStaticMarkup(React.createElement(WorkflowParameters, {
    descriptor, step: { id: "step", parameters: { title: "Saved title", amount: 25, enabled: true, mode: "beta", app: "catalog-id", folder: "C:\\資料 space" } },
    errors: {}, apps: [{ id: "catalog-id", display_name: "Installed Test App" }], appError: "", appsLoading: false,
    refreshApps() {}, change() {}, service: {},
  }));
  for (const expected of ['value="Saved title"', 'value="25"', 'type="checkbox"', 'checked=""', 'value="beta" selected=""',
    "Installed Test App", "Search installed applications", "Choose file", "Choose folder", "C:\\資料 space"]) assert.ok(markup.includes(expected), expected);
});

test("saved workflow list renders independent description, count, update time and unavailable details", () => {
  const markup = renderToStaticMarkup(React.createElement(SavedWorkflows, {
    items: [{ workflow: { id: "stable-id", name: "My workflow", description: "A useful description", aliases: [], steps: [{}, {}], updated_at: 1700000000000, revision: 2 }, executable: false, issues: ["Step 1 reference unavailable"] }],
    loading: false, error: "", disabled: false, service: {},
  }));
  for (const expected of ["My workflow", "A useful description", "2", "actions", "Updated", "2023-11-14T22:13:20.000Z", "Needs attention", "Step 1 reference unavailable", "Create New Workflow", "Search saved workflows", "Delete"]) assert.ok(markup.includes(expected), expected);
});

test("empty state is shown only after a successful load", () => {
  for (const [loading, error, empty] of [[false, "", true], [true, "", false], [false, "Storage failed", false]]) {
    const markup = renderToStaticMarkup(React.createElement(SavedWorkflows, { items: [], loading, error, disabled: false, service: {} }));
    assert.equal(markup.includes("No saved workflows yet"), empty);
  }
});

test("missing application and obsolete choice values stay visible for repair", () => {
  const markup = renderToStaticMarkup(React.createElement(WorkflowParameters, {
    descriptor: { parameters: [{ key: "app", type: "application", label: "App", required: true }, { key: "mode", type: "choice", label: "Mode", options: ["current"], required: true }] },
    step: { id: "saved-step", parameters: { app: "missing-app-id", mode: "obsolete" } },
    errors: {}, apps: [], appError: "", appsLoading: false, refreshApps() {}, change() {}, service: {},
  }));
  assert.ok(markup.includes('value="missing-app-id" selected=""'));
  assert.ok(markup.includes("Saved application unavailable"));
  assert.ok(markup.includes('value="obsolete" selected=""'));
  assert.ok(markup.includes("Unavailable saved value: obsolete"));
});
