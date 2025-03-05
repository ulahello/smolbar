# `smolbar` is unmaintained

**smolbar is no longer maintained, but was complete in 2023.**
I recommend using [i3blocks](https://github.com/vivien/i3blocks) instead.

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
The user controls `smolbar`'s behavior through its configuration file.
Note that its configurable behavior is a superset of the behavior outlined in the protocol.

[^1]: It's a good idea to read through this man page if you're having issues configuring or understanding `smolbar`.

### Block

A block is a unit of refreshable content.
The structure of this content is defined by the `Body` JSON object from the protocol, with additional information to make them useful.
To be dynamic, blocks require both a "what" (a source of content) and a "when" (when to refresh the content).

The "what" is implemented by giving blocks a command to execute.
The entire `Body` JSON object is filled in with the output of this command.

The "when" is currently implemented in two ways: periodic intervals and operating system signals.
This means that a block's content gets refreshed on a timer and whenever `smolbar` receives a specific signal.

See [local scope configuration](#local-scope).

### Bar

The bar is the owner of blocks[^2].
The core behavior of `smolbar` is to send the bar's blocks whenever a block has new content.

The bar is also responsible for responding to `cont_signal` and `stop_signal`, which it sends in the `Header` JSON object (also from the protocol).
If it receives `stop_signal`, `smolbar` will gracefully shut down, as per spec.
Upon receiving `cont_signal`, `smolbar` will reload its configuration.
Note that `smolbar` has given new meaning to `cont_signal`, since the meaning described by the protocol isn't particularly applicable.

[^2]: Outside of the codebase, "bar" isn't a very useful abstraction, and could be thought of as `smolbar` itself.

## Configuration

`smolbar` is configured through a TOML file.

If `--config` is not specified as an argument, `smolbar` looks for a file named `config.toml` in `$XDG_CONFIG_HOME/smolbar` or `$HOME/.config/smolbar`.

[Examples](./examples) of configurations are available.

### Header

The `Header` first sent to sway (defined by `swaybar-protocol(7)`) can be configured in the `header` TOML table.
It inherits all keys verbatim from `Header`.

```toml
[header]
cont_signal = "SIGCONT" # default value, as per swaybar-protocol(7)
stop_signal = "SIGINT"
```

### Blocks

There are three scopes at which the content[^3] and behavior of blocks can be defined.

- "Global" scope has the lowest precedence, but applies to all blocks.
- "Local" scope is defined per block.
- "Immediate" scope has the highest precedence, and is defined per block, but by the block's command.

This means that the global and local scopes can be used to give `Body` properties default values, while immediate scopes are useful for properties that change.

[^3]: "Content of the blocks" refers to a superset of the properties of the `Body` JSON object defined by `swaybar-protocol(7)`. More information on this is found in the [mental model](#mental-model) section.

#### Global scope

The global scope is configured at the root level of the configuration file.

| Key              | Type   | Description                                                                                                                                                                                                           |
|------------------|--------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| command\_dir     | string | sets the directory in which to execute `command` (defined in local scope)                                                                                                                                             |
| smolbar\_version | string | requires the current `smolbar` version to satisfy the given version requirement (parsed according to [Cargo's flavor of Semantic Versioning](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html)) |

The global scope also inherits all the properties from the `Body` JSON object defined by `swaybar-protocol(7)`.

For example:

```toml
# global
full_text = "only visible if no other scopes define full_text"

[[block]]
# local
full_text = "never see global full_text"
```

#### Local scope

All local scopes are tables in the table array `block`.

| Key      | Type   | Description                                                      |
|----------|--------|------------------------------------------------------------------|
| command  | string | path of command to execute in full[^4] for new content           |
| prefix   | string | prefixes `full_text`                                             |
| postfix  | string | appended to `full_text`                                          |
| interval | number | interval, in seconds, at which to periodically refresh the block |
| signal   | string | operating system signal name to refresh the block when received  |

The local scope inherits all other keys from `Body`.

For example:

```toml
[[block]]
# this block displays the date every second
command = "date" # assuming date coreutil is in $PATH
prefix = "Today is "
interval = 1
```

[^4]: A refresh will not disrupt the execution of the command, it will wait until the command finishes.

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
# cont_signal is SIGCONT by default, as per swaybar-protocol(7)
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

## Supported signals

The following operating system signals are currently supported:

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

## Security considerations

**By nature, `smolbar` executes arbitrary code** as defined in its configuration file.

If an attacker can write to the configuration file, or to *any* of the files defined as commands, that attacker is able to execute arbitrary code (either immediately or when the configuration is reloaded).

It is **your responsibility** to prevent this.
It's a good idea to ensure that no other users are granted write permissions for the configuration or its commands.
However, measures you take will **depend on your situation**.

## License

`smolbar` is licensed under the GNU General Public License v3.0 or later.

See [LICENSE](./LICENSE) for the full license text.
