# Moved to https://codeberg.org/icewind/prometheus-edge-trigger

# Prometheus edge trigger

Trigger http requests a set delay after an edge in a prometheus query.

## Example

Given 2 wifi enabled switches that have their state logged into prometheus,
automatically turn off `switch2` 5 minutes `switch1` is turned off.

```toml
[prometheus]
url = "http://prometheus"

[[trigger]]
name = "Delay swtich"

[trigger.condition]
query = "switch_state{instance=\"$instance\"}"
from = 1
to = 0
params.instance = { type = "mdns", service = "_switch-http._tcp.local", host = "switch1" }

[trigger.action]
method = "PUT"
params.host = { type = "mdns", service = "_switch-http._tcp.local", host = "switch2" }
url = "http://$host/off"
delay = 300
```

## Parameters

To remove the need to hard code ip addresses or host names, you can configure parameters to query for an ip address and
use it in a query or url.

### MDNS

 ```toml
params.host = { type = "mdns", service = "_switch-http._tcp.local", host = "switch2" }
 ```

Will attempt to lookup a host advertising itself under the `_switch-http._tcp.local` service with host name `switch2`
and return the ip address and port of the host.

### Service

```toml
params.host = { type = "service", file = "services.json", key = "hostname", value = "switch2" }
```

Will look for a host in a json file in [prometheus' file based service discovery format](https://prometheus.io/docs/guides/file-sd/)
for an entry containing the label `hostname` with value `switch2` and return the first target.
