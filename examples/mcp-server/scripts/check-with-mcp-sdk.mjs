import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";

function commandConfig() {
  const command = process.env.MCP_SERVER_COMMAND ?? "cargo";
  const argsFromEnv = process.env.MCP_SERVER_ARGS;

  if (argsFromEnv && argsFromEnv.trim().length > 0) {
    return { command, args: argsFromEnv.split(" ").filter(Boolean) };
  }

  return {
    command,
    args: ["run", "-q", "-p", "mcp-server", "--", "serve"],
  };
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

async function main() {
  const { command, args } = commandConfig();
  console.log(`Using MCP server command: ${command} ${args.join(" ")}`);

  const client = new Client(
    {
      name: "fittings-real-client-check",
      version: "0.1.0",
    },
    {
      capabilities: {},
    },
  );

  const transport = new StdioClientTransport({ command, args });

  try {
    await client.connect(transport);

    const tools = await client.listTools();
    const toolNames = (tools.tools ?? []).map((tool) => tool.name).sort();
    console.log("tools/list =>", toolNames);
    assert(toolNames.includes("echo"), "expected `echo` tool");
    assert(toolNames.includes("add"), "expected `add` tool");

    const echo = await client.callTool({
      name: "echo",
      arguments: { message: "hello from real MCP client" },
    });
    console.log("tools/call echo =>", JSON.stringify(echo));
    assert(Array.isArray(echo.content), "echo response should include content array");

    const add = await client.callTool({
      name: "add",
      arguments: { a: 2, b: 3 },
    });
    console.log("tools/call add =>", JSON.stringify(add));
    assert(Array.isArray(add.content), "add response should include content array");

    console.log("✅ Real MCP client check passed.");
  } finally {
    await client.close().catch(() => {});
  }
}

main().catch((error) => {
  console.error("❌ Real MCP client check failed:");
  console.error(error?.stack ?? error);
  process.exit(1);
});
