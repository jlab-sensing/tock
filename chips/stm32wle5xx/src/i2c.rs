// Licensed under the Apache License, Version 2.0 or the MIT License.
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Copyright Tock Contributors 2022.

use core::cell::Cell;

use kernel::hil;
use kernel::hil::i2c::{self, Error, I2CHwMasterClient, I2CMaster};
use kernel::platform::chip::ClockInterface;
use kernel::utilities::cells::{OptionalCell, TakeCell};
use kernel::utilities::registers::interfaces::{ReadWriteable, Readable, Writeable};
use kernel::utilities::registers::{register_structs, register_bitfields, ReadOnly, ReadWrite, WriteOnly};
use kernel::utilities::StaticRef;

use crate::clocks::{phclk, Stm32wle5xxClocks};

use kernel::debug;

pub enum I2CSpeed {
    Speed100k,
    Speed400k,
}

struct I2CTiming {
    // t_HD;DAT -> Data hold time (us)
    t_hold: f32,
    // t_VD;DAT -> Data valid time (us)
    t_valid: f32,
    // t_SU;DAT -> Data setup time (us)
    t_setup: f32,
    // Rise time of both signals (us)
    t_r: f32,
    // Fall time of both signals (ns)
    t_f: f32,
}

//// Inter-Integrated Circuit
register_structs! {
    I2CRegisters {
        // control register 1
        (0x000 => cr1: ReadWrite<u32, CR1::Register>),
        // control register 2
        (0x004 => cr2: ReadWrite<u32, CR2::Register>),
        // own address register 1
        (0x008 => oar1: ReadWrite<u32, OAR1::Register>),
        // own address register 2
        (0x00C => oar2: ReadWrite<u32, OAR2::Register>),
        // timing register
        (0x010 => timingr: ReadWrite<u32, TIMINGR::Register>),
        // timeout register
        (0x014 => timeoutr: ReadWrite<u32, TIMEOUTR::Register>),
        // interrupt and status register
        (0x018 => isr: ReadOnly<u32, ISR::Register>),
        // interrupt clear register
        (0x01C => icr: WriteOnly<u32, ICR::Register>),
        // PEC register
        (0x020 => pecr: ReadOnly<u32, PECR::Register>),
        // receive data register
        (0x024 => rxdr: ReadOnly<u32, RXDR::Register>),
        // transmit data register
        (0x028 => txdr: ReadWrite<u32, TXDR::Register>),
        // end
        (0x02C => @END),
    }

}

register_bitfields![u32,
    // control register 1
    CR1 [
        // PEC enable
        PECEN OFFSET(23) NUMBITS(1) [],
        // SMBus alert enable
        ALERTEN OFFSET(22) NUMBITS(1) [],
        // SMBus device default address enable
        SMBDEN OFFSET(21) NUMBITS(1) [],
        // SMBus host address enable
        SMBHEN OFFSET(20) NUMBITS(1) [],
        // General call enable
        GCEN OFFSET(19) NUMBITS(1) [],
        // Wake-up from Stop mode enable
        WUPEN OFFSET(18) NUMBITS(1) [],
        // Clock stretching disable
        NOSTRETCH OFFSET(17) NUMBITS(1) [],
        // Target byte control
        SBC OFFSET(16) NUMBITS(1) [],
        // DMA reception request enable
        RXDMAEN OFFSET(15) NUMBITS(1) [],
        // DMA transmission request enable
        TXDMAEN OFFSET(14) NUMBITS(1) [],
        // Analog noise filter OFF
        ANFOFF OFFSET(12) NUMBITS(1) [],
        // Digital noise filter
        DNF OFFSET(8) NUMBITS(4) [],
        // Error interrupt enable
        ERRIE OFFSET(7) NUMBITS(1) [],
        // Transfer complete interrupt enable
        TCIE OFFSET(6) NUMBITS(1) [],
        // STOP detection interrupt enable
        STOPIE OFFSET(5) NUMBITS(1) [],
        // Not acknowledge received interrupt enable
        NACKIE OFFSET(4) NUMBITS(1) [],
        // Address match interrupt enable (target only)
        ADDRIE OFFSET(3) NUMBITS(1) [],
        // RX interrupt enable
        RXIE OFFSET(2) NUMBITS(1) [],
        // TX interrupt enable
        TXIE OFFSET(1) NUMBITS(1) [],
        // Peripheral enable
        PE OFFSET(0) NUMBITS(1) [],
    ],
    CR2 [
        // Packet error checking byte
        PECBYTE OFFSET(26) NUMBITS(1) [],
        // Automatic end mode (controller mode)
        AUTOEND OFFSET(25) NUMBITS(1) [],
        // NBYTES reload mode
        RELOAD OFFSET(24) NUMBITS(1) [],
        // Number of bytes
        NBYTES OFFSET(16) NUMBITS(8) [],
        // NACK generation (target mode)
        NACK OFFSET(15) NUMBITS(1) [],
        // STOP condition generation
        STOP OFFSET(14) NUMBITS(1) [],
        // START condition generation
        START OFFSET(13) NUMBITS(1) [],
        // 10-bit address header only read direction
        HEAD10R OFFSET(12) NUMBITS(1) [],
        // 10-bit addressing mode
        ADD10 OFFSET(11) NUMBITS(1) [
            Bit7 = 0,
            Bit10 = 1
        ],
        // Transfer direction (controller mode)
        RD_WRN OFFSET(10) NUMBITS(1) [
            Write = 0,
            Read = 1
        ],
        // Target address (controller mode)
        SADD OFFSET(0) NUMBITS(10) [],
    ],
    OAR1 [
        // Own address 1 enable
        OA1EN OFFSET(15) NUMBITS(1) [],
        // Addressing mode (target mode)
        OA1MODE OFFSET(10) NUMBITS(1) [
            Bit7 = 0,
            Bit10 = 1
        ],
        // Interface address
        OA1 OFFSET(1) NUMBITS(7) [],
    ],
    OAR2 [
        // Own address 2 enable
        OA2EN OFFSET(15) NUMBITS(1) [],
        // Own address 2 masks
        OA2MSK OFFSET(8) NUMBITS(3) [],
        // Interface address
        OA2 OFFSET(1) NUMBITS(7) [],
    ],
    TIMINGR [
        // Timing prescalar
        PRESC OFFSET(28) NUMBITS(4) [],
        // Data setup time
        SCLDEL OFFSET(20) NUMBITS(4) [],
        // Data hold time
        SDADEL OFFSET(16) NUMBITS(4) [],
        // SCL high period (controller mode)
        SCLH OFFSET(8) NUMBITS(8) [],
        // SCL low period (controller mode)
        SCLL OFFSET(0) NUMBITS(8) [],
    ],
    TIMEOUTR [
        // Extended clock timeout
        TEXTEN OFFSET(31) NUMBITS(1) [],
        // Bus timeout B
        TIMEOUTB OFFSET(16) NUMBITS(12) [],
        // Clock timeout enable
        TIMEOUTEN OFFSET(15) NUMBITS(1) [],
        // Idle clock timeout detection
        TIDLE OFFSET(12) NUMBITS(1) [],
        // Bus timeout A
        TIMEOUTA OFFSET(0) NUMBITS(12) [],
    ],
    ISR [
        // Address match moe (target mode)
        ADDCDE OFFSET(17) NUMBITS(7) [],
        // Transfer direction (target mode)
        DIR OFFSET(16) NUMBITS(1) [
            Write = 0,
            Read = 1
        ],
        // Bus busy
        BUSY OFFSET(15) NUMBITS(1) [],
        // SMBus alert
        ALERT OFFSET(13) NUMBITS(1) [],
        // Timeout or t_low detection flag
        TIMEOUT OFFSET(12) NUMBITS(1) [],
        // PEC error in reception
        PECERR OFFSET(11) NUMBITS(1) [],
        // Overrun/underrun (target mode)
        OVR OFFSET(10) NUMBITS(1) [],
        // Arbitration loss
        ARLO OFFSET(9) NUMBITS(1) [],
        // Bus error
        BERR OFFSET(8) NUMBITS(1) [],
        // Transfer complete reload
        TCR OFFSET(7) NUMBITS(1) [],
        // Transfer complete (conntroller mode)
        TC OFFSET(6) NUMBITS(1) [],
        // STOP detection flag
        STOPF OFFSET(5) NUMBITS(1) [],
        // Not achnowledge received flag
        NACKF OFFSET(4) NUMBITS(1) [],
        // Address matched (îrget mode)
        ADDR OFFSET(3) NUMBITS(1) [],
        // Receive data register not empty (receivers)
        RXNE OFFSET(2) NUMBITS(1) [],
        // Transmit interrupt status
        TXIS OFFSET(1) NUMBITS(1) [],
        // Transmit data register empty (transmitters)
        TXE OFFSET(0) NUMBITS(1) [],
    ],
    ICR [
        // Alert flag clear
        ALERTCF OFFSET(13) NUMBITS(1) [],
        // Timeout detection flag clear
        TIMEOUTCF OFFSET(12) NUMBITS(1) [],
        // PEC error flag clear
        PECCF OFFSET(11) NUMBITS(1) [],
        // Overrun/underrun flag clear
        OVRCF OFFSET(10) NUMBITS(1) [],
        // Abritration lost flag clear
        ARLOCF OFFSET(9) NUMBITS(1) [],
        // Bus error flag clear
        BERRCF OFFSET(8) NUMBITS(1) [],
        // STOP detection flag clear
        STOPCF OFFSET(5) NUMBITS(1) [],
        // Not acknowledge flag clear
        NACKCF OFFSET(4) NUMBITS(1) [],
        // Address matched flag clear
        ADDRCF OFFSET(3) NUMBITS(1) [],
    ],
    PECR [
        // Packet error checking register
        PEC OFFSET(0) NUMBITS(8) [],
    ],
    RXDR [
        // 8-bit receive data
        RXDATA OFFSET(0) NUMBITS(8) [],
    ],
    TXDR [
        // 8-bit transmit data
        TXDATA OFFSET(0) NUMBITS(8) [],
    ],
];

const I2C1_BASE: StaticRef<I2CRegisters> =
    unsafe { StaticRef::new(0x4000_5400 as *const I2CRegisters) };
//const I2C2_BASE: StaticRef<I2CRegisters> =
//    unsafe { StaticRef::new(0x4000_5800 as *const I2CRegisters) };
//const I2C3_BASE: StaticRef<I2CRegisters> =
//    unsafe { StaticRef::new(0x4000_5C00 as *const I2CRegisters) };

struct I2CClock<'a>(phclk::PeripheralClock<'a>);

impl ClockInterface for I2CClock<'_> {
    fn is_enabled(&self) -> bool {
        self.0.is_enabled()
    }

    fn enable(&self) {
        self.0.enable();
    }

    fn disable(&self) {
        self.0.disable();
    }
}

pub struct I2C<'a> {
    registers: StaticRef<I2CRegisters>,
    clock: I2CClock<'a>,

    // I2C slave support not yet implemented
    master_client: OptionalCell<&'a dyn hil::i2c::I2CHwMasterClient>,

    buffer: TakeCell<'static, [u8]>,
    tx_position: Cell<usize>,
    rx_position: Cell<usize>,
    tx_len: Cell<usize>,
    rx_len: Cell<usize>,

    slave_address: Cell<u8>,

    status: Cell<I2CStatus>,
}

#[derive(Copy, Clone, PartialEq)]
enum I2CStatus {
    Idle,
    Writing,
    WritingReading,
    Reading,
}

impl<'a> I2C<'a> {
    pub fn new(clocks: &'a dyn Stm32wle5xxClocks) -> Self {
        Self {
            registers: I2C1_BASE,
            clock: I2CClock(phclk::PeripheralClock::new(
                phclk::PeripheralClockType::APB1(phclk::PCLK1::I2C1),
                clocks,
            )),

            master_client: OptionalCell::empty(),

            slave_address: Cell::new(0),

            buffer: TakeCell::empty(),
            tx_position: Cell::new(0),
            rx_position: Cell::new(0),

            tx_len: Cell::new(0),
            rx_len: Cell::new(0),

            status: Cell::new(I2CStatus::Idle),
        }
    }
    
    pub fn set_speed(&self, speed: I2CSpeed, system_clock_in_mhz: usize) {
        let timing = match speed {
            I2CSpeed::Speed100k => I2CTiming {
                t_hold: 0.,
                t_valid: 3.45,
                t_setup: 0.250,
                t_r: 1.0,
                t_f: 0.3,
            },
            I2CSpeed::Speed400k => I2CTiming {
                t_hold: 0.,
                t_valid: 0.9,
                t_setup: 0.1,
                t_r: 0.300,
                t_f: 0.3,
            }
        };

        // set the prescalar value
        let presc: u32 = 0;

        let t_i2c_clk = 1_000_000.0 / (system_clock_in_mhz as f32 * 1_000_000.0);

        // calculate timing requirements
        let scldel = (timing.t_valid - timing.t_f - (4. * t_i2c_clk)) / ((presc as f32 + 1.) * t_i2c_clk); 
        let sdadel = (timing.t_r + timing.t_setup) / ((presc as f32 + 1.) * t_i2c_clk) - 1.;


        debug!("I2C timing: presc {}, scldel {}, sdadel {}", presc, scldel, sdadel);

        // Set register values
        self.registers.timingr.modify(TIMINGR::PRESC.val(presc));
        // round down
        self.registers.timingr.modify(TIMINGR::SCLDEL.val(scldel as u32));
        // round up 
        self.registers.timingr.modify(TIMINGR::SDADEL.val(sdadel as u32 + 1));
    }
    
    pub fn is_enabled_clock(&self) -> bool {
        self.clock.is_enabled()
    }

    pub fn enable_clock(&self) {
        self.clock.enable();
    }

    pub fn disable_clock(&self) {
        self.clock.disable();
    }

    pub fn handle_event(&self) {
        // TODO implement event handler
       
        // handle no acknowledge
        if self.registers.isr.is_set(ISR::NACKF) {
            self.handle_error();
            return;
        }

        // send next byte when TXIS is set
        if self.registers.isr.is_set(ISR::TXIS) {
            // check data is available
            if self.buffer.is_some() && self.tx_position.get() < self.tx_len.get() {
                // ready to send data
                self.buffer.take().map(|buf| {
                    let pos = self.tx_position.get();
                    if pos < self.tx_len.get() {
                        self.registers.txdr.set(u32::from(buf[pos]));
                        self.tx_position.set(pos + 1);
                        self.buffer.replace(buf);
                    }
                });
            } else {
                // TODO error handling?
            }
        }

        // handle a stop condition
        // From HAL drivers: apparently there's no need to check for TC since the STOP condition is
        // automatically generated.
        if self.registers.isr.is_set(ISR::STOPF) {
            // clear stop flag
            self.registers.icr.write(ICR::STOPCF::SET);
            // transaction complete
            self.status.set(I2CStatus::Idle);
            // send message back to driver
            self.master_client.map(|client| {
                self.buffer
                    .take()
                    .map(|buf| client.command_complete(buf, Ok(())))
            });
        }
    }
    
    pub fn handle_error(&self) {
        self.master_client.map(|client| {
            self.buffer
                .take()
                .map(|buf| client.command_complete(buf, Err(Error::DataNak)))
        });
        self.stop();
    }

    fn reset(&self) {
        self.disable();
        self.enable();
    }

    fn start_write(&self) {
        // check interface is not busy? maybe just error out?
        while self.registers.isr.read(ISR::BUSY) != 0 {}

        if self.tx_len.get() <= 255 {
            self.tx_position.set(0);
            // set number of bytes to send
            self.registers.cr2.modify(CR2::NBYTES.val(self.tx_len.get() as u32));
            // automatically send STOP after NBYTES
            self.registers.cr2.modify(CR2::AUTOEND::SET);
            // set target address
            self.registers.cr2.modify(CR2::SADD.val(u32::from(self.slave_address.get())));
            // set start
            self.registers.cr2.modify(CR2::START::SET);
        } else {
            // TODO handle more than 255 bytes
            self.handle_error();
            return;
        }
    }

    fn stop(&self) {
        self.status.set(I2CStatus::Idle);
        // clear CR2 register
        self.registers.cr2.set(0);
        // NOTE maybe clear interrupt flags?
    }

    fn start_read(&self) {
        // TODO implement start read
    }
}

impl<'a> i2c::I2CMaster<'a> for I2C<'a> {
    fn set_master_client(&self, master_client: &'a dyn I2CHwMasterClient) {
        self.master_client.replace(master_client);
    }

    fn enable(&self) {
        self.registers.cr1.modify(CR1::PE::SET);
    }

    fn disable(&self) {
        self.registers.cr1.modify(CR1::PE::CLEAR);
    }

    fn write_read(
        &self,
        addr: u8,
        data: &'static mut [u8],
        write_len: usize,
        read_len: usize,
    ) -> Result<(), (Error, &'static mut [u8])> {
        if self.status.get() == I2CStatus::Idle {
            self.reset();
            self.status.set(I2CStatus::WritingReading);
            self.slave_address.set(addr);
            self.buffer.replace(data);
            self.tx_len.set(write_len);
            self.rx_len.set(read_len);
            self.start_write();
            Ok(())
        } else {
            Err((Error::Busy, data))
        }
    }

    fn write(
        &self,
        addr: u8,
        data: &'static mut [u8],
        len: usize,
    ) -> Result<(), (Error, &'static mut [u8])> {
        debug!("I2C write to addr {:#x}, len {}", addr, len);
        if self.status.get() == I2CStatus::Idle {
            self.reset();
            self.status.set(I2CStatus::Writing);
            self.slave_address.set(addr);
            self.buffer.replace(data);
            self.tx_len.set(len);
            self.start_write();
            Ok(())
        } else {
            Err((Error::Busy, data))
        }
    }

    fn read(
        &self,
        addr: u8,
        buffer: &'static mut [u8],
        len: usize,
    ) -> Result<(), (Error, &'static mut [u8])> {
        if self.status.get() == I2CStatus::Idle {
            self.reset();
            self.status.set(I2CStatus::Reading);
            self.slave_address.set(addr);
            self.buffer.replace(buffer);
            self.rx_len.set(len);
            self.start_read();
            Ok(())
        } else {
            Err((Error::ArbitrationLost, buffer))
        }
    }
}

