# sawbill

Sawbill provides network connection analysis, and is meant to quickly provide connection information from remote devices - servers, desktops, etc. A GUI interface is planned that will provide both macro and micro level views of connections. 

**Note - sawbill is currently in the early stages and has limited features.**

# Step 1 - Start redis backend

The initial implementation of sawbill relies on a locally running redis Docker container. To start local redis using docker:
```
$ docker run --name=redis-sawbill --publish=6379:6379 --hostname=redis --restart=on-failure --detach redis:latest                                             
880f98611511d71de96fcf7648a4f2116cb5731cdb0e6dab8c47b594e4eece6e
```

The redis backend is used to store pieces of information pertaining to each TCP flow. The backend container is useful because memory and CPU can be carefully controlled. Remote intstances of redis, as well as other backend database types, is also planned.

# Step 2 - Start sawbill

Once the redis container is running, sawbill can be started against an interface to begin processing TCP flows. An interface argument is required. In the example below, `en8` is provided. Optionally, TCP flows can be filtered based on IPv4 address. In the example below, `-i 8.8.8.8` means that only TCP flows with an underlying IPv4 address of 8.8.8.8 will be captured.

```
$ RUST_LOG=debug cargo run en8 -i 8.8.8.8
[2021-10-18T00:16:16Z DEBUG sawbill_cli] en8: flags=8863<UP,BROADCAST,MULTICAST>
          index: 12
           inet: 192.168.254.18/24
[2021-10-18T00:16:16Z DEBUG sawbill_cli] Found local Ipv4 address: "192.168.254.18"
```

# Step 3 - Test sawbill

To test sawbill, once its running, `curl` to 8.8.8.8 using a separate terminal.

```
$ curl -v 8.8.8.8                         
*   Trying 8.8.8.8...
* TCP_NODELAY set
```

`curl` to 8.8.8.8 generates unanswered TCP SYN datagrams, which sawbill will eventually warn against:

Log messages:

```
[2021-10-18T00:11:18Z DEBUG sawbill_cli] Flow determination | local 192.168.254.18 | src 192.168.254.18 | dst 8.8.8.8 | z_to_a
[2021-10-18T00:11:19Z DEBUG sawbill_cli] Flow determination | local 192.168.254.18 | src 192.168.254.18 | dst 8.8.8.8 | z_to_a
[2021-10-18T00:11:19Z DEBUG sawbill_cli] 8.8.8.8:80<->192.168.254.18:55036 | z_to_a_syn_counter: 1
[2021-10-18T00:11:20Z DEBUG sawbill_cli] Flow determination | local 192.168.254.18 | src 192.168.254.18 | dst 8.8.8.8 | z_to_a
[2021-10-18T00:11:20Z DEBUG sawbill_cli] 8.8.8.8:80<->192.168.254.18:55036 | z_to_a_syn_counter: 2
[2021-10-18T00:11:21Z DEBUG sawbill_cli] Flow determination | local 192.168.254.18 | src 192.168.254.18 | dst 8.8.8.8 | z_to_a
[2021-10-18T00:11:21Z DEBUG sawbill_cli] 8.8.8.8:80<->192.168.254.18:55036 | z_to_a_syn_counter: 3
[2021-10-18T00:11:21Z WARN  sawbill_cli] 8.8.8.8:80<->192.168.254.18:55036 | 3 or more unanswered SYN packets
```

Z end is considered the local machine on which sawbill is running, and the A end is any remote machine. In this example the `curl` was against 8.8.8.8 (A end) from 192.168.254.18 (Z end). Notice that after 3 unanswered SYN datagrams a WARN log is generated that indicates "3 or more unanswered SYN packets". 

The TCP analysis capabilities of `sawbill` will continue to grow, and a roadmap will be added soon.

