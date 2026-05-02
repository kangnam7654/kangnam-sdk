#!/bin/sh
echo '{"type":"system","model":"claude-sonnet-4-5"}'
echo '{"type":"assistant","message":{"model":"claude-sonnet-4-5","content":[{"type":"text","text":"Hi "}]}}'
echo '{"type":"assistant","message":{"model":"claude-sonnet-4-5","content":[{"type":"text","text":"there!"}],"usage":{"input_tokens":5,"output_tokens":3}}}'
echo '{"type":"result","result":"Hi there!","total_cost_usd":0.001,"is_error":false,"usage":{"input_tokens":5,"output_tokens":3}}'
exit 0
