use core::cell::RefCell;
use core::ops::{Deref, DerefMut};

use embassy_time::{Duration, Ticker};
use alloc::string::{String};

use embassy_sync::blocking_mutex::{Mutex, raw::ThreadModeRawMutex};

use crate::thermometer::*;
use crate::gpio::*;
use crate::led::*;
use crate::util::*;


static ERROR_DETECTOR : Mutex<ThreadModeRawMutex, RefCell<Option<ErrorDetector>>> = Mutex::new(RefCell::new(None));
static CTRL_SEQ:  Mutex<ThreadModeRawMutex, RefCell<State>> = Mutex::new(RefCell::new(State::Initializing));

// Control heater 
const HEATER_CONTROL_TASK_TICK_MS : u32 = 50;
const HEATER_ON_DETECT_TIME_MS : u32 = 5000;
const HEATER_OFF_DETECT_TIME_MS : u32 = 1000;
const HEATER_ON_THRESHOLD_CELCIUS : f32 = 34.0;
const HEATER_OFF_THRESHOLD_CELCIUS : f32 = 35.0;

// Detect Error
const ERROR_OVERHEAT_DETECT_TIME_MS : u32 = 2000;
const ERROR_CTH_DISCONNECT_DETECT_TIME_MS : u32 = 1000;
const ERROR_OVERHEAT_THRESHOLD_CELCIUS : f32 = 45.0;
const ERROR_CTH_DISCONNECT_THRESHOLD_CELCIUS : f32 = -10.0;


#[derive(Copy, Clone)]
pub enum State
{
    Initializing,
    Heating,
    Saturating,
    Error,
}

struct HeaterControl
{
    heater_is_on : bool,
    heater_on_cnt : Counter,
    heater_off_cnt : Counter,
}

impl HeaterControl
{
    pub fn new() -> Self 
    {
        Self {
            heater_is_on:   false, 
            heater_on_cnt:  Counter::new(HEATER_ON_DETECT_TIME_MS / HEATER_CONTROL_TASK_TICK_MS),  // 50ms * 100 = 5000ms
            heater_off_cnt: Counter::new(HEATER_OFF_DETECT_TIME_MS / HEATER_CONTROL_TASK_TICK_MS)  // 50ms *  20 = 1000ms
        }
    }

    pub fn control(&mut self, temperature: f32)
    {
        self.detect_heater_on(temperature);
        self.detect_heater_off(temperature);
    }

    pub fn is_on(&self) -> bool 
    {
        self.heater_is_on
    }

    fn detect_heater_on(&mut self, temperature: f32)
    {
        if self.heater_is_on == false {
            if self.heater_on_cnt.count( temperature < HEATER_ON_THRESHOLD_CELCIUS ).is_reach_limit() {
                self.heater_on();
                self.heater_on_cnt.reset();
            }
        }
    }

    fn detect_heater_off(&mut self, temperature: f32)
    {
        if self.heater_is_on == true {
            if self.heater_off_cnt.count( temperature >= HEATER_OFF_THRESHOLD_CELCIUS ).is_reach_limit() {
                self.heater_off();
                self.heater_off_cnt.reset();
            }
        }
    }

    fn heater_on(&mut self)
    {
        self.heater_is_on = true;
    }

    fn heater_off(&mut self)
    {
        self.heater_is_on = false;
    }
}

#[derive(PartialEq, Clone)]
pub enum ErrorCode
{
    None,
    Heater1OverHeatError { errcode: u32, message: String },
    Heater1ThermistorDisconnectError { errcode: u32, message: String },
}

struct ErrorDetector
{
    heater_overheat: Counter,
    heater_thermistor_disconnect: Counter,
    detected_error: ErrorCode,
}

impl ErrorDetector
{
    pub fn new() -> Self
    {
        Self {
            heater_overheat: Counter::new(ERROR_OVERHEAT_DETECT_TIME_MS / HEATER_CONTROL_TASK_TICK_MS),                    // 50ms * 100 = 5000ms
            heater_thermistor_disconnect: Counter::new(ERROR_CTH_DISCONNECT_DETECT_TIME_MS / HEATER_CONTROL_TASK_TICK_MS), // 50ms * 20  = 1000ms
            detected_error: ErrorCode::None,
        }
    }

    pub fn heater_overheat(&mut self)
    {
        let heater1_temp = heater1_temperature();

        if self.heater_overheat.count( heater1_temp >= ERROR_OVERHEAT_THRESHOLD_CELCIUS ).is_reach_limit() {
            self.detected_error = ErrorCode::Heater1OverHeatError{ errcode: 1, message: String::from("Heater1 overheat error.") };
        }
    }

    pub fn heater_thermistor_disconnect(&mut self)
    {
        let heater1_temp = heater1_temperature();

        if self.heater_thermistor_disconnect.count( heater1_temp < ERROR_CTH_DISCONNECT_THRESHOLD_CELCIUS ).is_reach_limit() {
            self.detected_error = ErrorCode::Heater1ThermistorDisconnectError{ errcode: 2, message: String::from("Heater1 thermistor disconnected error.") };
        }
    }

    pub fn errcode(&self) -> ErrorCode
    {
        self.detected_error.clone()
    }
}


#[embassy_executor::task]
pub async fn controller_task()
{
    let mut heater_controller = HeaterControl::new();
    ERROR_DETECTOR.lock(|lock| {
        *(lock.borrow_mut()) = Some(ErrorDetector::new());
    });
    let mut ticker = Ticker::every(Duration::from_millis(HEATER_CONTROL_TASK_TICK_MS as u64));

    loop {
        // input/decision process 
        control_sequence(&mut heater_controller);
        detect_error();

        // output process
        set_led_status();

        ticker.next().await;
    }
}

fn control_sequence(mut heater_controller: &mut HeaterControl)
{
    CTRL_SEQ.lock( |lock| {
        let mut state = lock.borrow_mut();
        let next_state;
        match *state {
            State::Initializing => {
                next_state = heater_control(&mut heater_controller);
            }
            State::Heating => {
                next_state = heater_control(&mut heater_controller);
            }
            State::Saturating => {
                next_state = heater_control(&mut heater_controller);
            }
            State::Error => {
                next_state = control_on_error();
            }
        }

        *state = next_state;
    });
}

fn heater_control(heater_controller: &mut HeaterControl) -> State
{
    let heater1_temp = heater1_temperature();
    heater_controller.control( heater1_temp );

    if heater_controller.is_on() {
        on_heater_port();
        State::Heating
    }
    else {
        off_heater_port();
        State::Saturating
    }
}

fn control_on_error() -> State
{
    // heater force off.
    off_heater_port();

    // Fix error state.
    State::Error
}

fn detect_error()
{
    ERROR_DETECTOR.lock(|lock| {
        if let Some(ref mut e) = lock.borrow_mut().deref_mut().as_mut() {
            e.heater_overheat();
            e.heater_thermistor_disconnect();
        }
    });

    let errc = errcode();
    if errc != ErrorCode::None {
        CTRL_SEQ.lock( |lock| {
            *(lock.borrow_mut()) = State::Error;
        });
    }
}

fn set_led_status()
{
    CTRL_SEQ.lock( |lock| {
        match *(lock.borrow_mut()) {
            State::Initializing => {
                set_led(LedStatus::Stop);
            }
            State::Heating => {
                set_led(LedStatus::Heating);
            }
            State::Saturating => {
                set_led(LedStatus::Saturating);
            }
            State::Error => {
                set_led(LedStatus::Error);
            }
        }
    });   
}

pub fn errcode() -> ErrorCode
{
    let mut errcode = ErrorCode::None;
    let error_detector = ERROR_DETECTOR.lock(|lock| {
        if let Some(ref err) = lock.borrow_mut().deref().as_ref() {
            errcode = err.errcode();
        }
    });

    errcode
}

pub fn current_status() -> State
{
    CTRL_SEQ.lock( |lock| {
        *(lock.borrow_mut())
    })
}

