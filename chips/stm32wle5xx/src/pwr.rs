// Licensed under the Apache License, Version 2.0 or the MIT License.
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Copyright Tock Contributors 2025.

//! Power control peripheral for the STM32WLE5xx series.
//! This is a partial implementation focusing on exposing the functionality
//! required for Sub-GHz radio operation and RTC backup domain access.

use kernel::utilities::registers::interfaces::{ReadWriteable, Readable};
use kernel::utilities::registers::{register_bitfields, ReadWrite, WriteOnly};
use kernel::utilities::StaticRef;

const PWR: StaticRef<PwrRegisters> = unsafe { StaticRef::new(0x5800_0400 as *const _) };

#[repr(C)]
struct PwrRegisters {
    cr1: ReadWrite<u32, CR1::Register>,
    cr2: ReadWrite<u32>,
    cr3: ReadWrite<u32>,
    cr4: ReadWrite<u32>,
    sr1: ReadWrite<u32>,
    sr2: ReadWrite<u32, SR2::Register>,
    scr: WriteOnly<u32>,
    pub cr5: ReadWrite<u32>,
    pub pucra: ReadWrite<u32>,
    pub pdcra: ReadWrite<u32>,
    pub pucrb: ReadWrite<u32>,
    pub pdcrb: ReadWrite<u32>,
    pub pucrc: ReadWrite<u32>,
    pub pdcrc: ReadWrite<u32>, // Offset 0x034
    _reserved0: [u32; 9],
    pub pucrh: ReadWrite<u32>, // Offset 0x058
    pub pdcrh: ReadWrite<u32>, // Offset 0x05C
    _reserved1: [u32; 10],
    pub extscr: ReadWrite<u32>,                             // Offset 0x088
    pub subghzspicr: ReadWrite<u32, SUBGHZSPICR::Register>, // Offset 0x090
}

register_bitfields![ u32,
    /// Power control register 1 (PWR_CR1)
    CR1 [
        /// Low-power run mode
        LPR OFFSET(14) NUMBITS(1) [],
        /// Voltage scaling range selection
        VOS OFFSET(9) NUMBITS(2) [
            Range1 = 0b01,
            Range2 = 0b10,
        ],
        /// Disable backup domain write protection
        /// 0: Access to RTC and Backup registers disabled
        /// 1: Access to RTC and Backup registers enabled
        DBP OFFSET(8) NUMBITS(1) [],
        /// Flash memory power-down mode during Low-power sleep mode
        FPD_LPSLP OFFSET(5) NUMBITS(1) [],
        /// Flash memory power-down mode during Low-power run mode
        FPD_LPRUN OFFSET(4) NUMBITS(1) [],
        /// Flash memory power-down mode during Stop mode
        FPD_STOP OFFSET(3) NUMBITS(1) [],
        /// Low-power mode selection
        LPMS OFFSET(0) NUMBITS(3) [
            Stop0 = 0b000,
            Stop1 = 0b001,
            Stop2 = 0b010,
            Standby = 0b011,
            Shutdown = 0b100,
        ],
    ],
    SR2 [
        PVMO3 OFFSET(14) NUMBITS(1),
        PVDO OFFSET(11) NUMBITS(1),
        VOSF OFFSET(10) NUMBITS(1),
        REGLPF OFFSET(9) NUMBITS(1),
        REGLPS OFFSET(8) NUMBITS(1),
        FLASHRDY OFFSET(7) NUMBITS(1),
        REGMRS OFFSET(6) NUMBITS(1),
        RFEOLF OFFSET(5) NUMBITS(1),
        LDORDY OFFSET(4) NUMBITS(1),
        SMPSRDY OFFSET(3) NUMBITS(1),
        RFBUSYMS OFFSET(2) NUMBITS(1),
        RFBUSYS OFFSET(1) NUMBITS(1),
    ],
    SUBGHZSPICR [
        NSS OFFSET(15) NUMBITS(1),
    ]
];

pub struct Pwr {
    registers: StaticRef<PwrRegisters>,
}

impl Pwr {
    pub fn new() -> Pwr {
        assert!(core::mem::size_of::<PwrRegisters>() == 0x94);
        Pwr { registers: PWR }
    }

    /// Enable backup domain write access
    pub fn enable_backup_domain_access(&self) {
        self.registers.cr1.modify(CR1::DBP::SET);
    }

    /// Disable backup domain write access
    pub fn disable_backup_domain_access(&self) {
        self.registers.cr1.modify(CR1::DBP::CLEAR);
    }

    /// Check if backup domain write access is enabled
    pub fn is_backup_domain_access_enabled(&self) -> bool {
        self.registers.cr1.is_set(CR1::DBP)
    }

    pub fn is_rfbusys(&self) -> bool {
        self.registers.sr2.is_set(SR2::RFBUSYS)
    }

    pub fn set_nss(&self) {
        self.registers.subghzspicr.modify(SUBGHZSPICR::NSS::SET);
        assert!(self.is_set_nss());
    }

    pub fn clear_nss(&self) {
        self.registers.subghzspicr.modify(SUBGHZSPICR::NSS::CLEAR);
        assert!(!self.is_set_nss());
    }

    pub fn is_set_nss(&self) -> bool {
        self.registers.subghzspicr.is_set(SUBGHZSPICR::NSS)
    }
}
