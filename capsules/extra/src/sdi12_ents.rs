// Licensed under the Apache License, Version 2.0 or the MIT License.
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Copyright Tock Contributors 2025.

// Written by Stephen Taylor, UCSD, 2025

//! Provides userspace with access to sdi12 environmental sensors. sdi12 uses UART as a bus protocol in conjunction with a command pin gpio.
//!
//! Userspace Interface
//! -------------------
//!
//! ### `command` System Call
//!
//! The `command` system call support one argument `cmd` which is used to specify the
//! operation, currently the following cmd's are supported:
//!
//! * `0`: check whether the driver exists
//! * `1`: query the address of connected SDI12 sensor. Note this requires the command pin to be connected.
//! If multiple sensors are connected, they will all respond to this command and may result in bus contention.
//!
//!

use enum_primitive::enum_from_primitive;

use capsules_core::driver;
use core::cell::Cell;
use kernel::errorcode::ErrorCode;
use kernel::grant::UpcallCount;
use kernel::grant::{AllowRoCount, AllowRwCount, Grant};
use kernel::hil::sdi12;
use kernel::hil::sdi12::TransmitClient;
use kernel::processbuffer::ReadableProcessBuffer;
use kernel::processbuffer::WriteableProcessBuffer;
use kernel::syscall::{CommandReturn, SyscallDriver};
use kernel::utilities::cells::OptionalCell;
use kernel::utilities::cells::TakeCell;
use kernel::ProcessId;

pub const DRIVER_NUM: usize = driver::NUM::Sdi12Ents as usize;

pub enum Sdi12Status {
    Sdi12Ok = 0,
    Sdi12Error = -1,
    Sdi12TimeoutOnRead = -2,
    Sdi12ParsingError = -3,
}

#[derive(Copy, Clone, PartialEq)]
pub struct Sdi12MeasurmentValues {
    address: u8,
    time: u16,
    numvalues: u8,
}
pub enum State {
    Idle,
    AddressRequest,
    GivingAddress,
    WakingSensors,
    SendingCommand,
    ReadingResponse,
}

/// IDs for subscribed upcalls.
mod upcall {
    /// Sdi12 received.
    pub const SDI12_RX: usize = 0;
    /// Sdi12 transmit complete.
    pub const SDI12_TX: usize = 1;
    /// Number of upcalls.
    pub const COUNT: u8 = 2;
}

mod ro_allow {
    /// Tx buffer for SDI12 transmit.
    pub const TX_BUFFER: usize = 0;
    /// Number of read-only allow buffers.
    pub const COUNT: u8 = 1;
}

mod rw_allow {
    /// Rx buffer for SDI12 receive.
    pub const RX_BUFFER: usize = 0;
    /// Number of read-write allow buffers.
    pub const COUNT: u8 = 1;
}

pub struct Sdi12Ents<'a, S: sdi12::Transmit<'a> + sdi12::Receive<'a>> {
    state: Cell<State>,
    tx_buffer: TakeCell<'static, [u8]>,
    rx_buffer: TakeCell<'static, [u8]>,
    sdi12: &'a S,
    grant: Grant<
        App,
        UpcallCount<{ upcall::COUNT }>,
        AllowRoCount<{ ro_allow::COUNT }>,
        AllowRwCount<{ rw_allow::COUNT }>,
    >,
    tx_in_progress: OptionalCell<ProcessId>,
    rx_in_progress: OptionalCell<ProcessId>,
    /// Bytes copied to the userspace buffer for the current receive.
    rx_count: Cell<usize>,
    /// Maximum bytes requested by userspace for the current receive.
    rx_max: Cell<usize>,
}

/// Holds buffers and whatnot that the application has passed us.
#[derive(Default)]
pub struct App;

impl<'a, S: sdi12::Transmit<'a> + sdi12::Receive<'a>> Sdi12Ents<'a, S> {
    pub fn new(
        tx_buffer: &'static mut [u8],
        rx_buffer: &'static mut [u8],
        sdi12: &'a S,
        grant: Grant<
            App,
            UpcallCount<{ upcall::COUNT }>,
            AllowRoCount<{ ro_allow::COUNT }>,
            AllowRwCount<{ rw_allow::COUNT }>,
        >,
    ) -> Sdi12Ents<'a, S> {
        Sdi12Ents {
            state: Cell::new(State::Idle),
            tx_buffer: TakeCell::new(tx_buffer),
            rx_buffer: TakeCell::new(rx_buffer),
            sdi12,
            grant,
            tx_in_progress: OptionalCell::empty(),
            rx_in_progress: OptionalCell::empty(),
            rx_count: Cell::new(0),
            rx_max: Cell::new(0),
        }
    }

    /**
     ******************************************************************************
     * @brief    Send data previously copied into tx_buffer
     *
     * @param    self,
     * @param    len, usize
     * @return   Result<(), ErrorCode>
     ******************************************************************************
     */
    pub fn sdi12_send_from_buffer(&self, len: usize) -> Result<(), ErrorCode> {
        self.state.set(State::SendingCommand);

        if let Some(buffer) = self.tx_buffer.take() {
            match self.sdi12.transmit(buffer, len) {
                Ok(()) => Ok(()),
                Err((e, buf)) => {
                    self.tx_buffer.replace(buf);
                    Err(e)
                }
            }
        } else {
            Err(ErrorCode::NOMEM)
        }
    }

    /**
     ******************************************************************************
     * @brief    Start a receive via SDI12
     *
     * @param    self,
     * @param    processid, ProcessID
     * @param    size, usize
     * @return   Sdi12Status
     ******************************************************************************
     */
    pub fn sdi12_start_receive(&self, size: usize) -> Result<Sdi12Status, Sdi12Status> {
        // take a kernel rx buffer
        if let Some(buf) = self.rx_buffer.take() {
            match self.sdi12.receive(buf, size) {
                Ok(()) => {
                    self.state.set(State::ReadingResponse);
                    Ok(Sdi12Status::Sdi12Ok)
                }
                Err((_ecode, returned_buf)) => {
                    // restore kernel buffer and return a generic SDI12 error
                    self.rx_buffer.replace(returned_buf);
                    Err(Sdi12Status::Sdi12Error)
                }
            }
        } else {
            Err(Sdi12Status::Sdi12Error)
        }
    }
}

use enum_primitive::cast::FromPrimitive;

enum_from_primitive! {
    #[derive(Debug, PartialEq, Clone, Copy)]
    pub enum Cmd {
        Ping = 0,
        Write = 1,
        Read = 2,
        WhoKnows = 3,
    }
}

impl<'a, S> SyscallDriver for Sdi12Ents<'a, S>
where
    S: sdi12::Transmit<'a> + sdi12::Receive<'a>,
{
    fn command(
        &self,
        command_num: usize,
        _data1: usize,
        data2: usize,
        processid: ProcessId,
    ) -> CommandReturn {
        if let Some(cmd) = Cmd::from_usize(command_num) {
            match cmd {
                // Driver existence check
                Cmd::Ping => CommandReturn::success(),
                // test send data command
                Cmd::Write => {
                    self.tx_in_progress.set(processid);
                    // copy data from userspace buffer to kernel
                    let len = self.grant.enter(processid, |_app, kernel_data| {
                        let mut copied_len = 0;
                        if let Ok(buffer) =
                            kernel_data.get_readonly_processbuffer(ro_allow::TX_BUFFER)
                        {
                            let _ = buffer.enter(|data| {
                                let max_len = data.len().min(self.tx_buffer.map_or(0, |b| b.len()));
                                self.tx_buffer.map(|tx_buf| {
                                    data[..max_len].copy_to_slice(&mut tx_buf[..max_len]);
                                    copied_len = max_len;
                                });
                            });
                        }
                        copied_len
                    });

                    match len {
                        Ok(l) if l > 0 => match self.sdi12_send_from_buffer(l) {
                            Ok(()) => CommandReturn::success(),
                            Err(_) => {
                                self.tx_in_progress.clear();
                                CommandReturn::failure(ErrorCode::FAIL)
                            }
                        },
                        _ => {
                            self.tx_in_progress.clear();
                            CommandReturn::failure(ErrorCode::INVAL)
                        }
                    }
                }
                Cmd::Read => {
                    // Receive until '\n' or `data2` bytes, whichever comes
                    // first. SDI-12 responses are variable length and end in
                    // "\r\n", so a fixed length read can hang or truncate.
                    // Bytes are received one at a time and copied directly to
                    // the userspace buffer.
                    if data2 == 0 {
                        return CommandReturn::failure(ErrorCode::INVAL);
                    }
                    self.rx_in_progress.set(processid);
                    self.rx_count.set(0);
                    self.rx_max.set(data2);

                    // start receive and record process
                    match self.sdi12_start_receive(1) {
                        Ok(_) => CommandReturn::success(),
                        Err(_) => {
                            self.rx_in_progress.clear();
                            CommandReturn::failure(ErrorCode::FAIL)
                        }
                    }
                }
                Cmd::WhoKnows => CommandReturn::success(),
            }
        } else {
            CommandReturn::failure(ErrorCode::NOSUPPORT)
        }
    }

    fn allocate_grant(&self, processid: ProcessId) -> Result<(), kernel::process::Error> {
        // Allocation is performed implicitly when the grant region is entered.
        self.grant.enter(processid, |_, _| {})
    }
}

impl<'a, S: sdi12::Transmit<'a> + sdi12::Receive<'a>> TransmitClient for Sdi12Ents<'a, S> {
    fn transmitted_buffer(
        &self,
        buffer: &'static mut [u8],
        length: usize,
        status: Result<(), ErrorCode>,
    ) {
        // Put the buffer back into the TakeCell for reuse
        self.tx_buffer.replace(buffer);
        self.state.set(State::Idle);

        // Schedule upcall to notify userspace
        self.tx_in_progress.take().map(|processid| {
            let _ = self.grant.enter(processid, |_app, kernel_data| {
                let _ = kernel_data.schedule_upcall(
                    upcall::SDI12_TX,
                    (kernel::errorcode::into_statuscode(status), length, 0),
                );
            });
        });
    }
}

impl<'a, S: sdi12::Transmit<'a> + sdi12::Receive<'a>> sdi12::ReceiveClient for Sdi12Ents<'a, S> {
    fn receive_buffer(
        &self,
        buffer: &'static mut [u8],
        length: usize,
        mut status: Result<(), ErrorCode>,
        _error: kernel::hil::uart::Error,
    ) {
        let mut done = status.is_err() || length == 0;

        if length > 0 {
            // Copy the received byte to the next position in the userspace
            // buffer
            let byte = buffer[0];
            let idx = self.rx_count.get();
            self.rx_in_progress.map(|processid| {
                let _ = self.grant.enter(processid, |_app, kernel_data| {
                    if let Ok(user_buf) =
                        kernel_data.get_readwrite_processbuffer(rw_allow::RX_BUFFER)
                    {
                        let _ = user_buf.mut_enter(|dest| {
                            if let Some(d) = dest.get(idx) {
                                d.set(byte);
                            }
                        });
                    }
                });
            });
            self.rx_count.set(idx + 1);

            // SDI-12 is 7 data bits, ignore the parity bit when checking for
            // the end of the response
            if byte & 0x7F == b'\n' || self.rx_count.get() >= self.rx_max.get() {
                done = true;
            }
        }

        if !done {
            // Wait for the next byte
            match self.sdi12.receive(buffer, 1) {
                Ok(()) => return,
                Err((e, buf)) => {
                    self.rx_buffer.replace(buf);
                    status = Err(e);
                }
            }
        } else {
            // Put the buffer back into the TakeCell for reuse
            self.rx_buffer.replace(buffer);
        }

        self.state.set(State::Idle);

        // Schedule upcall to notify userspace
        let count = self.rx_count.get();
        self.rx_in_progress.take().map(|processid| {
            let _ = self.grant.enter(processid, |_app, kernel_data| {
                let _ = kernel_data.schedule_upcall(
                    upcall::SDI12_RX,
                    (kernel::errorcode::into_statuscode(status), count, 0),
                );
            });
        });
    }
}
