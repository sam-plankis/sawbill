# sawbill
Sawbill provides network connection analysis, and is meant to quickly provide connection information on a remote server. A GUI interface is planned that will provide both macro and micro level views of connections.

To start local redis using docker:
docker run --name=redis-sawbill --publish=6379:6379 --hostname=redis --restart=on-failure --detach redis:latest