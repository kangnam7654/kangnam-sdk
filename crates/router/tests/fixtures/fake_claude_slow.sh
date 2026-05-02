#!/bin/sh
sleep 5
echo '{"type":"assistant","message":{"content":[{"type":"text","text":"too late"}]}}'
echo '{"type":"result","subtype":"success","total_cost_usd":0,"usage":{"input_tokens":1,"output_tokens":1}}'
