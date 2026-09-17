# Discover tools from the MCP server
# MCP_SESSION_ID must be set, obtain it using test_initialize.sh

curl -s -X POST "http://localhost:8080/mcp" \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -H "Mcp-Session-Id: $MCP_SESSION_ID" \
  -d '{ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} }'

