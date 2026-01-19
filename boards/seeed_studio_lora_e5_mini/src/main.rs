// Licensed under the Apache License, Version 2.0 or the MIT License.
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Copyright Tock Contributors 2025.

//! Board file for STM32WLE5JC Seeed Studio LoRa E5 HF mini development board.
//!
//! - <https://wiki.seeedstudio.com/LoRa_E5_mini/>

#![no_std]
// Disable this attribute when documenting, as a workaround for
// https://github.com/rust-lang/rust/issues/62184.
#![cfg_attr(not(doc), no_main)]
#![deny(missing_docs)]

use core::ptr::addr_of_mut;

use capsules_core::virtualizers::virtual_alarm::VirtualMuxAlarm;
use capsules_core::virtualizers::virtual_uart::UartDevice;
use capsules_core::{gpio, led};
use capsules_extra::sdi12_ents::Sdi12Ents;
use components::gpio::GpioComponent;
use kernel::capabilities;
use kernel::component::Component;
use kernel::debug::PanicResources;
use kernel::hil::gpio::Output;
use kernel::hil::led::LedLow;
use kernel::hil::sdi12::{Receive as Sdi12Receive, Transmit as Sdi12Transmit};
use kernel::hil::time::Alarm;
use kernel::hil::time::Counter;
use kernel::hil::uart::{Receive, Transmit};
use kernel::platform::{KernelResources, SyscallDriverLookup};
use kernel::process::ProcessArray;
use kernel::scheduler::round_robin::RoundRobinSched;
use kernel::utilities::single_thread_value::SingleThreadValue;
use kernel::{create_capability, debug, static_init};
use stm32wle5jc::chip_specs::Stm32wle5jcSpecs;
use stm32wle5jc::clocks::msi::MSI_FREQUENCY_MHZ;
use stm32wle5jc::gpio::{PinId, PortId};
use stm32wle5jc::interrupt_service::Stm32wle5jcDefaultPeripherals;
use stm32wle5jc::subghz_radio::SubGhzRadioVirtualGpio;

/// Support routines for debugging I/O.
pub mod io;

#[allow(dead_code)]
mod test;

///This platform's chip type:
pub type ChipHw = stm32wle5jc::chip::Stm32wle5xx<
    'static,
    stm32wle5jc::interrupt_service::Stm32wle5jcDefaultPeripherals<'static>,
>;

// Number of concurrent processes this platform supports.
const NUM_PROCS: usize = 4;

// Actual memory for holding the active process structures.
static mut PROCESSES: Option<&'static ProcessArray<NUM_PROCS>> = None;

static mut CHIP: Option<&'static stm32wle5jc::chip::Stm32wle5xx<Stm32wle5jcDefaultPeripherals>> =
    None;

static mut PROCESS_PRINTER: Option<&'static capsules_system::process_printer::ProcessPrinterText> =
    None;

static mut SDI12_TX_BUF: [u8; 64] = [0; 64];
static mut SDI12_RX_BUF: [u8; 64] = [0; 64];
type ProcessPrinterInUse = capsules_system::process_printer::ProcessPrinterText;

/// Resources for when a board panics used by io.rs.
static PANIC_RESOURCES: SingleThreadValue<PanicResources<ChipHw, ProcessPrinterInUse>> =
    SingleThreadValue::new(PanicResources::new());

// How should the kernel respond when a process faults.
const FAULT_RESPONSE: capsules_system::process_policies::PanicFaultPolicy =
    capsules_system::process_policies::PanicFaultPolicy {};

const LORA_SPI_DRIVER_NUM: usize = capsules_core::driver::NUM::LoRaPhySPI as usize;
const LORA_GPIO_DRIVER_NUM: usize = capsules_core::driver::NUM::LoRaPhyGPIO as usize;

/// Dummy buffer that causes the linker to reserve enough space for the stack.
#[no_mangle]
#[link_section = ".stack_buffer"]
pub static mut STACK_MEMORY: [u8; 0x2000] = [0; 0x2000];

/// A structure representing this platform that holds references to all
/// capsules for this platform.
struct SeeedStudioLoraE5Mini {
    scheduler: &'static RoundRobinSched<'static>,
    systick: cortexm4::systick::SysTick,
    console: &'static capsules_core::console::Console<'static>,
    led: &'static capsules_core::led::LedDriver<
        'static,
        LedLow<'static, stm32wle5jc::gpio::Pin<'static>>,
        1,
    >,
    alarm: &'static capsules_core::alarm::AlarmDriver<
        'static,
        VirtualMuxAlarm<'static, stm32wle5jc::tim2::Tim2<'static>>,
    >,
    lora_spi_controller: &'static capsules_core::spi_controller::Spi<
        'static,
        capsules_core::virtualizers::virtual_spi::VirtualSpiMasterDevice<
            'static,
            stm32wle5jc::spi::Spi<'static>,
        >,
    >,
    lora_gpio: &'static capsules_core::gpio::GPIO<
        'static,
        stm32wle5jc::subghz_radio::SubGhzRadioVirtualGpio<'static>,
    >,
    sdi12_ents: &'static Sdi12Ents<
        'static,
        stm32wle5jc::sdi12::Sdi12<
            'static,
            stm32wle5jc::usart::Usart<'static>,
            VirtualMuxAlarm<'static, stm32wle5jc::tim2::Tim2<'static>>,
        >,
    >,
    i2c_master: &'static capsules_core::i2c_master::I2CMasterDriver<
        'static,
        stm32wle5jc::i2c::I2C<'static>,
    >,
}

/// Mapping of integer syscalls to objects that implement syscalls.
impl SyscallDriverLookup for SeeedStudioLoraE5Mini {
    fn with_driver<F, R>(&self, driver_num: usize, f: F) -> R
    where
        F: FnOnce(Option<&dyn kernel::syscall::SyscallDriver>) -> R,
    {
        match driver_num {
            capsules_core::console::DRIVER_NUM => f(Some(self.console)),
            capsules_core::led::DRIVER_NUM => f(Some(self.led)),
            capsules_core::alarm::DRIVER_NUM => f(Some(self.alarm)),
            LORA_SPI_DRIVER_NUM => f(Some(self.lora_spi_controller)),
            LORA_GPIO_DRIVER_NUM => f(Some(self.lora_gpio)),
            capsules_extra::sdi12_ents::DRIVER_NUM => f(Some(self.sdi12_ents)),
            // kernel::ipc::DRIVER_NUM => f(Some(&self.ipc)),
            // capsules_core::gpio::DRIVER_NUM => f(Some(self.gpio)),
            capsules_core::i2c_master::DRIVER_NUM => f(Some(self.i2c_master)),
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
    > for SeeedStudioLoraE5Mini
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

/// Helper function for miscellaneous peripheral functions
unsafe fn setup_peripherals(tim2: &stm32wle5jc::tim2::Tim2, subghz_spi: &stm32wle5jc::spi::Spi) {
    cortexm4::nvic::Nvic::new(stm32wle5jc::nvic::USART1).enable();
    cortexm4::nvic::Nvic::new(stm32wle5jc::nvic::USART2).enable();

    cortexm4::nvic::Nvic::new(stm32wle5jc::nvic::RADIO_IRQ).enable();
    cortexm4::nvic::Nvic::new(stm32wle5jc::nvic::SUBGHZ_SPI).enable();
    subghz_spi.enable_clock();

    cortexm4::nvic::Nvic::new(stm32wle5jc::nvic::TIM2).enable();
    tim2.enable_clock();
    tim2.start().expect("Failure starting stm32wle5jc TIM2.");

    cortexm4::nvic::Nvic::new(stm32wle5jc::nvic::I2C2_EV).enable();
    cortexm4::nvic::Nvic::new(stm32wle5jc::nvic::I2C2_ER).enable();
}

/// Statically initialize the core peripherals for the chip.
///
/// This is in a separate, inline(never) function so that its stack frame is
/// removed when this function returns. Otherwise, the stack space used for
/// these static_inits is wasted.
#[inline(never)]
unsafe fn create_peripherals() -> &'static mut Stm32wle5jcDefaultPeripherals<'static> {
    // We use the default MSI 4Mhz clock
    let rcc = static_init!(stm32wle5jc::rcc::Rcc, stm32wle5jc::rcc::Rcc::new());

    let clocks = static_init!(
        stm32wle5jc::clocks::Clocks<Stm32wle5jcSpecs>,
        stm32wle5jc::clocks::Clocks::new(rcc)
    );

    let syscfg = static_init!(
        stm32wle5jc::syscfg::Syscfg,
        stm32wle5jc::syscfg::Syscfg::new()
    );

    let exti = static_init!(
        stm32wle5jc::exti::Exti,
        stm32wle5jc::exti::Exti::new(syscfg)
    );

    let peripherals = static_init!(
        Stm32wle5jcDefaultPeripherals,
        Stm32wle5jcDefaultPeripherals::new(clocks, exti, syscfg)
    );

    peripherals
}

/// Main function
///
/// This is called after RAM initialization is complete.
#[no_mangle]
pub unsafe fn main() {
    // Initialize deferred calls very early.
    kernel::deferred_call::initialize_deferred_call_state::<
        <ChipHw as kernel::platform::chip::Chip>::ThreadIdProvider,
    >();

    stm32wle5jc::init();

    let peripherals = create_peripherals();
    peripherals.init();
    let base_peripherals = &peripherals.stm32wle;

    // Create an array to hold process references.
    let processes = components::process_array::ProcessArrayComponent::new()
        .finalize(components::process_array_component_static!(NUM_PROCS));
    PROCESSES = Some(processes);

    // Setup space to store the core kernel data structure.
    let board_kernel = static_init!(kernel::Kernel, kernel::Kernel::new(processes.as_slice()));

    let chip = static_init!(
        stm32wle5jc::chip::Stm32wle5xx<Stm32wle5jcDefaultPeripherals>,
        stm32wle5jc::chip::Stm32wle5xx::new(peripherals)
    );

    CHIP = Some(chip);

    setup_peripherals(&base_peripherals.tim2, &base_peripherals.subghz_spi);

    // Create capabilities that the board needs to call certain protected kernel
    // functions.
    let main_loop_capability = create_capability!(capabilities::MainLoopCapability);
    let process_management_capability =
        create_capability!(capabilities::ProcessManagementCapability);

    // Clock to all GPIO Ports is enabled in `set_pin_primary_functions()`
    let gpio_ports = &base_peripherals.gpio_ports;
    gpio_ports.get_port_from_port_id(PortId::B).enable_clock();
    gpio_ports.get_port_from_port_id(PortId::A).enable_clock();
    gpio_ports.get_port_from_port_id(PortId::C).enable_clock();

    //--------------------------------------------------------------------
    // Usart
    //--------------------------------------------------------------------
    base_peripherals.usart1.enable_clock();
    base_peripherals.usart2.enable_clock();

    // USART1: PB6=TX , PB7=RX
    gpio_ports.get_pin(PinId::PB06).map(|pin| {
        pin.set_mode(stm32wle5jc::gpio::Mode::AlternateFunctionMode);
        pin.set_alternate_function(stm32wle5jc::gpio::AlternateFunction::AF7);
    });

    gpio_ports.get_pin(PinId::PB07).map(|pin| {
        pin.set_mode(stm32wle5jc::gpio::Mode::AlternateFunctionMode);
        pin.set_alternate_function(stm32wle5jc::gpio::AlternateFunction::AF7);
    });

    let uart_mux_1 = components::console::UartMuxComponent::new(&base_peripherals.usart1, 115200)
        .finalize(components::uart_mux_component_static!());

    // USART2: PA2=TX , PA3=RX
    gpio_ports.get_pin(PinId::PA02).map(|pin| {
        pin.set_mode(stm32wle5jc::gpio::Mode::AlternateFunctionMode);
        pin.set_alternate_function(stm32wle5jc::gpio::AlternateFunction::AF7);
    });

    gpio_ports.get_pin(PinId::PA03).map(|pin| {
        pin.set_mode(stm32wle5jc::gpio::Mode::AlternateFunctionMode);
        pin.set_alternate_function(stm32wle5jc::gpio::AlternateFunction::AF7);
    });

    let uart_mux_2 = components::console::UartMuxComponent::new(&base_peripherals.usart2, 1200)
        .finalize(components::uart_mux_component_static!());

    (*addr_of_mut!(io::WRITER)).set_initialized();

    //--------------------------------------------------------------------
    // Alarm
    //--------------------------------------------------------------------
    let tim2 = &base_peripherals.tim2;
    let mux_alarm = components::alarm::AlarmMuxComponent::new(tim2).finalize(
        components::alarm_mux_component_static!(stm32wle5jc::tim2::Tim2),
    );

    let alarm = components::alarm::AlarmDriverComponent::new(
        board_kernel,
        capsules_core::alarm::DRIVER_NUM,
        mux_alarm,
    )
    .finalize(components::alarm_component_static!(stm32wle5jc::tim2::Tim2));

    //--------------------------------------------------------------------
    // Console.
    //--------------------------------------------------------------------
    let console = components::console::ConsoleComponent::new(
        board_kernel,
        capsules_core::console::DRIVER_NUM,
        uart_mux_1,
    )
    .finalize(components::console_component_static!());

    let uart_device = static_init!(
        capsules_core::virtualizers::virtual_uart::UartDevice<'static>,
        capsules_core::virtualizers::virtual_uart::UartDevice::new(&uart_mux_2, true)
    );
    uart_device.setup();

    // Create the debugger object that handles calls to `debug!()`.
    components::debug_writer::DebugWriterComponent::new::<
        <ChipHw as kernel::platform::chip::Chip>::ThreadIdProvider,
    >(
        uart_mux_1,
        create_capability!(capabilities::SetDebugWriterCapability),
    )
    .finalize(components::debug_writer_component_static!());

    let process_printer = components::process_printer::ProcessPrinterTextComponent::new()
        .finalize(components::process_printer_text_component_static!());
    PROCESS_PRINTER = Some(process_printer);

    //--------------------------------------------------------------------
    // LED
    //--------------------------------------------------------------------
    let led = components::led::LedsComponent::new().finalize(components::led_component_static!(
        LedLow<'static, stm32wle5jc::gpio::Pin>,
        LedLow::new(gpio_ports.get_pin(stm32wle5jc::gpio::PinId::PB05).unwrap()),
    ));

    //--------------------------------------------------------------------
    // SPI
    //--------------------------------------------------------------------
    let chip_select =
        kernel::hil::spi::cs::IntoChipSelect::<_, kernel::hil::spi::cs::ActiveLow>::into_cs(
            gpio_ports.get_pin(stm32wle5jc::gpio::PinId::PB08).unwrap(),
        );

    base_peripherals.subghz_spi.set_nss(&base_peripherals.pwr);

    let lora_spi_mux = components::spi::SpiMuxComponent::new(&base_peripherals.subghz_spi)
        .finalize(components::spi_mux_component_static!(
            stm32wle5jc::spi::Spi<'static>
        ));

    let lora_spi_controller = components::spi::SpiSyscallComponent::new(
        board_kernel,
        lora_spi_mux,
        chip_select,
        LORA_SPI_DRIVER_NUM,
    )
    .finalize(components::spi_syscall_component_static!(
        stm32wle5jc::spi::Spi<'static>
    ));

    //--------------------------------------------------------------------
    // LoRa
    //--------------------------------------------------------------------
    // reset lora module
    base_peripherals.clocks.reset_subghzradio();

    let lora_interrupt_pin = static_init!(
        stm32wle5jc::subghz_radio::SubGhzRadioVirtualGpio,
        stm32wle5jc::subghz_radio::SubGhzRadioVirtualGpio::new(
            &base_peripherals.subghz_radio_interrupt
        )
    );

    let lora_busy_base = static_init!(
        stm32wle5jc::subghz_radio::SubGhzRadioBusy,
        stm32wle5jc::subghz_radio::SubGhzRadioBusy::new(&base_peripherals.pwr)
    );
    let lora_busy_pin = static_init!(
        stm32wle5jc::subghz_radio::SubGhzRadioVirtualGpio,
        stm32wle5jc::subghz_radio::SubGhzRadioVirtualGpio::new(lora_busy_base)
    );

    let lora_gpio = components::gpio::GpioComponent::new(
        board_kernel,
        LORA_GPIO_DRIVER_NUM,
        components::gpio_component_helper!(
            SubGhzRadioVirtualGpio<'static>,
            1 => lora_busy_pin,
            2 => lora_interrupt_pin,
        ),
    )
    .finalize(components::gpio_component_static!(
        stm32wle5jc::subghz_radio::SubGhzRadioVirtualGpio
    ));

    //--------------------------------------------------------------------
    // I2C2
    //--------------------------------------------------------------------
    gpio_ports.get_pin(PinId::PA15).map(|pin| {
        pin.set_mode(stm32wle5jc::gpio::Mode::AlternateFunctionMode);
        pin.set_alternate_function(stm32wle5jc::gpio::AlternateFunction::AF4);
    });

    gpio_ports.get_pin(PinId::PB15).map(|pin| {
        pin.set_mode(stm32wle5jc::gpio::Mode::AlternateFunctionMode);
        pin.set_alternate_function(stm32wle5jc::gpio::AlternateFunction::AF4);
    });

    base_peripherals.i2c2.enable_clock();
    let i2c_master = components::i2c::I2CMasterDriverComponent::new(
        board_kernel,
        capsules_core::i2c_master::DRIVER_NUM,
        &base_peripherals.i2c2,
    )
    .finalize(components::i2c_master_component_static!(
        stm32wle5jc::i2c::I2C
    ));

    // temporary code for setting GPIO powerdown pin for board peripherals
    gpio_ports.get_pin(PinId::PA09).map(|pin| {
        pin.set_mode(stm32wle5jc::gpio::Mode::GeneralPurposeOutputMode);
        pin.set();
    });

    // Uncomment to run I2C scan test
    // test::i2c_dummy::i2c_scan_slaves(&base_peripherals.i2c2);

    //--------------------------------------------------------------------
    // PROCESS CONSOLE
    //--------------------------------------------------------------------
    let process_console = components::process_console::ProcessConsoleComponent::new(
        board_kernel,
        uart_mux_1,
        mux_alarm,
        process_printer,
        Some(cortexm4::support::reset),
    )
    .finalize(components::process_console_component_static!(
        stm32wle5jc::tim2::Tim2
    ));
    let _ = process_console.start();

    let scheduler = components::sched::round_robin::RoundRobinComponent::new(processes)
        .finalize(components::round_robin_component_static!(NUM_PROCS));

    let sdi12_command_pin = gpio_ports.get_pin(PinId::PC01).unwrap();
    let virtual_alarm = static_init!(
        VirtualMuxAlarm<'static, stm32wle5jc::tim2::Tim2<'static>>,
        VirtualMuxAlarm::new(mux_alarm)
    );
    virtual_alarm.setup();

    let sdi12_usart_pin = gpio_ports.get_pin(PinId::PA02).unwrap();

    let sdi12_driver = static_init!(
        stm32wle5jc::sdi12::Sdi12<
            'static,
            stm32wle5jc::usart::Usart<'static>,
            VirtualMuxAlarm<'static, stm32wle5jc::tim2::Tim2<'static>>,
        >,
        stm32wle5jc::sdi12::Sdi12::new(
            &base_peripherals.usart2,
            sdi12_usart_pin,
            sdi12_command_pin,
            virtual_alarm,
        )
    );
    //TODO: figure out why there needs to be a uart_device for the uart to transmit
    // but the peripheral needs to be set to the transmit client for that to work
    virtual_alarm.set_alarm_client(sdi12_driver);
    //uart_device.set_transmit_client(sdi12_driver);
    base_peripherals.usart2.set_transmit_client(sdi12_driver);
    base_peripherals.usart2.set_receive_client(sdi12_driver);

    let sdi12_grant_cap = create_capability!(capabilities::MemoryAllocationCapability);
    let sdi12_driver_process_grant =
        board_kernel.create_grant(capsules_extra::sdi12_ents::DRIVER_NUM, &sdi12_grant_cap);

    let sdi12_ents = static_init!(
        Sdi12Ents<
            'static,
            stm32wle5jc::sdi12::Sdi12<
                'static,
                stm32wle5jc::usart::Usart<'static>,
                VirtualMuxAlarm<'static, stm32wle5jc::tim2::Tim2<'static>>,
            >,
        >,
        capsules_extra::sdi12_ents::Sdi12Ents::new(
            &mut SDI12_TX_BUF,
            &mut SDI12_RX_BUF,
            sdi12_driver,
            sdi12_driver_process_grant
        ),
    );
    sdi12_driver.set_transmit_client(sdi12_ents);
    sdi12_driver.set_receive_client(sdi12_ents);

    let seeed_studio_lora_e5_mini = SeeedStudioLoraE5Mini {
        scheduler,
        systick: cortexm4::systick::SysTick::new_with_calibration(
            (MSI_FREQUENCY_MHZ * 1_000_000) as u32,
        ),
        console,
        led,
        alarm,
        lora_spi_controller,
        lora_gpio,
        sdi12_ents,
        i2c_master,
    };

    assert!(base_peripherals.subghz_spi.is_enabled_clock());
    debug!("Initialization complete. Entering main loop...");
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
        &seeed_studio_lora_e5_mini,
        chip,
        None::<&kernel::ipc::IPC<2>>,
        &main_loop_capability,
    );
}
