# loranto

A simple tool to send messages into [FreakWAN](https://github.com/antirez/freakwan)
over Bluetooth low energy.

**This project is in pre-alpha please use the [python implementation](https://pypi.org/project/freakble/)
instead.**

The name of the project is the italian word for [Loranthus](https://en.wikipedia.org/wiki/Loranthus).

## Installation

### From source

To build the latest version of `loranto` clone the repository and run:

```
cargo build --release
```

During development you want to build in debug mode with just:

```
cargo build
```

## Usage

```console
Usage: loranto [OPTIONS] [COMMAND]

Commands:
  scan  Scan to find Bluetooth LE devices
  help  Print this message or the help of the given subcommand(s)

Options:
      --adapter <ADAPTER>  [default: hci0]
  -h, --help               Print help
  -V, --version            Print version
```
