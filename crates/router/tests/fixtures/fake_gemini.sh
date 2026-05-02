#!/bin/sh
# Ignore all args; emit canned stream-json events mimicking real Gemini CLI output.
echo '{"type":"system","model":"gemini-1.5-flash"}'
echo '{"type":"assistant","message":{"content":[{"type":"text","text":"Hello "}]}}'
echo '{"type":"assistant","message":{"content":[{"type":"text","text":"world!"}]}}'
echo '{"type":"result","status":"ok","total_cost_usd":0.0001}'
exit 0
