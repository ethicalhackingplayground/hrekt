<h1 align="center">hrekt
  <br>
</h1>

<h4 align="center">A really fast http prober.</h4>

<p align="center">
  <a href="/LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg"/></a>
  <a href="https://www.rust-lang.org/"><img src="https://camo.githubusercontent.com/2ed8a73e5c5d21391f6dfc3ed93f70470c1d4ccf32824d96f943420163df9963/68747470733a2f2f696d672e736869656c64732e696f2f62616467652f4c616e67756167652d527573742d3138313731373f636f6c6f723d726564"/></a>
  <a href="https://github.com/ethicalhackingplayground/hrekt/issues"><img src="https://img.shields.io/badge/contributions-welcome-brightgreen.svg?style=flat"></a>
  <a href="https://twitter.com/z0idsec"><img src="https://img.shields.io/twitter/follow/z0idsec.svg?logo=twitter"></a>
  <a href="https://discord.gg/MQWCem5b"><img src="https://img.shields.io/discord/862900124740616192.svg?logo=discord"></a>
  <br>
</p>

---

<p align="center">
  <a href="#installation">Install</a> •
  <a href="#usage">Usage</a> •
  <a href="#examples">Examples</a> •
  <a href="#fyi">FYI</a> •
  <a href="#contributing">Contributing</a> •
  <a href="#license">License</a> •
  <a href="https://discord.gg/MQWCem5b">Join Discord</a> 
</p>

---

## Installation

#### Fresh Installs
```bash
cargo build -r
mv target/release/hrekt /bin/hrekt
```

#### Already been installed
```bash
cargo build -r
mv target/release/hrekt /<user>/.cargo/bin/
```


Make sure to replace `<user>` with your username.

or 

#### Installer
```bash
chmod +x install.sh ; ./install.sh
```

Can only be compiled locally right now.


## Usage

```bash
USAGE:
    hrekt [OPTIONS]

OPTIONS:
    -r, --rate <rate>
            Maximum in-flight requests per second [default: 1000]

    -c, --concurrency <concurrency>
            The amount of concurrent requests [default: 100]

    -t, --timeout <timeout>
            The delay between each request [default: 3]

    -w, --workers <workers>
            The amount of workers [default: 1]

    -p, --ports <ports>
            the ports to probe default ports are (80,443) [default: 80,443]

    -i, --title
            display the page titles

    -d, --tech-detect
            display the technology used

    -s, --status-code
            display the status-codes

    -x, --path <path>
            probe the specified path [default: ]

    -b, --body-regex <body-regex>
            regex to be used to match a specific pattern in the response [default: ]

    -h, --header-regex <header-regex>
            regex to be used to match a specific pattern in the header [default: ]

    -l, --follow-redirects
            follow http redirects

    -q, --silent
            suppress output

        --help
            Print help information

    -V, --version
            Print version information
```

---

## Examples

#### Display titles

```bash
cat subs.txt | hrekt --title
```

#### Probe ports

```bash
cat subs.txt | hrekt --ports 443,80,9200 
```

#### Display technologies

```bash
cat subs.txt | hrekt --tech-detect
```

#### Probe the response body

```bash
cat subs.txt | hrekt --body-regex 'href="\/content\/dam.*'
```

#### Probe the headers

```bash
cat subs.txt | hrekt --header-regex 'Server:.*'
```

#### Probe the path

```bash
cat subs.txt | hrekt --path /v1/api
```

#### Multiple Flags

```bash
cat subs.txt | hrekt --path /etc.clientlibs --tech-detect --title --body-regex 'href="\/content\/dam.*'
```

## FYI
It's advisable to only use tech detection when needed, as it tends to result in slow discoveries because we use chromium based detection.

---

If you find any cool bugs, it would be nice if I have some sorta appreciation such as shouting me out on your Twitter, buying me a coffee or donating to my Paypal.
  
[![BuyMeACoffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-ffdd00?style=for-the-badge&logo=buy-me-a-coffee&logoColor=black)](https://www.buymeacoffee.com/SBhULWm) [![PayPal](https://img.shields.io/badge/PayPal-00457C?style=for-the-badge&logo=paypal&logoColor=white)](https://www.paypal.com/paypalme/cyberlixpty)

I hope you enjoy

## Contributing

Pull requests are welcome. For major changes, please open an issue first
to discuss what you would like to change.

Please make sure to update tests as appropriate.


## License

Hrekt is distributed under [MIT License](https://github.com/ethicalhackingplayground/hrekt/blob/main/LICENSE)
