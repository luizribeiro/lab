import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";

const command = "cargo";
const args = ["run", "-q", "-p", "mcp-server", "--", "serve"];

const client = new Client(
  { name: "manual-validation", version: "0.1.0" },
  { capabilities: { experimental: { progressNotifications: true } } },
);

const transport = new StdioClientTransport({ command, args });

const progressEvents = [];

await client.connect(transport);

console.log("=== leg 1: progress_demo (expect 3 progress notifications) ===");
const progressResult = await client.callTool(
  { name: "progress_demo", arguments: {} },
  undefined,
  {
    onprogress: (event) => {
      progressEvents.push(event);
      console.log("← notifications/progress", JSON.stringify(event));
    },
  },
);
console.log("→ progress_demo result:", JSON.stringify(progressResult));
console.log(`captured ${progressEvents.length} progress notification(s)`);

console.log("\n=== leg 2: long_running_demo cancelled mid-flight ===");
const controller = new AbortController();
const callPromise = client
  .callTool({ name: "long_running_demo", arguments: {} }, undefined, {
    signal: controller.signal,
  })
  .then(
    (result) => ({ ok: true, result }),
    (error) => ({ ok: false, error: String(error?.message ?? error) }),
  );

await new Promise((resolve) => setTimeout(resolve, 250));
console.log("→ aborting (sends notifications/cancelled)");
controller.abort();
const cancelOutcome = await callPromise;
console.log("→ long_running_demo outcome:", JSON.stringify(cancelOutcome));

await client.close();

if (progressEvents.length < 1) {
  console.error("FAIL: expected at least one progress notification");
  process.exit(1);
}
if (cancelOutcome.ok) {
  console.error("FAIL: expected cancelled call to not return a normal result");
  process.exit(1);
}

console.log("\nOK: one progress notification + one cancelled call captured");
