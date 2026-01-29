#!/bin/bash

echo "🚀 Demonstrating Pnpm Enforcement Hook"
echo "======================================="

# Build the project first
echo "📦 Building project-lint..."
if command -v cargo &> /dev/null; then
    cargo build --quiet
else
    echo "❌ Cargo not found. Please install Rust or use the nix dev environment:"
    echo "   nix develop"
    exit 1
fi

echo ""
echo "📋 Testing hook with sample npm command..."
echo "   (This simulates what an IDE would send to the hook)"

# Create a sample event that mimics Windsurf sending an npm command
cat << 'EOF' | ./target/debug/project-lint hook --source windsurf --path ./test-area/sample-pnpm-project
{
  "agent_action_name": "pre_mcp_tool_use",
  "tool_info": {
    "mcp_tool_name": "bash",
    "mcp_tool_arguments": {
      "input": "npm install express"
    }
  },
  "trajectory_id": "demo-session-123",
  "timestamp": "2025-01-28T20:00:00Z"
}
EOF

echo ""
echo "✅ Demo complete!"
echo ""
echo "📖 To use this in an IDE:"
echo "   Windsurf: project-lint hook --source windsurf"
echo "   Claude:   project-lint hook --source claude"
echo "   Generic:  project-lint hook --source generic"
