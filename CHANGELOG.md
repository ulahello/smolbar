# changelog

## [unreleased]
* log time instead of uptime

## [0.5.0] - 2022-08-08
* switched to tracing for logging
* fixed extremely unlikely invalid state

## [0.4.2] - 2022-08-02
* feat: added source of log to logs
* feat: log which block requests a refresh
* fixed several potential panics

## [0.4.1] - 2022-07-21
* fix: improved interval precision
  * uses deadline instead of duration, so time won't drift
* fixed zero intervals freezing up program

## [0.4.0] - 2022-07-14
* feat: make command optional
* feat: refer to blocks with IDs instead of by command

## [0.3.3] - 2022-07-13
* fixed block not updating if command fails
* changed command_dir log level to info

## [0.3.2] - 2022-06-28
* fixed missing error message for fatal errors
* feat: log command_dir
* improved `--config` help message

## [0.3.1] - 2022-06-28
* fixed incorrect command_dir
  * previously the configuration path wasn't canonicalized
* fixed trace log from dependency showing on program end
* fixed fatal errors not having timestamps

## [0.3.0] - 2022-06-08
* added support for floating point intervals

## [0.2.0] - 2022-06-08
* added crate documentation
* added timestamps to logs
* fixed slow shutdown with slow command
* fixed incorrect documentation in README

## [0.1.1] - 2022-06-04
* fixed panic when receiving continue signal and stop signal simultaneously
* improved portability of exit code

## [0.1.0] - 2022-06-02
* feat: refresh configurable blocks on signals/intervals
* feat: add `--config` & `--license` cli flags
