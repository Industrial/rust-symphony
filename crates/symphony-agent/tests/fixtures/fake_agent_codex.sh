#!/bin/sh
# Fake Codex agent: responds to initialize, thread/start, turn/start then sends turn/completed.
read -r line  # initialize (id 1)
printf '{"jsonrpc":"2.0","id":1,"result":{}}\n'
read -r line  # initialized (notification)
read -r line  # thread/start (id 2)
printf '{"jsonrpc":"2.0","id":2,"result":{"thread":{"id":"t1"}}}\n'
read -r line  # turn/start (id 3)
printf '{"jsonrpc":"2.0","id":3,"result":{"turn":{"id":"turn1"}}}\n'
printf '{"method":"turn/completed","params":{}}\n'
