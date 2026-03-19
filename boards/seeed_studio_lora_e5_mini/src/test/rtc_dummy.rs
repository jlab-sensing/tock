// Licensed under the Apache License, Version 2.0 or the MIT License.
// SPDX-License-Identifier: Apache-2.0 OR MIT
// Copyright Tock Contributors 2025.

//! RTC Test Module for STM32WLE5x
//!
//! This module provides comprehensive testing for the RTC driver including:
//! - GET → SET → GET datetime test sequence
//! - Alarm test
//! - Wakeup timer periodic test
//!
//! Test Sequence:
//! 1. GET current time (initial)
//! 2. SET new time
//! 3. GET time again (verify change)
//! 4. Alarm fires at second 15
//! 5. Wakeup timer fires 5 times every 2 seconds

use core::cell::Cell;
use kernel::debug;
use kernel::hil::date_time::{DateTime, DateTimeClient, DateTimeValues, DayOfWeek, Month};
use kernel::utilities::cells::OptionalCell;
use kernel::ErrorCode;

use stm32wle5jc::rtc::{
    AlarmId, AlarmMask, AlarmTime, Rtc, RtcAlarmClient, RtcWakeupClient, WakeupClockSource,
};

// ===========================================
// Test State Machine
// ===========================================

/// Test sequence state for tracking the complete test flow
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TestState {
    /// No test running
    Idle,
    /// Step 1: Initial GET to see current time
    FirstGet,
    /// Step 2: SET new time
    SettingTime,
    /// Step 3: Second GET to verify the change
    SecondGet,
    /// Step 4: Alarm test running
    AlarmWaiting,
    /// Step 5: Wakeup timer running
    WakeupRunning,
    /// All tests complete
    Complete,
}

// ===========================================
// RTC Test Client (for DateTime operations)
// ===========================================

/// Test client for RTC datetime operations with GET → SET → GET sequence
pub struct RtcTestClient<'a> {
    /// Reference to the RTC driver for chaining operations
    rtc: OptionalCell<&'a dyn DateTime<'a>>,
    /// Current test state
    test_state: Cell<TestState>,
    /// Reference to extended test client for triggering alarm test
    ext_client: OptionalCell<&'a RtcExtendedTestClient<'a>>,
}

impl<'a> RtcTestClient<'a> {
    /// Create a new RTC test client
    pub fn new() -> Self {
        Self {
            rtc: OptionalCell::empty(),
            test_state: Cell::new(TestState::Idle),
            ext_client: OptionalCell::empty(),
        }
    }

    /// Set the RTC reference for chaining operations
    pub fn set_rtc(&self, rtc: &'a dyn DateTime<'a>) {
        self.rtc.set(rtc);
    }

    /// Set the extended test client reference for triggering alarm test
    pub fn set_ext_client(&self, ext_client: &'a RtcExtendedTestClient<'a>) {
        self.ext_client.set(ext_client);
    }

    /// Get current test state
    pub fn get_state(&self) -> TestState {
        self.test_state.get()
    }

    /// Set test state
    pub fn set_state(&self, state: TestState) {
        self.test_state.set(state);
    }

    /// Start the GET → SET → GET test sequence
    pub fn start_get_set_get_test(&self) {
        debug!("");
        debug!("╔══════════════════════════════════════════════════════════╗");
        debug!("║         RTC GET → SET → GET TEST SEQUENCE                ║");
        debug!("╚══════════════════════════════════════════════════════════╝");

        self.test_state.set(TestState::FirstGet);
        self.rtc.map(|rtc| {
            debug!("");
            debug!("┌─────────────────────────────────────────────────────────┐");
            debug!("│ STEP 1: Reading CURRENT time (before set)               │");
            debug!("└─────────────────────────────────────────────────────────┘");
            match rtc.get_date_time() {
                Ok(()) => debug!("  → get_date_time initiated..."),
                Err(e) => debug!("  ✗ get_date_time failed: {:?}", e),
            }
        });
    }
}

impl DateTimeClient for RtcTestClient<'_> {
    /// Callback when get_date_time completes
    fn get_date_time_done(&self, datetime: Result<DateTimeValues, ErrorCode>) {
        match datetime {
            Ok(dt) => {
                debug!(
                    "  ✓ Time: {:04}-{:02}-{:02} {:02}:{:02}:{:02} ({:?})",
                    dt.year,
                    month_to_u8(dt.month),
                    dt.day,
                    dt.hour,
                    dt.minute,
                    dt.seconds,
                    dt.day_of_week
                );

                // Chain next operation based on test state
                match self.test_state.get() {
                    TestState::FirstGet => {
                        // After first get, do set
                        self.test_state.set(TestState::SettingTime);
                        self.rtc.map(|rtc| {
                            debug!("");
                            debug!("┌─────────────────────────────────────────────────────────┐");
                            debug!("│ STEP 2: SETTING new time to 2025-01-15 10:00:00         │");
                            debug!("└─────────────────────────────────────────────────────────┘");
                            let new_time = DateTimeValues {
                                year: 2025,
                                month: Month::January,
                                day: 15,
                                day_of_week: DayOfWeek::Wednesday,
                                hour: 10,
                                minute: 0,
                                seconds: 0,
                            };
                            match rtc.set_date_time(new_time) {
                                Ok(()) => debug!("  → set_date_time initiated..."),
                                Err(e) => debug!("  ✗ set_date_time failed: {:?}", e),
                            }
                        });
                    }
                    TestState::SecondGet => {
                        // GET → SET → GET complete, move to alarm test
                        debug!("");
                        debug!("╔══════════════════════════════════════════════════════════╗");
                        debug!("║  ✓ GET → SET → GET TEST COMPLETE!                        ║");
                        debug!("╚══════════════════════════════════════════════════════════╝");
                        self.test_state.set(TestState::AlarmWaiting);

                        // Trigger alarm test automatically
                        self.ext_client.map(|ext| {
                            let target = ext.alarm_target_second.get();
                            ext.setup_alarm(target);
                        });
                    }
                    TestState::WakeupRunning => {
                        // This is a time read during wakeup - just display it
                    }
                    _ => {}
                }
            }
            Err(e) => {
                debug!("  ✗ get_date_time error: {:?}", e);
                self.test_state.set(TestState::Idle);
            }
        }
    }

    /// Callback when set_date_time completes
    fn set_date_time_done(&self, result: Result<(), ErrorCode>) {
        match result {
            Ok(()) => {
                debug!("  ✓ set_date_time SUCCESS");

                // After set, do second get to verify
                if self.test_state.get() == TestState::SettingTime {
                    self.test_state.set(TestState::SecondGet);
                    self.rtc.map(|rtc| {
                        debug!("");
                        debug!("┌─────────────────────────────────────────────────────────┐");
                        debug!("│ STEP 3: Reading NEW time (after set) - verify change    │");
                        debug!("└─────────────────────────────────────────────────────────┘");
                        match rtc.get_date_time() {
                            Ok(()) => debug!("  → get_date_time initiated..."),
                            Err(e) => debug!("  ✗ get_date_time failed: {:?}", e),
                        }
                    });
                }
            }
            Err(e) => {
                debug!("  ✗ set_date_time error: {:?}", e);
                self.test_state.set(TestState::Idle);
            }
        }
    }
}

// ===========================================
// Extended RTC Test Client (for Wakeup/Alarm)
// ===========================================

/// Extended test client for wakeup timer and alarm testing
pub struct RtcExtendedTestClient<'a> {
    /// Reference to the RTC driver
    rtc: OptionalCell<&'a Rtc<'a>>,
    /// Counter for wakeup timer events
    wakeup_count: Cell<u32>,
    /// Maximum number of wakeup events
    max_wakeups: Cell<u32>,
    /// Reference to the basic test client for state coordination
    test_client: OptionalCell<&'a RtcTestClient<'a>>,
    /// Target alarm second (for setting alarm after wakeup completes)
    alarm_target_second: Cell<u8>,
}

impl<'a> RtcExtendedTestClient<'a> {
    /// Create a new extended RTC test client
    pub fn new() -> Self {
        Self {
            rtc: OptionalCell::empty(),
            wakeup_count: Cell::new(0),
            max_wakeups: Cell::new(5),
            test_client: OptionalCell::empty(),
            alarm_target_second: Cell::new(15),
        }
    }

    /// Set the RTC reference
    pub fn set_rtc(&self, rtc: &'a Rtc<'a>) {
        self.rtc.set(rtc);
    }

    /// Set the test client reference for state coordination
    pub fn set_test_client(&self, client: &'a RtcTestClient<'a>) {
        self.test_client.set(client);
    }

    /// Set the maximum number of wakeup events
    pub fn set_max_wakeups(&self, count: u32) {
        self.max_wakeups.set(count);
    }

    /// Reset the wakeup count
    pub fn reset_wakeup_count(&self) {
        self.wakeup_count.set(0);
    }

    /// Set the target alarm second
    pub fn set_alarm_target(&self, second: u8) {
        self.alarm_target_second.set(second);
    }

    /// Start the wakeup timer test
    pub fn start_wakeup_test(&self, interval_seconds: u16, count: u32) {
        debug!("");
        debug!("╔══════════════════════════════════════════════════════════╗");
        debug!("║         WAKEUP TIMER TEST                                ║");
        debug!(
            "║         {} wakeups every {} seconds                      ║",
            count, interval_seconds
        );
        debug!("╚══════════════════════════════════════════════════════════╝");

        self.reset_wakeup_count();
        self.set_max_wakeups(count);

        // Calculate reload value: period = (reload + 1) seconds
        let reload = if interval_seconds > 0 {
            interval_seconds - 1
        } else {
            0
        };

        self.rtc.map({
            |rtc| match rtc.set_wakeup_timer(reload, WakeupClockSource::CkSpre) {
                Ok(()) => {
                    debug!(
                        "  → Wakeup timer started ({}-second intervals)",
                        interval_seconds
                    );
                    debug!("  → Waiting for {} wakeup events...", count);
                    debug!("");
                }
                Err(e) => {
                    debug!("  ✗ Failed to configure wakeup timer: {:?}", e);
                }
            }
        });
    }

    /// Set up the alarm to fire at a specific second
    pub fn setup_alarm(&self, target_second: u8) {
        debug!("");
        debug!("╔══════════════════════════════════════════════════════════╗");
        debug!("║         ALARM TEST                                       ║");
        debug!(
            "║         Alarm A set to fire at second {:02}                 ║",
            target_second
        );
        debug!("╚══════════════════════════════════════════════════════════╝");

        let alarm_time = AlarmTime {
            hour: 0,
            minute: 0,
            seconds: target_second,
            day: 1,
            weekday_select: false,
            mask: AlarmMask {
                mask_seconds: false, // Match seconds
                mask_minutes: true,  // Ignore minutes
                mask_hours: true,    // Ignore hours
                mask_date: true,     // Ignore date
            },
        };

        self.rtc
            .map(|rtc| match rtc.set_alarm(AlarmId::AlarmA, alarm_time) {
                Ok(()) => {
                    debug!(
                        "  → Alarm A configured to fire at second {:02}",
                        target_second
                    );
                    debug!("  → Waiting for alarm...");
                    debug!("");
                }
                Err(e) => {
                    debug!("  ✗ Failed to configure Alarm A: {:?}", e);
                }
            });

        self.test_client.map(|client| {
            client.set_state(TestState::AlarmWaiting);
        });
    }
}

impl RtcAlarmClient for RtcExtendedTestClient<'_> {
    fn alarm_fired(&self, alarm: AlarmId) {
        match alarm {
            AlarmId::AlarmA => {
                debug!("");
                debug!("┌─────────────────────────────────────────────────────────┐");
                debug!("│ ★★★ ALARM A FIRED! ★★★                                  │");
                debug!("└─────────────────────────────────────────────────────────┘");

                // Disable the alarm after it fires
                self.rtc.map(|rtc| {
                    rtc.disable_alarm(AlarmId::AlarmA);
                });

                debug!("");
                debug!("╔══════════════════════════════════════════════════════════╗");
                debug!("║  ✓ ALARM TEST COMPLETE!                                  ║");
                debug!("╚══════════════════════════════════════════════════════════╝");

                // Move to wakeup test
                self.test_client.map(|client| {
                    client.set_state(TestState::WakeupRunning);
                });

                // Start wakeup timer test
                debug!("");
                self.start_wakeup_test(2, 5);
            }
            AlarmId::AlarmB => {
                debug!("*** ALARM B FIRED! ***");
            }
        }
    }
}

impl RtcWakeupClient for RtcExtendedTestClient<'_> {
    fn wakeup_fired(&self) {
        let count = self.wakeup_count.get() + 1;
        self.wakeup_count.set(count);

        debug!(
            "┌─ Wakeup #{} ─────────────────────────────────────────────┐",
            count
        );

        // Read and print current time
        self.rtc.map(|rtc| {
            match rtc.get_date_time() {
                Ok(()) => {
                    // Time will be printed in the callback
                }
                Err(e) => {
                    debug!("│  ✗ get_date_time failed: {:?}", e);
                }
            }
        });

        debug!("└─────────────────────────────────────────────────────────┘");

        // Check if we've reached max wakeups
        if count >= self.max_wakeups.get() {
            self.rtc.map(|rtc| rtc.disable_wakeup_timer());

            debug!("");
            debug!("╔══════════════════════════════════════════════════════════╗");
            debug!(
                "║  ✓ WAKEUP TIMER TEST COMPLETE! ({} wakeups)               ║",
                count
            );
            debug!("╚══════════════════════════════════════════════════════════╝");

            // Move to complete state
            self.test_client.map(|client| {
                client.set_state(TestState::Complete);
            });

            debug!("");
            debug!("╔══════════════════════════════════════════════════════════╗");
            debug!("║  ✓ ALL RTC TESTS COMPLETE!                               ║");
            debug!("╚══════════════════════════════════════════════════════════╝");
            debug!("");
        }
    }
}

// ===========================================
// Helper Functions
// ===========================================

/// Convert Month enum to numeric value for display
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

// ===========================================
// Main Test Function
// ===========================================

/// Run the complete RTC test sequence:
/// 1. GET → SET → GET (datetime test)
/// 2. Alarm (fires at second 15)
/// 3. Wakeup timer (5 times, every 2 seconds)
///
/// # Arguments
/// * `rtc` - Reference to the RTC driver
/// * `test_client` - Reference to the datetime test client
/// * `ext_client` - Reference to the extended test client
pub fn run_complete_rtc_test<'a>(
    _rtc: &'a Rtc<'a>,
    test_client: &'a RtcTestClient<'a>,
    ext_client: &'a RtcExtendedTestClient<'a>,
) {
    debug!("");
    debug!("╔══════════════════════════════════════════════════════════╗");
    debug!("║                                                          ║");
    debug!("║           STM32WLE5x RTC COMPREHENSIVE TEST              ║");
    debug!("║                                                          ║");
    debug!("║  Test Sequence:                                          ║");
    debug!("║  1. GET current time                                     ║");
    debug!("║  2. SET new time (2025-01-15 10:00:00)                   ║");
    debug!("║  3. GET time again (verify change)                       ║");
    debug!("║  4. Alarm: fires at second 15                            ║");
    debug!("║  5. Wakeup timer: 5 times, every 2 seconds               ║");
    debug!("║                                                          ║");
    debug!("╚══════════════════════════════════════════════════════════╝");
    debug!("");

    // Set alarm target to second 15
    ext_client.set_alarm_target(15);

    // Start the GET → SET → GET sequence
    test_client.start_get_set_get_test();
}

/// Start the alarm test (called after GET → SET → GET completes)
pub fn start_alarm_test<'a>(ext_client: &'a RtcExtendedTestClient<'a>) {
    // Set up alarm to fire at second 15
    let target = ext_client.alarm_target_second.get();
    ext_client.setup_alarm(target);
}

/// Start the wakeup timer test (called after alarm fires)
pub fn start_wakeup_and_alarm_test<'a>(ext_client: &'a RtcExtendedTestClient<'a>) {
    // Start wakeup timer: 5 times, every 2 seconds
    ext_client.start_wakeup_test(2, 5);
}
