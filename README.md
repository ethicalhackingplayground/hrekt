# rekt
A really fast http prober.

### Installation
```rust
cargo install rekt
```

### Usage

```bash
USAGE:
    rekt [OPTIONS]

OPTIONS:
    -c, --rate <rate>                  Maximum in-flight requests per second [default: 1000]
    -t, --concurrency <concurrency>    The amount of concurrent requests [default: 100]
    -t, --timeout <timeout>            The delay between each request [default: 3]
    -w, --workers <workers>            The amount of workers [default: 1]
    -p, --ports <ports>                the ports to probe default is (80,443) [default: 80,443]
    -r, --regex <regex>                regex to be used to match a specific pattern in the response
                                       [default: ]
    -h, --help                         Print help information
    -V, --version                      Print version information
```

```bash
cat subs.txt | rekt
```
