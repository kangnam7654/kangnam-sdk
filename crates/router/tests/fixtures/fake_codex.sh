#!/bin/sh
# Ignore all args; emit canned stream-json events mimicking codex exec --json output
echo '{"type":"thread.started","model":"gpt-5-codex"}'
echo '{"type":"item.completed","item":{"type":"agent_message","text":"Hello "}}'
echo '{"type":"item.completed","item":{"type":"agent_message","text":"world!"}}'
echo '{"type":"turn.completed","usage":{"input_tokens":10,"output_tokens":20}}'
exit 0
