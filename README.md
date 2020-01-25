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