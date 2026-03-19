// Licensed under the Apache License, Version 2.0 or the MIT License.
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Copyright Tock Contributors 2025.

//! Real-Time Clock (RTC) driver for the STM32WLE5x.
//!
//! The RTC provides a calendar with programmable time and date,
//! alarm functions, and wakeup timer capabilities.
//!

use core::cell::Cell;
use kernel::deferred_call::{DeferredCall, DeferredCallClient};
use kernel::hil::date_time::{DateTimeClient, DateTimeValues, DayOfWeek, Month};
use kernel::platform::chip::ClockInterface;
use kernel::utilities::cells::OptionalCell;
use kernel::utilities::registers::interfaces::{ReadWriteable, Readable, Writeable};
use kernel::utilities::registers::{register_bitfields, ReadOnly, ReadWrite, WriteOnly};
use kernel::utilities::StaticRef;
use kernel::ErrorCode;

use crate::clocks::{phclk, Stm32wle5xxClocks};

/// RTC base address
const RTC_BASE: StaticRef<RtcRegisters> =
    unsafe { StaticRef::new(0x4000_2800 as *const RtcRegisters) };

/// RTC Register Block
///
/// Memory-mapped registers for the RTC peripheral.
#[repr(C)]
pub struct RtcRegisters {
    /// Time register (TR) - offset 0x00
    /// Contains time in BCD format (hours, minutes, seconds)
    pub tr: ReadWrite<u32, TR::Register>,

    /// Date register (DR) - offset 0x04
    /// Contains date in BCD format (year, month, day, weekday)
    pub dr: ReadWrite<u32, DR::Register>,

    /// Sub-second register (SSR) - offset 0x08
    /// Contains the sub-second value
    pub ssr: ReadOnly<u32, SSR::Register>,

    /// Initialization and status register (ICSR) - offset 0x0C
    /// Contains initialization and status flags
    pub icsr: ReadWrite<u32, ICSR::Register>,

    /// Prescaler register (PRER) - offset 0x10
    /// Contains the prescaler values for generating 1 Hz clock
    pub prer: ReadWrite<u32, PRER::Register>,

    /// Wakeup timer register (WUTR) - offset 0x14
    /// Contains the wakeup timer reload value
    pub wutr: ReadWrite<u32, WUTR::Register>,

    /// Control register (CR) - offset 0x18
    /// Contains control bits for RTC operation
    pub cr: ReadWrite<u32, CR::Register>,

    /// Reserved - offset 0x1C and 0x20
    _reserved0: [u8; 8],

    /// Write protection register (WPR) - offset 0x24
    /// Used to enable/disable write protection
    pub wpr: WriteOnly<u32, WPR::Register>,

    /// Calibration register (CALR) - offset 0x28
    /// Contains calibration settings
    pub calr: ReadWrite<u32, CALR::Register>,

    /// Shift control register (SHIFTR) - offset 0x2C
    /// Used for time shift operations
    pub shiftr: WriteOnly<u32, SHIFTR::Register>,

    /// Timestamp time register (TSTR) - offset 0x30
    /// Contains timestamp time in BCD format
    pub tstr: ReadOnly<u32, TSTR::Register>,

    /// Timestamp date register (TSDR) - offset 0x34
    /// Contains timestamp date in BCD format
    pub tsdr: ReadOnly<u32, TSDR::Register>,

    /// Timestamp sub-second register (TSSSR) - offset 0x38
    /// Contains timestamp sub-second value
    pub tsssr: ReadOnly<u32, TSSSR::Register>,

    /// Reserved - offset 0x3C
    _reserved1: [u8; 4],

    /// Alarm A register (ALRMAR) - offset 0x40
    /// Contains alarm A time/date settings
    pub alrmar: ReadWrite<u32, ALRMAR::Register>,

    /// Alarm A sub-second register (ALRMASSR) - offset 0x44
    /// Contains alarm A sub-second settings
    pub alrmassr: ReadWrite<u32, ALRMASSR::Register>,

    /// Alarm B register (ALRMBR) - offset 0x48
    /// Contains alarm B time/date settings
    pub alrmbr: ReadWrite<u32, ALRMBR::Register>,

    /// Alarm B sub-second register (ALRMBSSR) - offset 0x4C
    /// Contains alarm B sub-second settings
    pub alrmbssr: ReadWrite<u32, ALRMBSSR::Register>,

    /// Status register (SR) - offset 0x50
    /// Contains status flags
    pub sr: ReadOnly<u32, SR::Register>,

    /// Masked interrupt status register (MISR) - offset 0x54
    /// Contains masked interrupt status flags
    pub misr: ReadOnly<u32, MISR::Register>,

    /// Reserved - offset 0x58
    _reserved2: [u8; 4],

    /// Status clear register (SCR) - offset 0x5C
    /// Used to clear status flags
    pub scr: WriteOnly<u32, SCR::Register>,
}

register_bitfields![u32,
    /// Time Register (TR)
    /// BCD format: HT[1:0]:HU[3:0]:MNT[2:0]:MNU[3:0]:ST[2:0]:SU[3:0]
    TR [
        /// AM/PM notation
        /// 0: AM or 24-hour format
        /// 1: PM
        PM OFFSET(22) NUMBITS(1) [],

        /// Hour tens in BCD format (0-2)
        HT OFFSET(20) NUMBITS(2) [],

        /// Hour units in BCD format (0-9)
        HU OFFSET(16) NUMBITS(4) [],

        /// Minute tens in BCD format (0-5)
        MNT OFFSET(12) NUMBITS(3) [],

        /// Minute units in BCD format (0-9)
        MNU OFFSET(8) NUMBITS(4) [],

        /// Second tens in BCD format (0-5)
        ST OFFSET(4) NUMBITS(3) [],

        /// Second units in BCD format (0-9)
        SU OFFSET(0) NUMBITS(4) []
    ],

    /// Date Register (DR)
    /// BCD format: YT[3:0]:YU[3:0]:WDU[2:0]:MT:MU[3:0]:DT[1:0]:DU[3:0]
    DR [
        /// Year tens in BCD format (0-9)
        YT OFFSET(20) NUMBITS(4) [],

        /// Year units in BCD format (0-9)
        YU OFFSET(16) NUMBITS(4) [],

        /// Week day units (1-7)
        /// 1 = Monday, 2 = Tuesday, ..., 7 = Sunday
        WDU OFFSET(13) NUMBITS(3) [],

        /// Month tens in BCD format (0-1)
        MT OFFSET(12) NUMBITS(1) [],

        /// Month units in BCD format (0-9)
        MU OFFSET(8) NUMBITS(4) [],

        /// Date tens in BCD format (0-3)
        DT OFFSET(4) NUMBITS(2) [],

        /// Date units in BCD format (0-9)
        DU OFFSET(0) NUMBITS(4) []
    ],

    /// Sub-second Register (SSR)
    SSR [
        /// Sub-second value
        /// Down-counter value (0 to PREDIV_S)
        SS OFFSET(0) NUMBITS(16) []
    ],

    /// Initialization and Status Register (ICSR)
    ICSR [
        /// Recalibration pending flag
        RECALPF OFFSET(16) NUMBITS(1) [],

        /// Binary mode
        /// 0: Free running BCD mode
        /// 1: Free running binary mode
        BIN OFFSET(8) NUMBITS(2) [
            BCD = 0b00,
            Binary = 0b01,
            Mixed = 0b10
        ],

        /// BCD update
        /// 0: Calendar update in BCD mode
        /// 1: Calendar update in mixed mode
        BCDU OFFSET(7) NUMBITS(3) [],

        /// Initialization mode
        /// 0: Free running mode
        /// 1: Initialization mode used to program time and date registers
        INIT OFFSET(7) NUMBITS(1) [],

        /// Initialization flag
        INITF OFFSET(6) NUMBITS(1) [],

        /// Registers synchronization flag
        RSF OFFSET(5) NUMBITS(1) [],

        /// Initialization status flag
        INITS OFFSET(4) NUMBITS(1) [],

        /// Shift operation pending
        SHPF OFFSET(3) NUMBITS(1) [],

        /// Wakeup timer write flag
        WUTWF OFFSET(2) NUMBITS(1) []
    ],

    /// Prescaler Register (PRER)
    PRER [
        /// Asynchronous prescaler factor (7-bit value)
        PREDIV_A OFFSET(16) NUMBITS(7) [],

        /// Synchronous prescaler factor (15-bit value)
        PREDIV_S OFFSET(0) NUMBITS(15) []
    ],

    /// Wakeup Timer Register (WUTR)
    WUTR [
        /// Wakeup auto-reload output clear value
        WUTOCLR OFFSET(16) NUMBITS(16) [],

        /// Wakeup auto-reload value
        WUT OFFSET(0) NUMBITS(16) []
    ],

    /// Control Register (CR)
    CR [
        /// Calibration output selection
        /// 0: 512 Hz output
        /// 1: 1 Hz output
        COE OFFSET(23) NUMBITS(1) [],

        /// Output selection
        /// 00: Output disabled
        /// 01: Alarm A output enabled
        /// 10: Alarm B output enabled
        /// 11: Wakeup output enabled
        OSEL OFFSET(21) NUMBITS(2) [
            Disabled = 0b00,
            AlarmA = 0b01,
            AlarmB = 0b10,
            Wakeup = 0b11
        ],

        /// Output polarity
        /// 0: Output pin is high when OSEL is matched
        /// 1: Output pin is low when OSEL is matched
        POL OFFSET(20) NUMBITS(1) [],

        /// Calibration output selection
        COSEL OFFSET(19) NUMBITS(1) [],

        /// Backup
        /// This bit can be written by the user to memorize whether
        /// the daylight saving time change has been performed
        BKP OFFSET(18) NUMBITS(1) [],

        /// Subtract 1 hour (winter time change)
        /// 0: No effect
        /// 1: Subtracts 1 hour to the calendar time
        SUB1H OFFSET(17) NUMBITS(1) [],

        /// Add 1 hour (summer time change)
        /// 0: No effect
        /// 1: Adds 1 hour to the calendar time
        ADD1H OFFSET(16) NUMBITS(1) [],

        /// Timestamp interrupt enable
        TSIE OFFSET(15) NUMBITS(1) [],

        /// Wakeup timer interrupt enable
        WUTIE OFFSET(14) NUMBITS(1) [],

        /// Alarm B interrupt enable
        ALRBIE OFFSET(13) NUMBITS(1) [],

        /// Alarm A interrupt enable
        ALRAIE OFFSET(12) NUMBITS(1) [],

        /// Timestamp enable
        TSE OFFSET(11) NUMBITS(1) [],

        /// Wakeup timer enable
        WUTE OFFSET(10) NUMBITS(1) [],

        /// Alarm B enable
        ALRBE OFFSET(9) NUMBITS(1) [],

        /// Alarm A enable
        ALRAE OFFSET(8) NUMBITS(1) [],

        /// Hour format
        /// 0: 24-hour format
        /// 1: AM/PM hour format
        FMT OFFSET(6) NUMBITS(1) [
            TwentyFourHour = 0,
            AmPm = 1
        ],

        /// Bypass the shadow registers
        /// 0: Calendar values are taken from shadow registers
        /// 1: Calendar values are taken directly from calendar counters
        BYPSHAD OFFSET(5) NUMBITS(1) [],

        /// Reference clock detection enable
        REFCKON OFFSET(4) NUMBITS(1) [],

        /// Timestamp event active edge
        /// 0: Rising edge generates timestamp event
        /// 1: Falling edge generates timestamp event
        TSEDGE OFFSET(3) NUMBITS(1) [],

        /// Wakeup clock selection
        /// 000: RTC/16 clock
        /// 001: RTC/8 clock
        /// 010: RTC/4 clock
        /// 011: RTC/2 clock
        /// 10x: ck_spre clock
        /// 11x: ck_spre clock and WUT[16] is added to WUT counter
        WUCKSEL OFFSET(0) NUMBITS(3) [
            RtcDiv16 = 0b000,
            RtcDiv8 = 0b001,
            RtcDiv4 = 0b010,
            RtcDiv2 = 0b011,
            CkSpre = 0b100,
            CkSpreWithWut16 = 0b110
        ]
    ],

    /// Write Protection Register (WPR)
    WPR [
        /// Write protection key
        /// To unlock write protection:
        /// 1. Write 0xCA to this register
        /// 2. Write 0x53 to this register
        /// To lock write protection:
        /// Write any wrong key (e.g., 0xFF)
        KEY OFFSET(0) NUMBITS(8) []
    ],

    /// Calibration Register (CALR)
    CALR [
        /// Use a 16-second calibration cycle period
        /// 0: 32-second calibration cycle
        /// 1: 16-second calibration cycle
        CALP OFFSET(15) NUMBITS(1) [],

        /// Use an 8-second calibration cycle period
        /// 0: No effect
        /// 1: 8-second calibration cycle
        CALW8 OFFSET(14) NUMBITS(1) [],

        /// Use a 16-second calibration cycle period
        CALW16 OFFSET(13) NUMBITS(1) [],

        /// Calibration minus
        /// The frequency of the calendar is reduced by masking
        /// CALM out of 2^20 RTCCLK pulses
        CALM OFFSET(0) NUMBITS(9) []
    ],

    /// Shift Control Register (SHIFTR)
    SHIFTR [
        /// Add one second
        /// 0: No effect
        /// 1: Add one second to the clock/calendar
        ADD1S OFFSET(31) NUMBITS(1) [],

        /// Subtract a fraction of a second
        SUBFS OFFSET(0) NUMBITS(15) []
    ],

    /// Timestamp Time Register (TSTR)
    TSTR [
        /// AM/PM notation
        PM OFFSET(22) NUMBITS(1) [],

        /// Hour tens in BCD format
        HT OFFSET(20) NUMBITS(2) [],

        /// Hour units in BCD format
        HU OFFSET(16) NUMBITS(4) [],

        /// Minute tens in BCD format
        MNT OFFSET(12) NUMBITS(3) [],

        /// Minute units in BCD format
        MNU OFFSET(8) NUMBITS(4) [],

        /// Second tens in BCD format
        ST OFFSET(4) NUMBITS(3) [],

        /// Second units in BCD format
        SU OFFSET(0) NUMBITS(4) []
    ],

    /// Timestamp Date Register (TSDR)
    TSDR [
        /// Week day units
        WDU OFFSET(13) NUMBITS(3) [],

        /// Month tens in BCD format
        MT OFFSET(12) NUMBITS(1) [],

        /// Month units in BCD format
        MU OFFSET(8) NUMBITS(4) [],

        /// Date tens in BCD format
        DT OFFSET(4) NUMBITS(2) [],

        /// Date units in BCD format
        DU OFFSET(0) NUMBITS(4) []
    ],

    /// Timestamp Sub-second Register (TSSSR)
    TSSSR [
        /// Sub-second value
        SS OFFSET(0) NUMBITS(16) []
    ],

    /// Alarm A Register (ALRMAR)
    ALRMAR [
        /// Alarm A date mask
        /// 0: Alarm A set if date/day match
        /// 1: Date/day don't care in alarm A comparison
        MSK4 OFFSET(31) NUMBITS(1) [],

        /// Week day selection
        /// 0: DU[3:0] represents the date units
        /// 1: DU[3:0] represents the week day
        WDSEL OFFSET(30) NUMBITS(1) [],

        /// Date tens in BCD format
        DT OFFSET(28) NUMBITS(2) [],

        /// Date units or day in BCD format
        DU OFFSET(24) NUMBITS(4) [],

        /// Alarm A hours mask
        MSK3 OFFSET(23) NUMBITS(1) [],

        /// AM/PM notation
        PM OFFSET(22) NUMBITS(1) [],

        /// Hour tens in BCD format
        HT OFFSET(20) NUMBITS(2) [],

        /// Hour units in BCD format
        HU OFFSET(16) NUMBITS(4) [],

        /// Alarm A minutes mask
        MSK2 OFFSET(15) NUMBITS(1) [],

        /// Minute tens in BCD format
        MNT OFFSET(12) NUMBITS(3) [],

        /// Minute units in BCD format
        MNU OFFSET(8) NUMBITS(4) [],

        /// Alarm A seconds mask
        MSK1 OFFSET(7) NUMBITS(1) [],

        /// Second tens in BCD format
        ST OFFSET(4) NUMBITS(3) [],

        /// Second units in BCD format
        SU OFFSET(0) NUMBITS(4) []
    ],

    /// Alarm A Sub-second Register (ALRMASSR)
    ALRMASSR [
        /// Clear flag in binary mode
        SSCLR OFFSET(31) NUMBITS(1) [],

        /// Mask the most-significant bits starting at this bit
        MASKSS OFFSET(24) NUMBITS(6) [],

        /// Sub-second value
        SS OFFSET(0) NUMBITS(15) []
    ],

    /// Alarm B Register (ALRMBR)
    ALRMBR [
        /// Alarm B date mask
        MSK4 OFFSET(31) NUMBITS(1) [],

        /// Week day selection
        WDSEL OFFSET(30) NUMBITS(1) [],

        /// Date tens in BCD format
        DT OFFSET(28) NUMBITS(2) [],

        /// Date units or day in BCD format
        DU OFFSET(24) NUMBITS(4) [],

        /// Alarm B hours mask
        MSK3 OFFSET(23) NUMBITS(1) [],

        /// AM/PM notation
        PM OFFSET(22) NUMBITS(1) [],

        /// Hour tens in BCD format
        HT OFFSET(20) NUMBITS(2) [],

        /// Hour units in BCD format
        HU OFFSET(16) NUMBITS(4) [],

        /// Alarm B minutes mask
        MSK2 OFFSET(15) NUMBITS(1) [],

        /// Minute tens in BCD format
        MNT OFFSET(12) NUMBITS(3) [],

        /// Minute units in BCD format
        MNU OFFSET(8) NUMBITS(4) [],

        /// Alarm B seconds mask
        MSK1 OFFSET(7) NUMBITS(1) [],

        /// Second tens in BCD format
        ST OFFSET(4) NUMBITS(3) [],

        /// Second units in BCD format
        SU OFFSET(0) NUMBITS(4) []
    ],

    /// Alarm B Sub-second Register (ALRMBSSR)
    ALRMBSSR [
        /// Clear flag in binary mode
        SSCLR OFFSET(31) NUMBITS(1) [],

        /// Mask the most-significant bits starting at this bit
        MASKSS OFFSET(24) NUMBITS(6) [],

        /// Sub-second value
        SS OFFSET(0) NUMBITS(15) []
    ],

    /// Status Register (SR)
    SR [
        /// Internal timestamp flag
        ITSF OFFSET(5) NUMBITS(1) [],

        /// Timestamp overflow flag
        TSOVF OFFSET(4) NUMBITS(1) [],

        /// Timestamp flag
        TSF OFFSET(3) NUMBITS(1) [],

        /// Wakeup timer flag
        WUTF OFFSET(2) NUMBITS(1) [],

        /// Alarm B flag
        ALRBF OFFSET(1) NUMBITS(1) [],

        /// Alarm A flag
        ALRAF OFFSET(0) NUMBITS(1) []
    ],

    /// Masked Interrupt Status Register (MISR)
    MISR [
        /// Internal timestamp masked flag
        ITSMF OFFSET(5) NUMBITS(1) [],

        /// Timestamp overflow masked flag
        TSOVMF OFFSET(4) NUMBITS(1) [],

        /// Timestamp masked flag
        TSMF OFFSET(3) NUMBITS(1) [],

        /// Wakeup timer masked flag
        WUTMF OFFSET(2) NUMBITS(1) [],

        /// Alarm B masked flag
        ALRBMF OFFSET(1) NUMBITS(1) [],

        /// Alarm A masked flag
        ALRAMF OFFSET(0) NUMBITS(1) []
    ],

    /// Status Clear Register (SCR)
    SCR [
        /// Clear internal timestamp flag
        CITSF OFFSET(5) NUMBITS(1) [],

        /// Clear timestamp overflow flag
        CTSOVF OFFSET(4) NUMBITS(1) [],

        /// Clear timestamp flag
        CTSF OFFSET(3) NUMBITS(1) [],

        /// Clear wakeup timer flag
        CWUTF OFFSET(2) NUMBITS(1) [],

        /// Clear alarm B flag
        CALRBF OFFSET(1) NUMBITS(1) [],

        /// Clear alarm A flag
        CALRAF OFFSET(0) NUMBITS(1) []
    ]
];

/// Write protection key values
pub const WPR_KEY1: u8 = 0xCA;
pub const WPR_KEY2: u8 = 0x53;

// ============================================================================
// Alarm Data Structures
// ============================================================================

/// Alarm mask configuration
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct AlarmMask {
    /// If true, seconds field is ignored (alarm triggers every second when all masks set)
    pub mask_seconds: bool,
    /// If true, minutes field is ignored
    pub mask_minutes: bool,
    /// If true, hours field is ignored
    pub mask_hours: bool,
    /// If true, date/day field is ignored
    pub mask_date: bool,
}

/// Alarm time configuration
#[derive(Clone, Copy, Debug)]
pub struct AlarmTime {
    /// Hour (0-23)
    pub hour: u8,
    /// Minute (0-59)
    pub minute: u8,
    /// Seconds (0-59)
    pub seconds: u8,
    /// Day of month (1-31) or day of week (1-7) depending on weekday_select
    pub day: u8,
    /// If true, day field represents day of week; if false, day of month
    pub weekday_select: bool,
    /// Mask configuration
    pub mask: AlarmMask,
}

/// Which alarm to configure
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AlarmId {
    /// Alarm A
    AlarmA,
    /// Alarm B
    AlarmB,
}

// ============================================================================
// Client
// ============================================================================

/// Client trait for alarm callbacks
pub trait RtcAlarmClient {
    /// Called when an alarm fires
    ///
    /// # Arguments
    /// * `alarm` - Which alarm triggered (A or B)
    fn alarm_fired(&self, alarm: AlarmId);
}

/// Client trait for wakeup timer callbacks
pub trait RtcWakeupClient {
    /// Called when the wakeup timer expires
    fn wakeup_fired(&self);
}

// ============================================================================
// Wakeup Timer Configuration
// ============================================================================

/// Wakeup timer clock source selection
///
/// The wakeup timer can use different clock sources, providing various
/// timing ranges from sub-millisecond to hours.
///
/// | Clock Source    | Frequency | Min Period | Max Period  |
/// |-----------------|-----------|------------|-------------|
/// | RtcDiv16        | ~2 kHz    | 0.5 ms     | 32.8 s      |
/// | RtcDiv8         | ~4 kHz    | 0.25 ms    | 16.4 s      |
/// | RtcDiv4         | ~8 kHz    | 0.125 ms   | 8.2 s       |
/// | RtcDiv2         | ~16 kHz   | 0.0625 ms  | 4.1 s       |
/// | CkSpre          | 1 Hz      | 1 s        | 18.2 hours  |
/// | CkSpreExtended  | 1 Hz      | 1 s        | 36.4 hours  |
///
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WakeupClockSource {
    /// RTC clock divided by 16 (~2 kHz for 32 kHz RTC clock)
    RtcDiv16,
    /// RTC clock divided by 8 (~4 kHz)
    RtcDiv8,
    /// RTC clock divided by 4 (~8 kHz)
    RtcDiv4,
    /// RTC clock divided by 2 (~16 kHz)
    RtcDiv2,
    /// 1 Hz clock (ck_spre)
    CkSpre,
    /// 1 Hz clock with extended range (WUT[16] added to counter)
    CkSpreExtended,
}

/// Default prescaler values for LSI clock (~32 kHz)
/// f_ck_spre = 32000 / ((127 + 1) × (255 + 1)) = 32000 / 32768 ≈ 1 Hz
pub const PREDIV_A_DEFAULT: u8 = 127;
pub const PREDIV_S_DEFAULT: u16 = 255;

/// Get a reference to the RTC registers
pub fn get_rtc_registers() -> StaticRef<RtcRegisters> {
    RTC_BASE
}

// ============================================================================
// BCD Conversion Functions
// ============================================================================

/// Convert a binary value (0-99) to BCD format.
///
/// BCD (Binary-Coded Decimal) encodes each decimal digit in 4 bits.
/// For example: 42 decimal = 0x42 BCD (4 in upper nibble, 2 in lower nibble)
///
/// # Arguments
/// * `val` - Binary value in range 0-99
///
/// # Returns
/// BCD-encoded value where upper nibble contains tens digit and lower nibble
/// contains units digit.
///
pub fn binary_to_bcd(val: u8) -> u8 {
    debug_assert!(
        val <= 99,
        "binary_to_bcd: value {} exceeds maximum of 99",
        val
    );
    ((val / 10) << 4) | (val % 10)
}

/// Convert a BCD value to binary format.
///
/// BCD (Binary-Coded Decimal) encodes each decimal digit in 4 bits.
/// For example: 0x42 BCD = 42 decimal (4 tens + 2 units)
///
/// # Arguments
/// * `bcd` - BCD-encoded value where upper nibble contains tens digit (0-9)
///   and lower nibble contains units digit (0-9)
///
/// # Returns
/// * `Some(binary_value)` - If the BCD value is valid (both nibbles 0-9)
/// * `None` - If either nibble is > 9 (invalid BCD)
///
pub fn bcd_to_binary(bcd: u8) -> Option<u8> {
    let tens = bcd >> 4;
    let units = bcd & 0x0F;

    // Validate that both nibbles are valid BCD digits (0-9)
    if tens > 9 || units > 9 {
        return None;
    }

    Some(tens * 10 + units)
}

// ============================================================================
// DateTime Validation
// ============================================================================

/// Validate a DateTimeValues struct for RTC compatibility.
///
/// This function checks that all fields in the DateTimeValues struct are
/// within valid ranges for the STM32WLE5x RTC hardware:
///
/// - Year: 0-99 (RTC stores only 2-digit year, representing 2000-2099)
/// - Month: Already validated by the Month enum type
/// - Day: 1-31 (basic range check; does not validate against specific month)
/// - Day of week: Already validated by the DayOfWeek enum type
/// - Hour: 0-23
/// - Minute: 0-59
/// - Seconds: 0-59
///
/// # Arguments
/// * `dt` - Reference to the DateTimeValues to validate
///
/// # Returns
/// * `true` - All fields are within valid ranges
/// * `false` - One or more fields are out of range
///
pub fn is_valid_datetime(dt: &DateTimeValues) -> bool {
    // Validate year: RTC stores 2-digit year (0-99), representing 2000-2099
    if dt.year < 2000 || dt.year > 2099 {
        return false;
    }

    // Validate day: 1-31
    if dt.day < 1 || dt.day > 31 {
        return false;
    }

    // Validate hour: 0-23
    if dt.hour > 23 {
        return false;
    }

    // Validate minute: 0-59
    if dt.minute > 59 {
        return false;
    }

    // Validate seconds: 0-59
    if dt.seconds > 59 {
        return false;
    }
    true
}

/// Validate an AlarmTime struct for RTC alarm configuration.
///
/// This function checks that all fields in the AlarmTime struct are
/// within valid ranges for the STM32WLE5x RTC alarm hardware:
///
/// - Hour: 0-23
/// - Minute: 0-59
/// - Seconds: 0-59
/// - Day: 1-31 (when weekday_select is false, representing day of month)
/// - Day: 1-7 (when weekday_select is true, representing day of week)
///
/// # Arguments
/// * `alarm_time` - Reference to the AlarmTime to validate
///
/// # Returns
/// * `true` - All fields are within valid ranges
/// * `false` - One or more fields are out of range
///
pub fn is_valid_alarm_time(alarm_time: &AlarmTime) -> bool {
    // Validate hour: 0-23
    if alarm_time.hour > 23 {
        return false;
    }

    // Validate minute: 0-59
    if alarm_time.minute > 59 {
        return false;
    }

    // Validate seconds: 0-59
    if alarm_time.seconds > 59 {
        return false;
    }

    // Validate day based on weekday_select mode
    if alarm_time.weekday_select {
        // Weekday mode: day represents day of week (1-7)
        // 1 = Monday, 2 = Tuesday, ..., 7 = Sunday
        if alarm_time.day < 1 || alarm_time.day > 7 {
            return false;
        }
    } else {
        // Date mode: day represents day of month (1-31)
        if alarm_time.day < 1 || alarm_time.day > 31 {
            return false;
        }
    }

    true
}

// ============================================================================
// RTC Clock Wrapper
// ============================================================================

/// RTC Clock wrapper struct
///
/// The RTC uses a separate clock domain from the main system clocks,
/// typically sourced from LSI (~32 kHz), LSE (32.768 kHz), or HSE/32.
///
pub struct RtcClock<'a>(phclk::PeripheralClock<'a>);

impl<'a> RtcClock<'a> {
    /// Create a new RtcClock instance
    ///
    /// # Arguments
    /// * `clocks` - Reference to the chip's clock infrastructure
    ///
    /// # Returns
    /// A new RtcClock instance configured to manage the RTC peripheral clock
    pub const fn new(clocks: &'a dyn Stm32wle5xxClocks) -> Self {
        Self(phclk::PeripheralClock::new(
            phclk::PeripheralClockType::RTC,
            clocks,
        ))
    }
}

impl ClockInterface for RtcClock<'_> {
    /// Check if the RTC kernel clock is enabled
    ///
    /// # Returns
    /// `true` if the RTC kernel clock is enabled, `false` otherwise
    fn is_enabled(&self) -> bool {
        self.0.is_enabled()
    }

    /// Enable the RTC kernel clock
    ///
    /// This method enables the RTC kernel clock using the LSI clock source
    /// by default. The LSI oscillator will be enabled and stabilized before
    /// the RTC clock is enabled.
    ///
    /// Note: To use a different clock source (LSE or HSE/32), use the
    /// RCC methods directly.
    fn enable(&self) {
        self.0.enable();
    }

    /// Disable the RTC kernel clock
    ///
    /// This method disables the RTC kernel clock. Note that this only
    /// disables the RTC clock, not the clock source (LSI/LSE/HSE).
    fn disable(&self) {
        self.0.disable();
    }
}

// ============================================================================
// RTC Driver Struct
// ============================================================================

/// Deferred call task types for asynchronous operations
///
/// The RTC driver uses deferred calls to notify clients when
/// get/set operations complete. This enum tracks which operation
/// is pending.
#[derive(Clone, Copy)]
enum DeferredCallTask {
    /// A get_date_time operation is pending
    Get,
    /// A set_date_time operation is pending
    Set,
}

/// RTC operation state tracking
#[derive(Clone, Copy, PartialEq)]
pub enum RtcStatus {
    /// No operation in progress, RTC is idle
    Idle,
    /// A get_date_time operation is in progress
    GettingTime,
    /// A set_date_time operation is in progress
    SettingTime,
}

/// Real-Time Clock (RTC) driver
pub struct Rtc<'a> {
    /// Reference to the memory-mapped RTC registers
    registers: StaticRef<RtcRegisters>,

    /// RTC clock wrapper for clock management
    clock: RtcClock<'a>,

    /// Client to notify when date/time operations complete
    client: OptionalCell<&'a dyn DateTimeClient>,

    /// Cached date/time value for deferred callback
    time: Cell<DateTimeValues>,

    /// Deferred call for asynchronous operation completion
    deferred_call: DeferredCall,

    /// Tracks which deferred call task is pending
    deferred_call_task: OptionalCell<DeferredCallTask>,

    /// Current operation state for tracking pending operations
    status: Cell<RtcStatus>,

    // ========================================================================
    // Alarm, and Wakeup Timer Fields
    // ========================================================================
    /// Client for alarm callbacks
    alarm_client: OptionalCell<&'a dyn RtcAlarmClient>,

    /// Client for wakeup timer callbacks
    wakeup_client: OptionalCell<&'a dyn RtcWakeupClient>,
}

#[allow(clippy::elidable_lifetime_names)]
impl<'a> Rtc<'a> {
    /// Create a new RTC driver instance
    ///
    /// # Arguments
    /// * `clocks` - Reference to the chip's clock infrastructure
    ///
    /// # Returns
    /// A new Rtc instance with default values:
    /// - No client registered
    /// - Time initialized to epoch (1970-01-01 00:00:00, Thursday)
    /// - No pending deferred call task
    /// - Status set to Idle
    ///
    pub fn new(clocks: &'a dyn Stm32wle5xxClocks) -> Self {
        Self {
            registers: RTC_BASE,
            clock: RtcClock::new(clocks),
            client: OptionalCell::empty(),
            time: Cell::new(DateTimeValues {
                year: 1970,
                month: Month::January,
                day: 1,
                day_of_week: DayOfWeek::Thursday, // January 1, 1970 was a Thursday
                hour: 0,
                minute: 0,
                seconds: 0,
            }),
            deferred_call: DeferredCall::new(),
            deferred_call_task: OptionalCell::empty(),
            status: Cell::new(RtcStatus::Idle),
            // Initialize alarm, and wakeup timer fields
            alarm_client: OptionalCell::empty(),
            wakeup_client: OptionalCell::empty(),
        }
    }

    /// Check if the RTC clock is enabled
    ///
    /// # Returns
    /// `true` if the RTC kernel clock is enabled, `false` otherwise
    pub fn is_enabled_clock(&self) -> bool {
        self.clock.is_enabled()
    }

    /// Enable the RTC clock
    pub fn enable_clock(&self) {
        self.clock.enable();
    }

    /// Disable the RTC clock
    pub fn disable_clock(&self) {
        self.clock.disable();
    }

    /// Get the current operation status
    ///
    /// Returns the current state of the RTC driver, indicating whether
    /// an operation is in progress or the driver is idle.
    ///
    /// # Returns
    /// * `RtcStatus::Idle` - No operation in progress
    /// * `RtcStatus::GettingTime` - A get_date_time operation is in progress
    /// * `RtcStatus::SettingTime` - A set_date_time operation is in progress
    ///
    pub fn get_status(&self) -> RtcStatus {
        self.status.get()
    }

    // ========================================================================
    // RTC Initialization
    // ========================================================================

    /// Initialize the RTC peripheral with default settings
    ///
    /// This method performs the complete RTC initialization sequence:
    /// 1. Enables the RTC clock (using LSI clock source by default)
    /// 2. Configures the prescaler for 1 Hz calendar clock
    /// 3. Sets the initial date/time to the RTC epoch (2000-01-01 00:00:00)
    ///
    /// - The prescaler formula is: f_ck_spre = f_rtcclk / ((PREDIV_A + 1) × (PREDIV_S + 1))
    /// - For LSI (~32 kHz): PREDIV_A = 127, PREDIV_S = 255 → ~1 Hz
    ///
    /// # Returns
    ///
    /// * `Ok(())` - RTC initialized successfully
    /// * `Err(ErrorCode::FAIL)` - Failed to enter initialization mode
    ///
    pub fn rtc_init(&self) -> Result<(), ErrorCode> {
        // Enable RTC clock
        self.enable_clock();

        // Unlock write protection to modify RTC registers
        self.unlock_write_protection();

        // Enter initialization mode
        let init_result = self.enter_init_mode();
        if init_result.is_err() {
            self.lock_write_protection();
            return init_result;
        }

        // Configure prescaler for 1 Hz calendar clock
        // For LSI (~32 kHz): PREDIV_A = 127, PREDIV_S = 255
        // f_ck_spre = 32000 / ((127 + 1) × (255 + 1)) = 32000 / 32768 ≈ 1 Hz
        self.registers.prer.modify(
            PRER::PREDIV_A.val(PREDIV_A_DEFAULT as u32)
                + PRER::PREDIV_S.val(PREDIV_S_DEFAULT as u32),
        );

        // Set initial date/time to RTC epoch (2000-01-01 00:00:00, Saturday)
        // Write Time Register (TR) - 00:00:00
        self.registers.tr.modify(
            TR::HT.val(0)
                + TR::HU.val(0)
                + TR::MNT.val(0)
                + TR::MNU.val(0)
                + TR::ST.val(0)
                + TR::SU.val(0)
                + TR::PM::CLEAR, // 24-hour format
        );

        // Write Date Register (DR) - 2000-01-01, Saturday
        // Year: 00 (BCD), Month: 01 (BCD), Day: 01 (BCD), Weekday: 6 (Saturday)
        self.registers.dr.modify(
            DR::YT.val(0)
                + DR::YU.val(0)
                + DR::WDU.val(6)
                + DR::MT.val(0)
                + DR::MU.val(1)
                + DR::DT.val(0)
                + DR::DU.val(1),
        );

        // Exit initialization mode and start calendar counters
        self.exit_init_mode();

        // Lock write protection
        self.lock_write_protection();

        Ok(())
    }

    // ========================================================================
    // Write Protection Helpers
    // ========================================================================

    /// Unlock RTC write protection
    pub(crate) fn unlock_write_protection(&self) {
        // To unlock write protection, write 0xCA then 0x53 to WPR register
        self.registers.wpr.set(WPR_KEY1 as u32);
        self.registers.wpr.set(WPR_KEY2 as u32);
    }

    /// Lock RTC write protection
    pub(crate) fn lock_write_protection(&self) {
        // Writing any wrong key (not 0xCA or 0x53) reactivates write protection
        // 0xFF the invalid key
        self.registers.wpr.set(0xFF);
    }

    // ========================================================================
    // Initialization Mode Helpers
    // ========================================================================

    /// Timeout iterations for entering initialization mode
    const INIT_MODE_TIMEOUT: usize = 100;

    /// Enter RTC initialization mode
    pub(crate) fn enter_init_mode(&self) -> Result<(), ErrorCode> {
        // Set the INIT bit to enter initialization mode
        self.registers.icsr.modify(ICSR::INIT::SET);

        // Wait for INITF flag to be set, indicating initialization mode is active
        for _ in 0..Self::INIT_MODE_TIMEOUT {
            if self.registers.icsr.is_set(ICSR::INITF) {
                return Ok(());
            }
        }

        // Timeout: failed to enter initialization mode
        // Clear the INIT bit before returning error
        self.registers.icsr.modify(ICSR::INIT::CLEAR);
        Err(ErrorCode::FAIL)
    }

    /// Exit RTC initialization mode
    pub(crate) fn exit_init_mode(&self) {
        // Clear the INIT bit to exit initialization mode
        self.registers.icsr.modify(ICSR::INIT::CLEAR);
    }
}

impl DeferredCallClient for Rtc<'_> {
    /// Handle a deferred call
    fn handle_deferred_call(&self) {
        self.deferred_call_task.take().map(|task| {
            // Reset status to Idle before invoking callback
            self.status.set(RtcStatus::Idle);

            match task {
                DeferredCallTask::Get => {
                    self.client
                        .map(|client| client.get_date_time_done(Ok(self.time.get())));
                }
                DeferredCallTask::Set => {
                    self.client.map(|client| client.set_date_time_done(Ok(())));
                }
            }
        });
    }

    /// Register this driver with the deferred call infrastructure
    fn register(&'static self) {
        self.deferred_call.register(self);
    }
}

// ============================================================================
// DateTime HIL Implementation
// ============================================================================

use kernel::hil::date_time::DateTime;

#[allow(clippy::elidable_lifetime_names)]
impl<'a> Rtc<'a> {
    // ========================================================================
    // Month/DayOfWeek Conversion Helpers
    // ========================================================================

    /// Convert Month enum to numeric value (1-12)
    ///
    /// The RTC hardware uses 1-based month numbering:
    /// January = 1, February = 2, ..., December = 12
    fn month_to_u8(month: Month) -> u8 {
        match month {
            Month::January => 1,
            Month::February => 2,
            Month::March => 3,
            Month::April => 4,
            Month::May => 5,
            Month::June => 6,
            Month::July => 7,
            Month::August => 8,
            Month::September => 9,
            Month::October => 10,
            Month::November => 11,
            Month::December => 12,
        }
    }

    /// Convert numeric month value (1-12) to Month enum
    ///
    /// # Arguments
    /// * `month` - Month number (1-12)
    ///
    /// # Returns
    /// * `Ok(Month)` - Valid month enum value
    /// * `Err(ErrorCode::INVAL)` - Invalid month number
    fn month_from_u8(month: u8) -> Result<Month, ErrorCode> {
        match month {
            1 => Ok(Month::January),
            2 => Ok(Month::February),
            3 => Ok(Month::March),
            4 => Ok(Month::April),
            5 => Ok(Month::May),
            6 => Ok(Month::June),
            7 => Ok(Month::July),
            8 => Ok(Month::August),
            9 => Ok(Month::September),
            10 => Ok(Month::October),
            11 => Ok(Month::November),
            12 => Ok(Month::December),
            _ => Err(ErrorCode::INVAL),
        }
    }

    /// Convert DayOfWeek enum to numeric value (1-7)
    fn day_of_week_to_u8(dow: DayOfWeek) -> u8 {
        match dow {
            DayOfWeek::Monday => 1,
            DayOfWeek::Tuesday => 2,
            DayOfWeek::Wednesday => 3,
            DayOfWeek::Thursday => 4,
            DayOfWeek::Friday => 5,
            DayOfWeek::Saturday => 6,
            DayOfWeek::Sunday => 7,
        }
    }

    /// Convert numeric day of week value (1-7) to DayOfWeek enum
    ///
    /// # Arguments
    /// * `dow` - Day of week number (1-7, where 1=Monday, 7=Sunday)
    ///
    /// # Returns
    /// * `Ok(DayOfWeek)` - Valid day of week enum value
    /// * `Err(ErrorCode::INVAL)` - Invalid day of week number
    fn day_of_week_from_u8(dow: u8) -> Result<DayOfWeek, ErrorCode> {
        match dow {
            1 => Ok(DayOfWeek::Monday),
            2 => Ok(DayOfWeek::Tuesday),
            3 => Ok(DayOfWeek::Wednesday),
            4 => Ok(DayOfWeek::Thursday),
            5 => Ok(DayOfWeek::Friday),
            6 => Ok(DayOfWeek::Saturday),
            7 => Ok(DayOfWeek::Sunday),
            _ => Err(ErrorCode::INVAL),
        }
    }

    // ========================================================================
    // Shadow Register Synchronization
    // ========================================================================

    /// Timeout iterations for shadow register synchronization
    const RSF_TIMEOUT: usize = 100;

    /// Wait for shadow register synchronization
    fn wait_for_rsf(&self) -> Result<(), ErrorCode> {
        // Clear RSF flag first by writing 0 to it
        self.registers.icsr.modify(ICSR::RSF::CLEAR);

        // Wait for RSF to be set by hardware
        for _ in 0..Self::RSF_TIMEOUT {
            if self.registers.icsr.is_set(ICSR::RSF) {
                return Ok(());
            }
        }

        Err(ErrorCode::FAIL)
    }
}

impl<'a> DateTime<'a> for Rtc<'a> {
    /// Get the current date and time from the RTC
    ///
    /// This method reads the current date and time from the RTC hardware
    /// registers and schedules a deferred callback to notify the client.
    ///
    /// - Read TR register first, then DR register for consistency
    /// - Wait for RSF flag to ensure shadow registers are synchronized
    /// - Convert BCD values to binary DateTimeValues
    ///
    /// # Returns
    /// * `Ok(())` - Operation started, callback will be invoked
    /// * `Err(ErrorCode::BUSY)` - Another operation is in progress
    /// * `Err(ErrorCode::FAIL)` - Shadow register synchronization timeout
    ///
    fn get_date_time(&self) -> Result<(), ErrorCode> {
        // Check if an operation is already in progress
        match self.status.get() {
            RtcStatus::Idle => {
                // No operation in progress, proceed
            }
            RtcStatus::GettingTime | RtcStatus::SettingTime => {
                // Operation in progress, return BUSY
                return Err(ErrorCode::BUSY);
            }
        }

        // Set status to GettingTime
        self.status.set(RtcStatus::GettingTime);

        // Wait for shadow register synchronization
        if let Err(e) = self.wait_for_rsf() {
            self.status.set(RtcStatus::Idle);
            return Err(e);
        }

        // Read TR register first, then DR register for consistency
        let tr = self.registers.tr.get();
        let dr = self.registers.dr.get();

        // Extract time fields from TR register (BCD format)
        let hour_bcd = ((tr >> 20) & 0x3) as u8 * 16 + ((tr >> 16) & 0xF) as u8;
        let minute_bcd = ((tr >> 12) & 0x7) as u8 * 16 + ((tr >> 8) & 0xF) as u8;
        let seconds_bcd = ((tr >> 4) & 0x7) as u8 * 16 + (tr & 0xF) as u8;

        // Extract date fields from DR register (BCD format)
        let year_bcd = ((dr >> 20) & 0xF) as u8 * 16 + ((dr >> 16) & 0xF) as u8;
        let month_bcd = ((dr >> 12) & 0x1) as u8 * 16 + ((dr >> 8) & 0xF) as u8;
        let day_bcd = ((dr >> 4) & 0x3) as u8 * 16 + (dr & 0xF) as u8;
        let dow = ((dr >> 13) & 0x7) as u8;

        // Convert BCD to binary
        let hour = bcd_to_binary(hour_bcd).unwrap_or(0);
        let minute = bcd_to_binary(minute_bcd).unwrap_or(0);
        let seconds = bcd_to_binary(seconds_bcd).unwrap_or(0);
        let year_offset = bcd_to_binary(year_bcd).unwrap_or(0);
        let month_num = bcd_to_binary(month_bcd).unwrap_or(1);
        let day = bcd_to_binary(day_bcd).unwrap_or(1);

        // Convert month and day of week to enum types
        let month = Self::month_from_u8(month_num).unwrap_or(Month::January);
        let day_of_week = Self::day_of_week_from_u8(dow).unwrap_or(DayOfWeek::Monday);

        // Build DateTimeValues struct
        // RTC stores 2-digit year (00-99), representing 2000-2099
        let datetime = DateTimeValues {
            year: 2000 + year_offset as u16,
            month,
            day,
            day_of_week,
            hour,
            minute,
            seconds,
        };

        // Store the datetime for the deferred callback
        self.time.set(datetime);

        // Schedule deferred callback
        self.deferred_call_task.set(DeferredCallTask::Get);
        self.deferred_call.set();

        Ok(())
    }

    /// Set the current date and time in the RTC
    ///
    /// This method writes the provided date and time to the RTC hardware
    /// registers and schedules a deferred callback to notify the client.
    ///
    /// - Validate input datetime
    /// - Unlock write protection
    /// - Enter initialization mode
    /// - Write TR and DR registers in BCD format
    /// - Exit initialization mode
    /// - Lock write protection
    ///
    /// # Arguments
    /// * `date_time` - The date and time values to set
    ///
    /// # Returns
    /// * `Ok(())` - Operation started, callback will be invoked
    /// * `Err(ErrorCode::BUSY)` - Another operation is in progress
    /// * `Err(ErrorCode::INVAL)` - Invalid date/time values
    /// * `Err(ErrorCode::FAIL)` - Failed to enter initialization mode
    ///
    fn set_date_time(&self, date_time: DateTimeValues) -> Result<(), ErrorCode> {
        // Check if an operation is already in progress
        match self.status.get() {
            RtcStatus::Idle => {
                // No operation in progress, proceed
            }
            RtcStatus::GettingTime | RtcStatus::SettingTime => {
                // Operation in progress, return BUSY
                return Err(ErrorCode::BUSY);
            }
        }

        // Validate input datetime
        if !is_valid_datetime(&date_time) {
            return Err(ErrorCode::INVAL);
        }

        // Set status to SettingTime
        self.status.set(RtcStatus::SettingTime);

        // Unlock write protection
        self.unlock_write_protection();

        // Enter initialization mode
        let init_result = self.enter_init_mode();
        if init_result.is_err() {
            self.lock_write_protection();
            self.status.set(RtcStatus::Idle);
            return init_result;
        }

        // Convert values to BCD format and write to registers
        let year_offset = (date_time.year - 2000) as u8; // RTC stores 2-digit year
        let month_num = Self::month_to_u8(date_time.month);
        let dow_num = Self::day_of_week_to_u8(date_time.day_of_week);

        // Write Time Register (TR) in BCD format
        let hour_bcd = binary_to_bcd(date_time.hour);
        let minute_bcd = binary_to_bcd(date_time.minute);
        let seconds_bcd = binary_to_bcd(date_time.seconds);

        self.registers.tr.modify(
            TR::HT.val((hour_bcd >> 4) as u32)
                + TR::HU.val((hour_bcd & 0xF) as u32)
                + TR::MNT.val((minute_bcd >> 4) as u32)
                + TR::MNU.val((minute_bcd & 0xF) as u32)
                + TR::ST.val((seconds_bcd >> 4) as u32)
                + TR::SU.val((seconds_bcd & 0xF) as u32)
                + TR::PM::CLEAR, // 24-hour format
        );

        // Write Date Register (DR) in BCD format
        let year_bcd = binary_to_bcd(year_offset);
        let month_bcd = binary_to_bcd(month_num);
        let day_bcd = binary_to_bcd(date_time.day);

        self.registers.dr.modify(
            DR::YT.val((year_bcd >> 4) as u32)
                + DR::YU.val((year_bcd & 0xF) as u32)
                + DR::WDU.val(dow_num as u32)
                + DR::MT.val((month_bcd >> 4) as u32)
                + DR::MU.val((month_bcd & 0xF) as u32)
                + DR::DT.val((day_bcd >> 4) as u32)
                + DR::DU.val((day_bcd & 0xF) as u32),
        );

        // Exit initialization mode
        self.exit_init_mode();

        // Lock write protection
        self.lock_write_protection();

        // Schedule deferred callback
        self.deferred_call_task.set(DeferredCallTask::Set);
        self.deferred_call.set();

        Ok(())
    }

    /// Set the client to receive date/time operation callbacks
    ///
    /// # Arguments
    /// * `client` - Reference to the DateTimeClient implementation
    ///
    fn set_client(&self, client: &'a dyn DateTimeClient) {
        self.client.set(client);
    }
}

// ============================================================================
// Wakeup Timer Implementation
// ============================================================================

#[allow(clippy::elidable_lifetime_names)]
impl<'a> Rtc<'a> {
    /// Timeout iterations for wakeup timer write flag
    ///
    /// This value was chosen based on similar timeout patterns in the STM32WLE5x
    /// clock drivers. The WUTWF flag should be set quickly once WUTE is cleared.
    const WUTWF_TIMEOUT: usize = 100;

    /// Configure and enable the wakeup timer
    ///
    /// This method configures the wakeup timer with the specified reload value
    /// and clock source, then enables the timer and its interrupt.
    ///
    /// 1. Disable wakeup timer (clear WUTE bit)
    /// 2. Wait for WUTWF flag to be set (indicates WUTR can be written)
    /// 3. Configure WUCKSEL bits for clock source
    /// 4. Write reload value to WUTR register
    /// 5. Enable wakeup timer (set WUTE bit)
    /// 6. Enable wakeup interrupt (set WUTIE bit)
    ///
    /// # Arguments
    /// * `reload` - Wakeup timer reload value (0-65535)
    /// * `clock_source` - Clock source for the wakeup timer
    ///
    /// # Returns
    /// * `Ok(())` - Wakeup timer configured successfully
    /// * `Err(ErrorCode::BUSY)` - Timeout waiting for WUTWF flag
    ///
    pub fn set_wakeup_timer(
        &self,
        reload: u16,
        clock_source: WakeupClockSource,
    ) -> Result<(), ErrorCode> {
        // Unlock write protection to modify RTC registers
        self.unlock_write_protection();

        // Disable wakeup timer first (clear WUTE bit)
        self.registers.cr.modify(CR::WUTE::CLEAR);

        // Wait for WUTWF flag to be set
        let mut wutwf_ready = false;
        for _ in 0..Self::WUTWF_TIMEOUT {
            if self.registers.icsr.is_set(ICSR::WUTWF) {
                wutwf_ready = true;
                break;
            }
        }

        if !wutwf_ready {
            // Timeout waiting for WUTWF flag
            self.lock_write_protection();
            return Err(ErrorCode::BUSY);
        }

        // Configure WUCKSEL bits for clock source
        let wucksel_val = match clock_source {
            WakeupClockSource::RtcDiv16 => CR::WUCKSEL::RtcDiv16,
            WakeupClockSource::RtcDiv8 => CR::WUCKSEL::RtcDiv8,
            WakeupClockSource::RtcDiv4 => CR::WUCKSEL::RtcDiv4,
            WakeupClockSource::RtcDiv2 => CR::WUCKSEL::RtcDiv2,
            WakeupClockSource::CkSpre => CR::WUCKSEL::CkSpre,
            WakeupClockSource::CkSpreExtended => CR::WUCKSEL::CkSpreWithWut16,
        };
        self.registers.cr.modify(wucksel_val);

        // Write reload value to WUTR register
        self.registers.wutr.modify(WUTR::WUT.val(reload as u32));

        // Enable wakeup timer (set WUTE bit)
        self.registers.cr.modify(CR::WUTE::SET);

        // Enable wakeup interrupt (set WUTIE bit)
        self.registers.cr.modify(CR::WUTIE::SET);

        // Lock write protection
        self.lock_write_protection();

        Ok(())
    }

    /// Disable the wakeup timer
    ///
    /// This method disables the wakeup timer and its interrupt.
    ///
    /// - Clear WUTE bit in CR register to disable the timer
    /// - Clear WUTIE bit in CR register to disable the interrupt
    ///
    pub fn disable_wakeup_timer(&self) {
        // Unlock write protection to modify RTC registers
        self.unlock_write_protection();

        // Clear WUTE bit to disable wakeup timer
        self.registers.cr.modify(CR::WUTE::CLEAR);

        // Clear WUTIE bit to disable wakeup interrupt
        self.registers.cr.modify(CR::WUTIE::CLEAR);

        // Lock write protection
        self.lock_write_protection();
    }

    /// Check if the wakeup timer is enabled
    ///
    /// # Returns
    /// `true` if the wakeup timer is enabled, `false` otherwise
    ///
    pub fn is_wakeup_enabled(&self) -> bool {
        self.registers.cr.is_set(CR::WUTE)
    }

    /// Set the wakeup timer client
    ///
    /// This method registers a client to receive wakeup timer callbacks.
    /// When the wakeup timer expires, the client's `wakeup_fired()` method
    /// will be called.
    ///
    /// # Arguments
    /// * `client` - Reference to the RtcWakeupClient implementation
    ///
    pub fn set_wakeup_client(&self, client: &'a dyn RtcWakeupClient) {
        self.wakeup_client.set(client);
    }

    // ========================================================================
    // Alarm Client Methods
    // ========================================================================

    /// Set the alarm client
    ///
    /// # Arguments
    /// * `client` - Reference to the RtcAlarmClient implementation
    ///
    pub fn set_alarm_client(&self, client: &'a dyn RtcAlarmClient) {
        self.alarm_client.set(client);
    }

    /// Configure and enable an alarm
    ///
    /// This method configures the specified alarm (A or B) with the provided
    /// time configuration and enables it.
    ///
    /// 1. Validate alarm time (return INVAL if invalid)
    /// 2. Disable alarm first (clear ALRAE/ALRBE)
    /// 3. Unlock write protection
    /// 4. Write alarm time to ALRMAR/ALRMBR in BCD format
    /// 5. Set mask bits (MSK1-MSK4) based on AlarmMask
    /// 6. Set WDSEL bit based on weekday_select
    /// 7. Enable alarm (set ALRAE/ALRBE)
    /// 8. Enable alarm interrupt (set ALRAIE/ALRBIE)
    /// 9. Lock write protection
    ///
    /// # Arguments
    /// * `alarm` - Which alarm to configure (A or B)
    /// * `time` - Alarm time configuration
    ///
    /// # Returns
    /// * `Ok(())` - Alarm configured successfully
    /// * `Err(ErrorCode::INVAL)` - Invalid time values
    ///
    pub fn set_alarm(&self, alarm: AlarmId, time: AlarmTime) -> Result<(), ErrorCode> {
        // Validate alarm time
        if !is_valid_alarm_time(&time) {
            return Err(ErrorCode::INVAL);
        }

        // Unlock write protection
        self.unlock_write_protection();

        // Disable alarm first (clear ALRAE/ALRBE)
        match alarm {
            AlarmId::AlarmA => {
                self.registers.cr.modify(CR::ALRAE::CLEAR);
                self.registers.cr.modify(CR::ALRAIE::CLEAR);
            }
            AlarmId::AlarmB => {
                self.registers.cr.modify(CR::ALRBE::CLEAR);
                self.registers.cr.modify(CR::ALRBIE::CLEAR);
            }
        }

        // Convert time values to BCD format
        let hour_bcd = binary_to_bcd(time.hour);
        let minute_bcd = binary_to_bcd(time.minute);
        let seconds_bcd = binary_to_bcd(time.seconds);
        let day_bcd = binary_to_bcd(time.day);

        // Build alarm register value with mask bits and WDSEL
        // Alarm register format (ALRMAR/ALRMBR):
        // Bit 31: MSK4 - Date mask
        // Bit 30: WDSEL - Week day selection
        // Bits 29:28: DT - Date tens (BCD)
        // Bits 27:24: DU - Date units (BCD)
        // Bit 23: MSK3 - Hours mask
        // Bit 22: PM - AM/PM (0 for 24-hour format)
        // Bits 21:20: HT - Hour tens (BCD)
        // Bits 19:16: HU - Hour units (BCD)
        // Bit 15: MSK2 - Minutes mask
        // Bits 14:12: MNT - Minute tens (BCD)
        // Bits 11:8: MNU - Minute units (BCD)
        // Bit 7: MSK1 - Seconds mask
        // Bits 6:4: ST - Second tens (BCD)
        // Bits 3:0: SU - Second units (BCD)

        match alarm {
            AlarmId::AlarmA => {
                // Write alarm time to ALRMAR register
                let alarm_a_val = ALRMAR::MSK4.val(u32::from(time.mask.mask_date))
                    + ALRMAR::MSK3.val(u32::from(time.mask.mask_hours))
                    + ALRMAR::MSK2.val(u32::from(time.mask.mask_minutes))
                    + ALRMAR::MSK1.val(u32::from(time.mask.mask_seconds))
                    + ALRMAR::WDSEL.val(u32::from(time.weekday_select))
                    + ALRMAR::DT.val((day_bcd >> 4) as u32)
                    + ALRMAR::DU.val((day_bcd & 0xF) as u32)
                    + ALRMAR::PM::CLEAR
                    + ALRMAR::HT.val((hour_bcd >> 4) as u32)
                    + ALRMAR::HU.val((hour_bcd & 0xF) as u32)
                    + ALRMAR::MNT.val((minute_bcd >> 4) as u32)
                    + ALRMAR::MNU.val((minute_bcd & 0xF) as u32)
                    + ALRMAR::ST.val((seconds_bcd >> 4) as u32)
                    + ALRMAR::SU.val((seconds_bcd & 0xF) as u32);

                self.registers.alrmar.modify(alarm_a_val);

                // Enable Alarm A (set ALRAE)
                self.registers.cr.modify(CR::ALRAE::SET);

                // Enable Alarm A interrupt (set ALRAIE)
                self.registers.cr.modify(CR::ALRAIE::SET);
            }
            AlarmId::AlarmB => {
                // Write alarm time to ALRMBR register
                let alarm_b_val = ALRMBR::MSK4.val(u32::from(time.mask.mask_date))
                    + ALRMBR::MSK3.val(u32::from(time.mask.mask_hours))
                    + ALRMBR::MSK2.val(u32::from(time.mask.mask_minutes))
                    + ALRMBR::MSK1.val(u32::from(time.mask.mask_seconds))
                    + ALRMBR::WDSEL.val(u32::from(time.weekday_select))
                    + ALRMBR::DT.val((day_bcd >> 4) as u32)
                    + ALRMBR::DU.val((day_bcd & 0xF) as u32)
                    + ALRMBR::PM::CLEAR
                    + ALRMBR::HT.val((hour_bcd >> 4) as u32)
                    + ALRMBR::HU.val((hour_bcd & 0xF) as u32)
                    + ALRMBR::MNT.val((minute_bcd >> 4) as u32)
                    + ALRMBR::MNU.val((minute_bcd & 0xF) as u32)
                    + ALRMBR::ST.val((seconds_bcd >> 4) as u32)
                    + ALRMBR::SU.val((seconds_bcd & 0xF) as u32);

                self.registers.alrmbr.modify(alarm_b_val);

                // Enable Alarm B (set ALRBE)
                self.registers.cr.modify(CR::ALRBE::SET);

                // Enable Alarm B interrupt (set ALRBIE)
                self.registers.cr.modify(CR::ALRBIE::SET);
            }
        }

        // Lock write protection
        self.lock_write_protection();

        Ok(())
    }

    /// Disable an alarm
    ///
    /// This method disables the specified alarm (A or B) and its interrupt.
    ///
    /// - Clear ALRAE/ALRBE bit in CR register to disable the alarm
    /// - Clear ALRAIE/ALRBIE bit in CR register to disable the interrupt
    ///
    /// # Arguments
    /// * `alarm` - Which alarm to disable (A or B)
    ///
    pub fn disable_alarm(&self, alarm: AlarmId) {
        // Unlock write protection to modify RTC registers
        self.unlock_write_protection();

        match alarm {
            AlarmId::AlarmA => {
                // Clear ALRAE bit to disable Alarm A
                self.registers.cr.modify(CR::ALRAE::CLEAR);
                // Clear ALRAIE bit to disable Alarm A interrupt
                self.registers.cr.modify(CR::ALRAIE::CLEAR);
            }
            AlarmId::AlarmB => {
                // Clear ALRBE bit to disable Alarm B
                self.registers.cr.modify(CR::ALRBE::CLEAR);
                // Clear ALRBIE bit to disable Alarm B interrupt
                self.registers.cr.modify(CR::ALRBIE::CLEAR);
            }
        }

        // Lock write protection
        self.lock_write_protection();
    }

    /// Check if an alarm is enabled
    ///
    /// # Arguments
    /// * `alarm` - Which alarm to check (A or B)
    ///
    /// # Returns
    /// `true` if the alarm is enabled, `false` otherwise
    ///
    pub fn is_alarm_enabled(&self, alarm: AlarmId) -> bool {
        match alarm {
            AlarmId::AlarmA => self.registers.cr.is_set(CR::ALRAE),
            AlarmId::AlarmB => self.registers.cr.is_set(CR::ALRBE),
        }
    }

    // ========================================================================
    // Interrupt Handler
    // ========================================================================

    /// Handle RTC interrupt
    ///
    /// This method should be called from the RTC interrupt handler.
    /// It checks the status register (SR) to determine the interrupt source(s)
    /// and invokes the appropriate client callbacks.
    ///
    /// - Read SR register to check interrupt sources
    /// - Handle each active interrupt source
    /// - Clear handled flags by writing to SCR register
    ///
    /// The following interrupt sources are handled:
    /// - WUTF (Wakeup Timer Flag): Invokes wakeup_client.wakeup_fired()
    /// - ALRAF (Alarm A Flag): Invokes alarm_client.alarm_fired(AlarmA)
    /// - ALRBF (Alarm B Flag): Invokes alarm_client.alarm_fired(AlarmB)
    ///
    pub fn handle_interrupt(&self) {
        // Read SR register to check interrupt sources
        let sr = self.registers.sr.get();

        // Handle WUTF (Wakeup Timer Flag)
        // This flag is set when the wakeup timer expires
        if sr & SR::WUTF::SET.value != 0 {
            // Clear WUTF flag by writing to SCR register
            self.registers.scr.write(SCR::CWUTF::SET);

            // Invoke wakeup client callback
            self.wakeup_client.map(|client| {
                client.wakeup_fired();
            });
        }

        // Handle ALRAF (Alarm A Flag)
        // This flag is set when Alarm A matches the current time
        if sr & SR::ALRAF::SET.value != 0 {
            // Clear ALRAF flag by writing to SCR register
            self.registers.scr.write(SCR::CALRAF::SET);

            // Invoke alarm client callback with AlarmA identifier
            self.alarm_client.map(|client| {
                client.alarm_fired(AlarmId::AlarmA);
            });
        }

        // Handle ALRBF (Alarm B Flag)
        // This flag is set when Alarm B matches the current time
        if sr & SR::ALRBF::SET.value != 0 {
            // Clear ALRBF flag by writing to SCR register
            self.registers.scr.write(SCR::CALRBF::SET);

            // Invoke alarm client callback with AlarmB identifier
            self.alarm_client.map(|client| {
                client.alarm_fired(AlarmId::AlarmB);
            });
        }
    }
}
