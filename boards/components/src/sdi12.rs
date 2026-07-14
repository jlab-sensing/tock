// Licensed under the Apache License, Version 2.0 or the MIT License.
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Copyright Tock Contributors 2025.

//! Components for SDI-12 environmental sensor interface.
//!
//! SDI-12 uses UART as the bus protocol in conjunction with a command pin GPIO.
//! This component provides a simplified interface to set up an SDI-12 peripheral
//! using a multiplexed UART connection.
//!
//! Usage
//! -----
//! ```rust
//! let uart_mux = UartMuxComponent::new(&sam4l::usart::USART3,
//!                                      1200,
//!                                      deferred_caller).finalize(components::uart_mux_component_static!());
//! let sdi12 = Sdi12Component::new(uart_mux)
//!    .finalize(sdi12_component_static!());
//! ```

use capsules_core::virtualizers::virtual_uart::{MuxUart, UartDevice};
use capsules_extra::sdi12_ents::Sdi12Ents;
use core::mem::MaybeUninit;
use kernel::capabilities;
use kernel::component::Component;
use kernel::create_capability;
use kernel::hil;

#[macro_export]
macro_rules! sdi12_component_static {
    // Common logic for buffer allocation
    ($rx_buffer_len: expr, $tx_buffer_len: expr) => {{
        use capsules_core::virtualizers::virtual_uart::UartDevice;
        use kernel::static_buf;
        let read_buf = static_buf!([u8; $rx_buffer_len]);
        let write_buf = static_buf!([u8; $tx_buffer_len]);
        // Create virtual device for SDI-12.
        let sdi12_uart = static_buf!(UartDevice);
        (write_buf, read_buf, sdi12_uart)
    }};
    () => {
        $crate::sdi12_component_static!(64, 64);
    };
    ($rx_buffer_len: literal, $tx_buffer_len: literal) => {
        $crate::sdi12_component_static!($rx_buffer_len, $tx_buffer_len);
    };
}

#[macro_export]
macro_rules! sdi12_ents_static {
    ($S:ty $(,)?) => {{
        use capsules_extra::sdi12_ents::Sdi12Ents;
        let _tx_buf = kernel::static_init!([u8; 64], [0; 64]);
        let _rx_buf = kernel::static_init!([u8; 64], [0; 64]);
        let sdi12_ents = kernel::static_init!(
            core::mem::MaybeUninit<Sdi12Ents<'static, $S>>,
            core::mem::MaybeUninit::uninit()
        );
        (sdi12_ents, _tx_buf, _rx_buf)
    }};
}

/// Helper function to initialize SDI-12 userspace driver.
///
/// This function handles the full SDI-12 driver setup including buffer allocation,
/// grant allocation, and userspace driver interface setup.
/// 
/// Note: The caller must ensure the returned pointer is stored in static memory
/// and lives for the entire program lifetime.
pub fn init_sdi12_ents<'a, S>(
    board_kernel: &'static kernel::Kernel,
    driver_num: usize,
    sdi12_driver: &'a S,
    (sdi12_ents, tx_buffer, rx_buffer): (
        &'static mut MaybeUninit<Sdi12Ents<'a, S>>,
        &'static mut [u8; 64],
        &'static mut [u8; 64],
    ),
) -> &'static Sdi12Ents<'a, S>
where
    S: hil::sdi12::Transmit<'a> + hil::sdi12::Receive<'a>,
{
    let sdi12_grant_cap = create_capability!(capabilities::MemoryAllocationCapability);
    let sdi12_driver_process_grant = board_kernel.create_grant(driver_num, &sdi12_grant_cap);

    let sdi12_ents_ref = sdi12_ents.write(Sdi12Ents::new(
        tx_buffer,
        rx_buffer,
        sdi12_driver,
        sdi12_driver_process_grant,
    ));

    sdi12_driver.set_transmit_client(sdi12_ents_ref);
    sdi12_driver.set_receive_client(sdi12_ents_ref);

    sdi12_ents_ref
}

/// Component for SDI-12 environmental sensor driver.
///
/// This component initializes an SDI-12 driver using a virtual UART device
/// from a UART mux. It provides both transmit and receive capabilities
/// for communicating with SDI-12 sensors.
pub struct Sdi12Component<const RX_BUF_LEN: usize, const TX_BUF_LEN: usize> {
    uart_mux: &'static MuxUart<'static>,
    #[allow(dead_code)]
    driver_num: usize,
}

impl<const RX_BUF_LEN: usize, const TX_BUF_LEN: usize> Sdi12Component<RX_BUF_LEN, TX_BUF_LEN> {
    pub fn new(
        uart_mux: &'static MuxUart<'static>,
        driver_num: usize,
    ) -> Sdi12Component<RX_BUF_LEN, TX_BUF_LEN> {
        Sdi12Component {
            uart_mux,
            driver_num,
        }
    }
}

impl<const RX_BUF_LEN: usize, const TX_BUF_LEN: usize> Component
    for Sdi12Component<RX_BUF_LEN, TX_BUF_LEN>
{
    type StaticInput = (
        &'static mut MaybeUninit<[u8; TX_BUF_LEN]>,
        &'static mut MaybeUninit<[u8; RX_BUF_LEN]>,
        &'static mut MaybeUninit<UartDevice<'static>>,
    );
    type Output = &'static UartDevice<'static>;

    fn finalize(self, s: Self::StaticInput) -> Self::Output {
        let _write_buffer = s.0.write([0; TX_BUF_LEN]);

        let _read_buffer = s.1.write([0; RX_BUF_LEN]);

        let sdi12_uart = s.2.write(UartDevice::new(self.uart_mux, true));
        sdi12_uart.setup();

        // Set up transmit and receive clients (applications will hook into this)
        // hil::uart::Transmit::set_transmit_client(sdi12_uart, client);
        // hil::uart::Receive::set_receive_client(sdi12_uart, client);

        sdi12_uart
    }
}
