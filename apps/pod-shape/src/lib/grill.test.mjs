// Run: node --test apps/pod-shape/src/lib/grill.test.mjs   (Node 23.6+ strips the TypeScript types)
// The pure helpers of the Grill screen: multiSelect join, the next open question, the ADR guard
// (must agree with Rust `is_adr_yes`), the Generate issues tooltip (owner decision).
import { test } from "node:test";
import assert from "node:assert/strict";
import { joinChoices, nextQuestion, isAdrQuestion, isAdrYes, isBriefQuestion, generateTooltip, composerPlaceholder } from "./grillHelpers.ts";

const q = (question, header, labels = ["Yes", "No"]) => ({
  question, header, multiSelect: false, options: labels.map((label) => ({ label, description: "" })),
});

test("multiSelect answers join the chosen labels with a comma", () => {
  assert.equal(joinChoices(["Phone", "Email"]), "Phone, Email");
  assert.equal(joinChoices(["Phone"]), "Phone");
});

test("the next question skips the ones with a draft answer", () => {
  const block = { messageId: 1, questions: { questions: [q("A?", "a"), q("B?", "b")] }, drafts: { "A?": "Yes" } };
  assert.deepEqual(nextQuestion(block), { index: 1, question: block.questions.questions[1] });
  assert.equal(nextQuestion({ ...block, drafts: { "A?": "x", "B?": "y" } }), null);
  assert.equal(nextQuestion(null), null);
});

test("an ADR yes fails closed, the same as Rust is_adr_yes", () => {
  for (const y of ["Yes", "yes, make the ADR", "Make an ADR", "YES"]) assert.ok(isAdrYes(y), y);
  for (const n of ["No, brief only", "Brief only", "Skip", "Yesterday's idea", "Maker", "not yes"]) assert.ok(!isAdrYes(n), n);
  assert.ok(isAdrQuestion(q("Make X an ADR?", "ADR")));
  assert.ok(!isAdrQuestion(q("Make X an ADR?", "Scope")));
});

test("brief questions and the Generate issues tooltip", () => {
  assert.ok(isBriefQuestion(q("Does this brief say what you mean?", "Brief")));
  assert.ok(isBriefQuestion(q("Does this brief say what you mean?", "Confirm")));
  assert.equal(generateTooltip("brief_ready", false), "Confirm the brief first");
  assert.equal(generateTooltip("confirmed", true), "Generate issues comes in build step 4");
});

test("composer placeholders follow the session state", () => {
  assert.equal(composerPlaceholder("brief_ready", null), "Ask for a change to the brief…");
  assert.equal(composerPlaceholder("waiting_owner", q("A?", "a", ["1", "2", "3"])), "Answer, or pick a suggestion (1–3)…");
  assert.equal(composerPlaceholder("stopped", null), "Press Resume to continue this session.");
  assert.equal(composerPlaceholder("running", null), "Wait for Claude…");
});
