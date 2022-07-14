# smolbar

`smolbar` is a smol status command for [sway](https://github.com/swaywm/sway).

## milestones

- [X] refresh configurable blocks on signals/intervals
- [X] respond to stop and continue signals
- [ ] support click events

# configuration

if `--config` is not specified, `smolbar` looks for the toml configuration file at `~/.config/smolbar/config.toml`.

for an example of a configuration, see the [examples](./examples).

## header

the header first sent to sway can be configured in the `header` table.
it inherets all keys from the `Header` JSON object defined in `swaybar-protocol(7)`.

## blocks

there are three scopes which can be used to configure individual blocks.
each scope has a level of control over the `Body`s of blocks: `immediate` has the highest precidence, then `local`, then `global`.

### global

`global` inherets all keys from the `Body` JSON object defined in `swaybar-protocol(7)`.

| key         | description                                  |
|-------------|----------------------------------------------|
| command_dir | sets the path to execute `command(local)` in |

### local

all local blocks are tables in the table array `block`.

`local` inherets all keys from the `Body` JSON object defined in `swaybar-protocol(7)`.

| key      | description                                                 |
|----------|-------------------------------------------------------------|
| command  | (optional) command to execute for `immediate` configuration |
| prefix   | string prefixing `full_text`                                |
| postfix  | string appended to `full_text`                              |
| interval | interval, in seconds, at which to refresh the block         |
| signal   | os signal to refresh the block when received                |

### immediate

each line of `command(local)`'s standard output is parsed in order as a field of the `Body` JSON object defined in `swaybar-protocol(7)`.

for example, suppose the following script was a block's command:

```sh
# interpreted as `full_text`
echo "amazing status information"

# interpreted as `short_text`
echo "short info"

# interpreted as `color`
echo "#ff0000"
```

## hot swapping

`smolbar` responds to `cont_signal` (see `swaybar-protocol(7)`) by reloading its configuration.

this means that by default, sending `smolbar`'s process `SIGCONT` will cause it to hot swap its config.

```toml
[header]
# cont_signal is 18 (SIGCONT) by default
```

```console
$ pkill -SIGCONT smolbar
# causes smolbar to reload config
```

`cont_signal` is also configurable.

```toml
[header]
cont_signal = 10
```

```console
$ pkill --signal 10 smolbar
# causes smolbar to reload config
```

### note

the header, fundamentally, can't be reconfigured during runtime.

this is because in `swaybar-protocol(7)`, it's only sent once, at the beginning of the status command's process.
