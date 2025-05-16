// Licensed under the Apache License, Version 2.0 or the MIT License.
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Copyright Tock Contributors 2022.

//! Board file for STM32F429I Discovery development board
//!
//! - <https://www.st.com/en/evaluation-tools/32f429idiscovery.html>

#![no_std]
// Disable this attribute when documenting, as a workaround for
// https://github.com/rust-lang/rust/issues/62184.
#![cfg_attr(not(doc), no_main)]
#![deny(missing_docs)]

use core::ptr::{addr_of, addr_of_mut, write_volatile};

use capsules_core::{gpio, led};
use components::gpio::GpioComponent;
use kernel::capabilities;
use kernel::component::Component;
use kernel::hil::gpio::Configure;
use kernel::hil::led::{Led, LedHigh, LedLow};
use kernel::platform::{KernelResources, SyscallDriverLookup};
use kernel::scheduler::round_robin::RoundRobinSched;
use kernel::{create_capability, debug, static_init};
use stm32wle5jc::chip_specs::Stm32wle5jcSpecs;
use stm32wle5jc::gpio::{PinId, PortId};
use stm32wle5jc::interrupt_service::Stm32wle5jcDefaultPeripherals;

/// Support routines for debugging I/O.
pub mod io;

// Number of concurrent processes this platform supports.
const NUM_PROCS: usize = 4;

// Actual memory for holding the active process structures.
static mut PROCESSES: [Option<&'static dyn kernel::process::Process>; NUM_PROCS] =
    [None, None, None, None];

static mut CHIP: Option<&'static stm32wle5jc::chip::Stm32wle5xx<Stm32wle5jcDefaultPeripherals>> =
    None;

static mut PROCESS_PRINTER: Option<&'static capsules_system::process_printer::ProcessPrinterText> =
    None;

// How should the kernel respond when a process faults.
const FAULT_RESPONSE: capsules_system::process_policies::PanicFaultPolicy =
    capsules_system::process_policies::PanicFaultPolicy {};

/// Dummy buffer that causes the linker to reserve enough space for the stack.
#[no_mangle]
#[link_section = ".stack_buffer"]
pub static mut STACK_MEMORY: [u8; 0x2000] = [0; 0x2000];

/// A structure representing this platform that holds references to all
/// capsules for this platform.
struct SeeedStudioLoraE5Hf {
    //led: &'static capsules_core::led::LedDriver<
    //    'static,
    //    LedHigh<'static, stm32wle5jc::gpio::Pin<'static>>,
    //    1,
    //>,
    // gpio: &'static capsules_core::gpio::GPIO<'static, stm32f429zi::gpio::Pin<'static>>,
    scheduler: &'static RoundRobinSched<'static>,
    systick: cortexm4::systick::SysTick,
}

/// Mapping of integer syscalls to objects that implement syscalls.
impl SyscallDriverLookup for SeeedStudioLoraE5Hf {
    fn with_driver<F, R>(&self, driver_num: usize, f: F) -> R
    where
        F: FnOnce(Option<&dyn kernel::syscall::SyscallDriver>) -> R,
    {
        match driver_num {
            // capsules_core::console::DRIVER_NUM => f(Some(self.console)),
            //capsules_core::led::DRIVER_NUM => f(Some(self.led)),
            // capsules_core::alarm::DRIVER_NUM => f(Some(self.alarm)),
            // kernel::ipc::DRIVER_NUM => f(Some(&self.ipc)),
            // capsules_core::gpio::DRIVER_NUM => f(Some(self.gpio)),
            _ => f(None),
        }
    }
}

impl
    KernelResources<
        stm32wle5jc::chip::Stm32wle5xx<
            'static,
            stm32wle5jc::interrupt_service::Stm32wle5jcDefaultPeripherals<'static>,
        >,
    > for SeeedStudioLoraE5Hf
{
    type SyscallDriverLookup = Self;
    type SyscallFilter = ();
    type ProcessFault = ();
    type Scheduler = RoundRobinSched<'static>;
    type SchedulerTimer = cortexm4::systick::SysTick;
    type WatchDog = ();
    type ContextSwitchCallback = ();

    fn syscall_driver_lookup(&self) -> &Self::SyscallDriverLookup {
        self
    }
    fn syscall_filter(&self) -> &Self::SyscallFilter {
        &()
    }
    fn process_fault(&self) -> &Self::ProcessFault {
        &()
    }
    fn scheduler(&self) -> &Self::Scheduler {
        self.scheduler
    }
    fn scheduler_timer(&self) -> &Self::SchedulerTimer {
        &self.systick
    }
    fn watchdog(&self) -> &Self::WatchDog {
        &()
    }
    fn context_switch_callback(&self) -> &Self::ContextSwitchCallback {
        &()
    }
}

/// Helper function called during bring-up that configures DMA.
// ENTS TODO: Is this needed fo W series? Unused currently.
/*
unsafe fn setup_dma(
    dma: &stm32f429zi::dma::Dma2,
    dma_streams: &'static [stm32f429zi::dma::Stream<'static, stm32f429zi::dma::Dma2>; 8],
    usart1: &'static stm32f429zi::usart::Usart<stm32f429zi::dma::Dma2>,
) {
    use stm32f429zi::dma::Dma2Peripheral;
    use stm32f429zi::usart;

    dma.enable_clock();

    let usart1_tx_stream = &dma_streams[Dma2Peripheral::USART1_TX.get_stream_idx()];
    let usart1_rx_stream = &dma_streams[Dma2Peripheral::USART1_RX.get_stream_idx()];

    usart1.set_dma(
        usart::TxDMA(usart1_tx_stream),
        usart::RxDMA(usart1_rx_stream),
    );

    usart1_tx_stream.set_client(usart1);
    usart1_rx_stream.set_client(usart1);

    usart1_tx_stream.setup(Dma2Peripheral::USART1_TX);
    usart1_rx_stream.setup(Dma2Peripheral::USART1_RX);

    cortexm4::nvic::Nvic::new(Dma2Peripheral::USART1_TX.get_stream_irqn()).enable();
    cortexm4::nvic::Nvic::new(Dma2Peripheral::USART1_RX.get_stream_irqn()).enable();
} */

/*
/// Helper function called during bring-up that configures multiplexed I/O.
unsafe fn set_pin_primary_functions(
    syscfg: &stm32f429zi::syscfg::Syscfg,
    gpio_ports: &'static stm32f429zi::gpio::GpioPorts<'static>,
) {
    use kernel::hil::gpio::Configure;

    syscfg.enable_clock();

    gpio_ports.get_port_from_port_id(PortId::G).enable_clock();

    // User LD4 (red) is connected to PG14. Configure PG14 as `debug_gpio!(0, ...)`
    gpio_ports.get_pin(PinId::PG14).map(|pin| {
        pin.make_output();

        // Configure kernel debug gpios as early as possible
        kernel::debug::assign_gpios(Some(pin), None, None);
    });

    gpio_ports.get_port_from_port_id(PortId::A).enable_clock();

    // Configure USART1 on Pins PA09 and PA10.
    // USART1 is connected to ST-LINK virtual COM port on Rev.1 of the Stm32f429i Discovery board
    gpio_ports.get_pin(PinId::PA09).map(|pin| {
        pin.set_mode(Mode::AlternateFunctionMode);
        // AF7 is USART1_TX
        pin.set_alternate_function(AlternateFunction::AF7);
    });
    gpio_ports.get_pin(PinId::PA10).map(|pin| {
        pin.set_mode(Mode::AlternateFunctionMode);
        // AF7 is USART1_RX
        pin.set_alternate_function(AlternateFunction::AF7);
    });

    // User button B1 is connected on pa00
    gpio_ports.get_pin(PinId::PA00).map(|pin| {
        // By default, upon reset, the pin is in input mode, with no internal
        // pull-up, no internal pull-down (i.e., floating).
        //
        // Only set the mapping between EXTI line and the Pin and let capsule do
        // the rest.
        pin.enable_interrupt();
    });
    // EXTI0 interrupts is delivered at IRQn 6 (EXTI0)
    cortexm4::nvic::Nvic::new(stm32f429zi::nvic::EXTI0).enable(); // TODO check if this is still necessary!

    // Enable clocks for GPIO Ports
    // Disable some of them if you don't need some of the GPIOs
    // Ports A, and B are already enabled
    //           A: already enabled
    gpio_ports.get_port_from_port_id(PortId::B).enable_clock();
    gpio_ports.get_port_from_port_id(PortId::C).enable_clock();
    gpio_ports.get_port_from_port_id(PortId::D).enable_clock();
    gpio_ports.get_port_from_port_id(PortId::E).enable_clock();
    gpio_ports.get_port_from_port_id(PortId::F).enable_clock();
    //           G: already enabled
    gpio_ports.get_port_from_port_id(PortId::H).enable_clock();

    // Arduino A0
    gpio_ports.get_pin(PinId::PA03).map(|pin| {
        pin.set_mode(stm32f429zi::gpio::Mode::AnalogMode);
    });

    // Arduino A1
    gpio_ports.get_pin(PinId::PC00).map(|pin| {
        pin.set_mode(stm32f429zi::gpio::Mode::AnalogMode);
    });

    // Arduino A2
    gpio_ports.get_pin(PinId::PC03).map(|pin| {
        pin.set_mode(stm32f429zi::gpio::Mode::AnalogMode);
    });

    // Arduino A3
    gpio_ports.get_pin(PinId::PF03).map(|pin| {
        pin.set_mode(stm32f429zi::gpio::Mode::AnalogMode);
    });

    // Arduino A4
    gpio_ports.get_pin(PinId::PF05).map(|pin| {
        pin.set_mode(stm32f429zi::gpio::Mode::AnalogMode);
    });

    // Arduino A5
    gpio_ports.get_pin(PinId::PF10).map(|pin| {
        pin.set_mode(stm32f429zi::gpio::Mode::AnalogMode);
    });
}
*/

/// Helper function for miscellaneous peripheral functions
// unsafe fn setup_peripherals(tim2: &stm32f429zi::tim2::Tim2) {
//     // USART1 IRQn is 37
//     cortexm4::nvic::Nvic::new(stm32f429zi::nvic::USART1).enable();
//
//     // TIM2 IRQn is 28
//     tim2.enable_clock();
//     tim2.start();
//     cortexm4::nvic::Nvic::new(stm32f429zi::nvic::TIM2).enable();
// }

/// Statically initialize the core peripherals for the chip.
///
/// This is in a separate, inline(never) function so that its stack frame is
/// removed when this function returns. Otherwise, the stack space used for
/// these static_inits is wasted.
#[inline(never)]
unsafe fn create_peripherals() -> &'static mut Stm32wle5jcDefaultPeripherals<'static> {
    // We use the default HSI 16Mhz clock
    let rcc = static_init!(stm32wle5jc::rcc::Rcc, stm32wle5jc::rcc::Rcc::new());
    let clocks = static_init!(
        stm32wle5jc::clocks::Clocks<Stm32wle5jcSpecs>,
        stm32wle5jc::clocks::Clocks::new(rcc)
    );

    let peripherals = static_init!(
        Stm32wle5jcDefaultPeripherals,
        Stm32wle5jcDefaultPeripherals::new(clocks)
    );

    peripherals
}

/// Main function
///
/// This is called after RAM initialization is complete.
#[no_mangle]
pub unsafe fn main() {
    stm32wle5jc::init();

    let peripherals = create_peripherals();
    peripherals.init();
    let base_peripherals = &peripherals.stm32wle;

    let board_kernel = static_init!(kernel::Kernel, kernel::Kernel::new(&*addr_of!(PROCESSES)));

    let chip = static_init!(
        stm32wle5jc::chip::Stm32wle5xx<Stm32wle5jcDefaultPeripherals>,
        stm32wle5jc::chip::Stm32wle5xx::new(peripherals)
    );
    CHIP = Some(chip);

    // Create capabilities that the board needs to call certain protected kernel
    // functions.
    let memory_allocation_capability = create_capability!(capabilities::MemoryAllocationCapability);
    let main_loop_capability = create_capability!(capabilities::MainLoopCapability);
    let process_management_capability =
        create_capability!(capabilities::ProcessManagementCapability);

    // Clock to all GPIO Ports is enabled in `set_pin_primary_functions()`
    let gpio_ports = &base_peripherals.gpio_ports;
    gpio_ports.get_port_from_port_id(PortId::B).enable_clock();
    gpio_ports.get_port_from_port_id(PortId::A).enable_clock();

    let scheduler = components::sched::round_robin::RoundRobinComponent::new(&*addr_of!(PROCESSES))
        .finalize(components::round_robin_component_static!(NUM_PROCS));

    let seeed_studio_lora_e5_hf = SeeedStudioLoraE5Hf {
        scheduler,
        systick: cortexm4::systick::SysTick::new(),
    };

    // debug!("Initialization complete. Entering main loop");
    // These symbols are defined in the linker script.
    extern "C" {
        /// Beginning of the ROM region containing app images.
        static _sapps: u8;
        /// End of the ROM region containing app images.
        static _eapps: u8;
        /// Beginning of the RAM region for app memory.
        static mut _sappmem: u8;
        /// End of the RAM region for app memory.
        static _eappmem: u8;
    }

    kernel::process::load_processes(
        board_kernel,
        chip,
        core::slice::from_raw_parts(
            core::ptr::addr_of!(_sapps),
            core::ptr::addr_of!(_eapps) as usize - core::ptr::addr_of!(_sapps) as usize,
        ),
        core::slice::from_raw_parts_mut(
            core::ptr::addr_of_mut!(_sappmem),
            core::ptr::addr_of!(_eappmem) as usize - core::ptr::addr_of!(_sappmem) as usize,
        ),
        &mut *addr_of_mut!(PROCESSES),
        &FAULT_RESPONSE,
        &process_management_capability,
    )
    .unwrap_or_else(|err| {
        debug!("Error loading processes!");
        debug!("{:?}", err);
    });

    //Uncomment to run multi alarm test
    /*components::test::multi_alarm_test::MultiAlarmTestComponent::new(mux_alarm)
    .finalize(components::multi_alarm_test_component_buf!(stm32f429zi::tim2::Tim2))
    .run();*/

    board_kernel.kernel_loop(
        &seeed_studio_lora_e5_hf,
        chip,
        None::<&kernel::ipc::IPC<2>>,
        &main_loop_capability,
    );
}
