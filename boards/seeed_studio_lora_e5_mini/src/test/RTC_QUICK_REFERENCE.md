# RTC Quick Reference Guide

## Test Execution Flow (Visual)

```
┌─────────────────────────────────────────────────────────────────┐
│                    SYSTEM STARTUP                               │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  main.rs: Initialize RTC Hardware                               │
│  - Create RTC instance                                          │
│  - Enable interrupts (NVIC)                                     │
│  - Call rtc_init()                                              │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  main.rs: Create and Register Clients                           │
│  - RtcTestClient (datetime)                                     │
│  - RtcExtendedTestClient (alarm/wakeup)                         │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  main.rs: Start Tests                                           │
│  - run_complete_rtc_test()                                      │
│  - start_alarm_test()                                           │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
╔═════════════════════════════════════════════════════════════════╗
║                    TEST PHASE 1: GET → SET → GET                ║
╚═════════════════════════════════════════════════════════════════╝
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Step 1: First GET                                              │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ start_get_set_get_test()                                 │   │
│  │   → State: Idle → FirstGet                               │   │
│  │   → rtc.get_date_time()                                  │   │
│  └──────────────────────────────────────────────────────────┘   │
│                              │                                  │
│                              ▼                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ [Hardware reads TR/DR registers]                         │   │
│  │ [Converts BCD to decimal]                                │   │
│  │ [Schedules deferred call]                                │   │
│  └──────────────────────────────────────────────────────────┘   │
│                              │                                  │
│                              ▼                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ get_date_time_done(datetime)                             │   │
│  │   → Display: "2000-01-01 00:00:00"                       │   │
│  │   → State: FirstGet → SettingTime                        │   │
│  │   → rtc.set_date_time(new_time)                          │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Step 2: SET Time                                               │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ [Converts decimal to BCD]                                │   │
│  │ [Enters init mode]                                       │   │
│  │ [Writes to TR/DR registers]                              │   │ 
│  │ [Exits init mode]                                        │   │
│  └──────────────────────────────────────────────────────────┘   │
│                              │                                  │
│                              ▼                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ set_date_time_done(Ok)                                   │   │
│  │   → Display: "set_date_time SUCCESS"                     │   │
│  │   → State: SettingTime → SecondGet                       │   │
│  │   → rtc.get_date_time()                                  │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Step 3: Second GET (Verify)                                    │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ [Reads TR/DR registers again]                            │   │
│  │ [Converts BCD to decimal]                                │   │
│  └──────────────────────────────────────────────────────────┘   │
│                              │                                  │
│                              ▼                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ get_date_time_done(datetime)                             │   │
│  │   → Display: "2025-01-15 10:00:00"                       │   │
│  │   → State: SecondGet → AlarmWaiting                      │   │
│  │   → Display: "GET → SET → GET COMPLETE!"                 │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
╔═════════════════════════════════════════════════════════════════╗
║                    TEST PHASE 2: ALARM                          ║
╚═════════════════════════════════════════════════════════════════╝
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Alarm Setup                                                    │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ start_alarm_test()                                       │   │
│  │   → setup_alarm(15)                                      │   │
│  │   → Creates AlarmTime struct                             │   │
│  │   → rtc.set_alarm(AlarmId::AlarmA, alarm_time)           │   │
│  └──────────────────────────────────────────────────────────┘   │
│                              │                                  │
│                              ▼                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ [Converts decimal to BCD]                                │   │
│  │ [Writes to ALRMAR register]                              │   │
│  │ [Enables alarm in CR register]                           │   │
│  │ [Enables alarm interrupt]                                │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Waiting for Alarm...                                           │
│  [RTC counts: 10:00:00 → 10:00:15]                              │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Alarm Fires!                                                   │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ [Hardware detects time match]                            │   │
│  │ [Generates interrupt]                                    │   │
│  │ [Interrupt handler called]                               │   │
│  └──────────────────────────────────────────────────────────┘   │
│                              │                                  │
│                              ▼                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ alarm_fired(AlarmId::AlarmA)                             │   │
│  │   → Display: "★★★ ALARM A FIRED! ★★★"                 │   │
│  │   → Disable alarm                                        │   │
│  │   → State: AlarmWaiting → WakeupRunning                  │   │
│  │   → start_wakeup_test(2, 5)                              │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
╔═════════════════════════════════════════════════════════════════╗
║                    TEST PHASE 3: WAKEUP TIMER                   ║
╚═════════════════════════════════════════════════════════════════╝
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Wakeup Setup                                                   │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ start_wakeup_test(2, 5)                                  │   │
│  │   → max_wakeups = 5                                      │   │
│  │   → reload = 2 - 1 = 1                                   │   │
│  │   → rtc.set_wakeup_timer(1, CkSpre)                      │   │
│  └──────────────────────────────────────────────────────────┘   │
│                              │                                  │
│                              ▼                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ [Disables wakeup timer]                                  │   │
│  │ [Writes reload to WUTR]                                  │   │
│  │ [Selects CkSpre clock source]                            │   │
│  │ [Enables wakeup timer]                                   │   │
│  │ [Enables wakeup interrupt]                               │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Wakeup Events (5 times)                                        │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ [Timer counts down: 1 → 0]                               │   │
│  │ [Generates interrupt]                                    │   │
│  │ wakeup_fired()                                           │   │
│  │   → count = 1                                            │   │
│  │   → Display: "Wakeup #1"                                 │   │
│  │   → Display time: "10:00:17"                             │   │
│  └──────────────────────────────────────────────────────────┘   │
│                              │                                  │
│                    [Repeats 4 more times]                       │
│                              │                                  │
│                              ▼                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ wakeup_fired() [5th time]                                │   │
│  │   → count = 5                                            │   │
│  │   → count >= max_wakeups                                 │   │
│  │   → Disable wakeup timer                                 │   │
│  │   → State: WakeupRunning                                 │   │
│  │   → Display: "WAKEUP TIMER TEST COMPLETE!"               │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
╔═════════════════════════════════════════════════════════════════╗
║                    ALL TESTS COMPLETE                           ║
╚═════════════════════════════════════════════════════════════════╝
```

## File Responsibilities

```
┌─────────────────────────────────────────────────────────────────┐
│  chips/stm32wle5xx/src/rtc.rs                                   │
│  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━   │
│  • Hardware register definitions                                │
│  • BCD conversion functions                                     │
│  • Core RTC operations:                                         │
│    - get_date_time() / set_date_time()                          │
│    - set_alarm() / disable_alarm()                              │
│    - set_wakeup_timer() / disable_wakeup_timer()                │
│  • Interrupt handling                                           │
│  • Client callback invocation                                   │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│  boards/.../src/test/rtc_dummy.rs                               │
│  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━   │
│  • Test client implementations:                                 │
│    - RtcTestClient (datetime)                                   │
│    - RtcExtendedTestClient (alarm/wakeup)                       │
│  • State machine (TestState enum)                               │
│  • Test orchestration functions                                 │
│  • Callback implementations                                     │
│  • Test helper functions                                        │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│  boards/.../src/main.rs                                         │
│  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━   │
│  • RTC hardware initialization                                  │
│  • Interrupt enable (NVIC)                                      │
│  • Client creation and registration                             │
│  • Test invocation                                              │
│  • Board-specific setup                                         │
└─────────────────────────────────────────────────────────────────┘
```

## Callback Summary

| Callback | Triggered By | Purpose |
|----------|--------------|---------|
| `get_date_time_done()` | GET operation complete | Receive datetime, chain next operation |
| `set_date_time_done()` | SET operation complete | Confirm success, trigger verification |
| `alarm_fired()` | Alarm time reached | Handle alarm event, start wakeup test |
| `wakeup_fired()` | Wakeup timer expires | Count events, check if done |
