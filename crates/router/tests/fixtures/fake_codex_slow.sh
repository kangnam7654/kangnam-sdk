#!/bin/sh
sleep 5
echo '{"type":"item.completed","item":{"item_type":"assistant_message","text":"too late"}}'
echo '{"type":"turn.completed","usage":{"input_tokens":1,"cached_input_tokens":0,"output_tokens":1}}'
