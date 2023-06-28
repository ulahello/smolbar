# `smolbar`

[![Crates.io](https://img.shields.io/crates/v/smolbar)](https://crates.io/crates/smolbar)
[![Crates.io](https://img.shields.io/crates/l/smolbar)](https://crates.io/crates/smolbar)

`smolbar` is a smol status command for [sway](https://github.com/swaywm/sway).

## Installation

`smolbar` is on crates.io!

```console
$ cargo install --locked smolbar
```

## Usage

Make sure that `smolbar` is installed and in your `$PATH`.
In your sway configuration, invoke the `bar` subcommand `status_command` with `smolbar`.

```
bar {
	status_command smolbar
	# the rest of your bar config
}
```

## Mental model

`smolbar` fulfills the role described by `swaybar-protocol(7)`[^1].
As the user, you can configure `smolbar`'s behavior and what data it sends[^2] through its configuration.
Note that its configurable behavior is a superset of the behavior outlined in the protocol.

[^1]: It's a good idea to read through this man page if you're having issues configuring or understanding `smolbar`.
[^2]: Send, meaning write data to standard output. In the context of the protocol, this write is semantically a send.

### Block

A block is a unit of refreshable content.
The structure of this content is defined[^3] by the `Body` JSON object from the protocol.
However, blocks hold additional information to make them useful.
In order for their content to change, they need both a "what" and a "when".

The "what" is implemented by giving blocks a command to execute.
The entire `Body` JSON object is filled in with the output of this command.
See TODO: config command

The "when" is currently implemented in two ways: periodic intervals and OS signals.
The "what" gets refreshed according to the "when": at periodic intervals, and whenever `smolbar` receives a specific signal.
See TODO: config interval & signal

[^3]: There are a few keys outside of `Body` used by the configuration, such as `prefix` and `postfix`.

### Bar

The bar is the owner of blocks[^4].
The core behavior of `smolbar` is to send the bar's blocks whenever a block has new content.

The bar is also responsible for responding to `cont_signal` and `stop_signal`, which it sends in the `Header` JSON object (also from the protocol).
If it receives `stop_signal`, `smolbar` will gracefully shut down, as per spec.
Upon receiving `cont_signal`, `smolbar` will reload its configuration.
Note that `smolbar` has given new meaning to `cont_signal`, since the meaning described by the protocol isn't particularly applicable.

[^4]: Outside of the codebase, "bar" isn't a very useful abstraction, and could be thought of as `smolbar` itself.

## Configuration

`smolbar` is configured through a TOML file.

If `--config` is not specified as an argument, `smolbar` looks for a file named `config.toml` in `$XDG_CONFIG_HOME/smolbar` or `$HOME/.config/smolbar`.

[Examples](./examples) of configurations are available.

### Header

The `Header` first sent to sway (defined by `swaybar-protocol(7)`) can be configured in the `header` TOML table.
It inherits all keys verbatim from `Header`.

```toml
[header]
cont_signal = "SIGCONT" # default value, as per `swaybar-protocol(7)`
stop_signal = "SIGINT"
```

### Blocks

There are three scopes at which the content[^5] and behavior of blocks can be defined.

- "Global" scope has the lowest precedence, but applies to all blocks.
- "Local" scope is defined per block.
- "Immediate" scope has the highest precedence, and is defined per block, but by the block's command.

This means that the global and local scopes can be used to give `Body` properties default values, while immediate scopes are useful for properties that change.

[^5]: "Content of the blocks" refers to a superset of the properties of the `Body` JSON object defined by `swaybar-protocol(7)`. More information on this is found in the [mental model](#mental-model) section.

#### Global scope

The global scope is configured at the root level of the configuration file.

| key              | type   | description                                                                                                                                                                                                           |
|------------------|--------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| command\_dir     | string | sets the directory to execute `command` (defined in local scope) in                                                                                                                                                   |
| smolbar\_version | string | requires that current `smolbar` version satisfies the given version requirement (parsed according to [Cargo's flavor of Semantic Versioning](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html)) |

The global scope also inherits all the properties from the `Body` JSON object defined by `swaybar-protocol(7)`.

```toml
# global
full_text = "only visible in a block if no other scopes define full_text"

[[block]]
# local
full_text = "never see global full_text"
```

#### Local scope

All local scopes are tables in the table array `block`.

| key      | type   | description                                         |
|----------|--------|-----------------------------------------------------|
| command  | string | command to execute[^6] for new content              |
| prefix   | string | prefixes `full_text`                                |
| postfix  | string | appended to `full_text`                             |
| interval | number | interval, in seconds, at which to refresh the block |
| signal   | string | OS signal name to refresh the block when received   |

The following signals are currently supported:

- `SIGALRM`
- `SIGCHLD`
- `SIGCONT`
- `SIGHUP`
- `SIGINT`
- `SIGIO`
- `SIGPIPE`
- `SIGQUIT`
- `SIGSTOP`
- `SIGTERM`
- `SIGUSR1`
- `SIGUSR2`
- `SIGWINCH`

The local scope inherits all other keys from `Body`.

```toml
[[block]]
# this block displays the date every second
command = "date" # assuming date coreutil is in $PATH
prefix = "Today is "
interval = 1
```

[^6]: The command is not currently passed to a shell, so if you try to pass arguments it will fail.

#### Immediate scope

Each line of the executed `command`'s (defined in local scope) standard output is parsed in order as a `Body` property.
The order is the same as they appear in `swaybar-protocol(7)`.

For example, suppose the following script was a block's command:

```sh
# interpreted as `full_text`
echo 'amazing status information'

# interpreted as `short_text`
echo 'short info'

# interpreted as `color`
echo '#ff0000'
```

### Hot swapping

`smolbar` responds to `cont_signal` by reloading its configuration.

This means that by default, sending `smolbar`'s process `SIGCONT` will cause it to hot swap its configuration.

```toml
[header]
# cont_signal is SIGCONT by default, as per `swaybar-protocol(7)`
```

```console
$ pkill -SIGCONT smolbar
# causes smolbar to reload config
```

`cont_signal` is also configurable.

```toml
[header]
cont_signal = "SIGUSR1"
```

```console
$ pkill -SIGUSR1 smolbar
# causes smolbar to reload config
```

Note that the header cannot be reconfigured during runtime.
This is because in `swaybar-protocol(7)`, it's only sent once, at the beginning of the status command's process.

## Security considerations

**By nature, `smolbar` executes arbitrary code** as defined in its configuration file.

If an attacker can write to the configuration file, or to *any* of the files defined as commands, that attacker is able to execute arbitrary code (either immediately or when the configuration is reloaded).

It is **your responsibility** to prevent this.
It's a good idea to ensure that no other users are granted write permissions for the configuration or its commands.
However, measures you take will **depend on your situation**.

## Contributions

Tickets and improvements are welcome and appreciated!
You can find the [issue tracker](https://github.com/ulahello/smolbar/issues) on GitHub.

Contributions will be licensed under the same license as `smolbar`.

## License

`smolbar` is licensed under the GNU General Public License v3.0 or later.

See [LICENSE](./LICENSE) for the full license text.
