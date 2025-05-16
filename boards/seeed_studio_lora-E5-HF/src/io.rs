// Licensed under the Apache License, Version 2.0 or the MIT License.
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Copyright Tock Contributors 2022.

use core::fmt::Write;
use core::panic::PanicInfo;
use core::ptr::addr_of;
use core::ptr::addr_of_mut;

use capsules_core::gpio;
use kernel::debug;
use kernel::debug::panic_blink_forever;
use kernel::debug::IoWrite;
use kernel::hil::led;

use kernel::hil::led::Led;
use stm32wle5jc::chip_specs::Stm32wle5jcSpecs;
use stm32wle5jc::gpio::PinId;

use crate::CHIP;
use crate::PROCESSES;
use crate::PROCESS_PRINTER;

use kernel::hil::gpio::Pin;

/// Writer is used by kernel::debug to panic message to the serial port.
pub struct Writer {
    initialized: bool,
}

/// Global static for debug writer
pub static mut WRITER: Writer = Writer { initialized: false };

impl Writer {
    /// Indicate that USART has already been initialized. Trying to double
    /// initialize USART1 causes stm32f429zi to go into in in-deterministic state.
    pub fn set_initialized(&mut self) {
        self.initialized = true;
    }
}

impl Write for Writer {
    fn write_str(&mut self, s: &str) -> ::core::fmt::Result {
        self.write(s.as_bytes());
        Ok(())
    }
}

impl IoWrite for Writer {
    fn write(&mut self, buf: &[u8]) -> usize {
        unimplemented!()
    }
}

/// Panic handler.
#[no_mangle]
#[panic_handler]
pub unsafe fn panic_fmt(info: &PanicInfo) -> ! {
    // For now we add a loop to blink the LED to an interesting way.
    // To ensure that all dependencies are set up, we initialize all clocks
    // and GPIOs here in this function.
    //
    // Relying on `main.rs` to initialize clocks/gpios may result in the gpio
    // not being properly configured if the panic occurs early in `main.rs`.
    let rcc = stm32wle5jc::rcc::Rcc::new();
    let clocks: stm32wle5jc::clocks::Clocks<Stm32wle5jcSpecs> =
        stm32wle5jc::clocks::Clocks::new(&rcc);
    let gpio_ports = stm32wle5jc::gpio::GpioPorts::new(&clocks);
    gpio_ports.setup_circular_deps();
    gpio_ports
        .get_port_from_port_id(stm32wle5jc::gpio::PortId::B)
        .enable_clock();
    let pin = stm32wle5jc::gpio::Pin::new(PinId::PB05);
    pin.set_ports_ref(&gpio_ports);
    let led = &mut led::LedLow::new(&pin);
    led.init();

    // Unique LED blink pattern for panic
    loop {
        led.on();
        // Wait for LONG
        delay_long();

        led.off();
        delay_short();

        // SHORT
        led.on();
        delay_short();

        led.off();
        delay_short();

        // SHORT
        led.on();
        delay_short();

        led.off();
        delay_short();

        // LONG
        led.on();
        delay_long();

        led.off();
        delay_long();
    }
}

fn delay_long() {
    for _ in 0..1_000_000_ {
        cortexm4::support::nop();
    }
}

fn delay_short() {
    for _ in 0..100_000 {
        cortexm4::support::nop();
    }
}
