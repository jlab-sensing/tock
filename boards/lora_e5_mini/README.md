Platform-Specific Instructions: Seeed Studio LoRa E5 Mini
=======================================================

The [Seeed Studio LoRa E5 Mini](https://wiki.seeedstudio.com/LoRa_E5_mini/) is a
development board based around the [STM32WLE5JC](https://www.st.com/en/microcontrollers-microprocessors/stm32wle5jc.html), an SoC with an ARM Cortex-M4
and LoRa SubGhz radio. This kit contains a UART to USB-C adaptor.

## Getting Started

First, follow the [Tock Getting Started guide](../../../doc/Getting_Started.md)

OpenOCD is the preferred method to program the board. The development kit does
not have an integrated debugger. An external ST-Link is recommended. This can be
connected to the board's SWD pins for programming.

## Programming the kernel
Once you have all software installed, you should be able to simply run
`make flash` in this directory to install a fresh kernel.

Optional Cargo features can be enabled at build time and used with
`#[cfg(feature = "...")]` in the board file:

```bash
$ make flash -- --dev
# or individually:
$ make flash -- --process-console --debug-macro --halt-on-panic
# or:
$ make flash dev=1
$ make flash process-console=1 debug-macro=1 halt-on-panic=1
```

- `--dev` — development convenience superset: enables process console,
  `debug!()`, and halt-on-panic fault policy
- `--process-console` — include the process console
- `--debug-macro` — include `debug!()` support
- `--halt-on-panic` — on process fault or kernel panic, halt the system

The standard build (without these flags) defaults to removing the process
console and `debug!()` to reduce the codesize. Additionally, the size of the
kernel stack is decreased in the standard build.

To build-check every feature combination:

```bash
$ make test-features
# or:
$ ./test-feature-configs.sh
```

## Programming user-level applications
You can program an application over USB using `tockloader`:

```bash
$ cd libtock-c/examples/<app>
$ make
$ tockloader install
```

## Console output
To view the console output on the Seeed Studio LoRa E5 HF Mini:

```bash
$ tockloader listen
```

