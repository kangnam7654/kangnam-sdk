#!/bin/sh
echo '{"type":"system","subtype":"init","model":"claude-sonnet-4-5"}'
echo '{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"Reasoning step..."}]}}'
echo '{"type":"assistant","message":{"content":[{"type":"text","text":"Final answer."}]}}'
echo '{"type":"result","subtype":"success","total_cost_usd":0.001,"usage":{"input_tokens":50,"output_tokens":30}}'
