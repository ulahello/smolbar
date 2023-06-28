# changelog

## [unreleased]
### added
* added man pages `smolbar(1)` and `smolbar(5)`
* added warning if header version is invalid

### changed
* overhauled [README](./README.md)
* replaced `termcolor` dependency with `nu-ansi-term`
  * `nu-ansi-term` is already in the dependency tree (from `tracing-subscriber`) and is simpler for smolbar's use case

## [0.8.1] - 2023-06-20
### added
* added security considerations to [README](./README.md)

### fixed
* added `tokio-util`, `signal-hook-registry`, and `semver` to `smolbar --license`
* fix prefix and postfix not appearing if full_text is not defined

### changed
* avoid sending the same sequence of blocks multiple times
* updated dependencies
* removed `dirs` dependency

## [0.8.0] - 2023-04-22
### added
* added global configuration key `smolbar_version` to specify version requirement for `smolbar`

### fixed
* fixed incorrect documentation about location of config file

### changed
* **BREAKING:** changed serialization of signals to their actual name (like "SIGSTOP", "SIGCONT")
  * this makes configurations more portable across architectures where the same signal may have a different value
  * the following signals are currently supported:
    * `SIGALRM`
    * `SIGCHLD`
    * `SIGCONT`
    * `SIGHUP`
    * `SIGINT`
    * `SIGIO`
    * `SIGPIPE`
    * `SIGQUIT`
    * `SIGSTOP`
    * `SIGTERM`
    * `SIGUSR1`
    * `SIGUSR2`
    * `SIGWINCH`
* rewrote most of codebase
  * added `tokio-util` dependency
* updated dependencies

## [0.7.3] - 2023-03-23
### added
* added license information for direct dependencies to `--license` flag

### fixed
* bring back `--version` flag

### changed
* updated dependencies

## [0.7.2] - 2023-03-20
### changed
* tighten `toml` feature flags
* replaced `clap` dependency with `argh`
  * we don't use many clap features so a less featured argument parser is fitting
* updated dependencies

## [0.7.1] - 2023-02-01
### changed
* will now pre-allocate memory for config file based on its metadata
* updated dependencies
* bumped msrv to 1.66.1

## [0.7.0] - 2023-01-25
### added
* added `--terse` flag to decrease log level

### fixed
* no longer using `tracing` for logging for fatal error messages

### changed
* updated dependencies

## [0.6.1] - 2022-12-18
### changed
* use `anyhow` for error handling
* warns whenever command exits with nonzero exit code
* bumped MSRV to 1.66.0 stable
* updated dependencies

## [0.6.0] - 2022-11-01
### changed
* **BREAKING:** deny unused fields in config

### fixed
* fixed `--help` not showing author and version

## [0.5.5] - 2022-10-18
### changed
* changed to only have bin (no bin + lib)
  * not considering this breaking because this is intended to be used as a binary

### fixed
* mistakes in [README](./README.md)

## [0.5.4] - 2022-10-16
### changed
* updated dependencies
* specified dependencies more precisely in `Cargo.toml`
* specified rust-version as `1.60`

## [0.5.3] - 2022-10-01
### changed
* updated to `clap` v4

### fixed
* fixed shutdown hang when `stop_signal` is valid but `cont_signal` isn't

## [0.5.2] - 2022-09-23
### added
* added more examples and explanation to [README](./README.md)

### fixed
* blank config file is now valid
  * config no longer requires `header` table or `block` table array to be explicitly defined
* fixed bug where block wouldn't update unless `local::command` was defined

## [0.5.1] - 2022-09-21
* published to crates.io

### changed
* log time instead of uptime

### fixed
* restructured finicky shutdown code
  * if there were any strange and rare bugs, they were there, and they are now fixed

## [0.5.0] - 2022-08-08
### changed
* switched to tracing for logging

### fixed
* fixed extremely unlikely invalid state

## [0.4.2] - 2022-08-02
### added
* feat: added source of log to logs
* feat: log which block requests a refresh

### fixed
* fixed several potential panics

## [0.4.1] - 2022-07-21
### fixed
* fix: improved interval precision
  * uses deadline instead of duration, so time won't drift
* fixed zero intervals freezing up program

## [0.4.0] - 2022-07-14
### added
* feat: make command optional
* feat: refer to blocks with IDs instead of by command

## [0.3.3] - 2022-07-13
### changed
* changed command_dir log level to info

### fixed
* fixed block not updating if command fails

## [0.3.2] - 2022-06-28
### added
* feat: log command_dir

### changed
* improved `--config` help message

### fixed
* fixed missing error message for fatal errors

## [0.3.1] - 2022-06-28
### fixed
* fixed incorrect command_dir
  * previously the configuration path wasn't canonicalized
* fixed trace log from dependency showing on program end
* fixed fatal errors not having timestamps

## [0.3.0] - 2022-06-08
### added
* added support for floating point intervals

## [0.2.0] - 2022-06-08
### added
* added crate documentation
* added timestamps to logs

### fixed
* fixed slow shutdown with slow command
* fixed incorrect documentation in [README](./README.md)

## [0.1.1] - 2022-06-04
### changed
* improved portability of exit code

### fixed
* fixed panic when receiving continue signal and stop signal simultaneously

## [0.1.0] - 2022-06-02
### added
* feat: refresh configurable blocks on signals/intervals
* feat: add `--config` & `--license` cli flags
