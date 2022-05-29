# smolbar

**warning: not done**

`smolbar` is a smol status command for [sway](https://github.com/swaywm/sway).

## milestones

- [X] refresh configurable blocks on signals/intervals
- [ ] respond to stop and continue signals
- [ ] support click events

## configuration

there are three scopes which can be used to configure individual blocks.

### global

| key         | description                                  |
|-------------|----------------------------------------------|
| command_dir | sets the path to execute `command(local)` in |

### local

`local` inherets all keys from the `Body` JSON object defined in `swaybar-protocol(5)`.

| key      | description                                        |
|----------|----------------------------------------------------|
| command  | command to execute for `immediate` configuration   |
| prefix   | string prefixing `full_text(immediate)`            |
| postfix  | string appended to `full_text(immediate)`          |
| interval | repeated interval in seconds to refresh this block |
| signal   | os signal when received to refresh this block      |

### immediate

each line of `command(local)`'s standard output is parsed in order as a field of the `Body` JSON object defined in `swaybar-protocol(5)`.

#### example

```sh
# interpreted as `full_text`
echo "amazing status information"

# interpreted as `short_text`
echo "short info"

# interpreted as `color`
echo "#ff0000"
```
