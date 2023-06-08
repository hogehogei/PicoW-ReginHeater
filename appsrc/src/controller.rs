use core::cell::RefCell;
use core::ops::{Deref, DerefMut};

use embassy_time::{Duration, Timer};
use alloc::string::{String};

use embassy_sync::blocking_mutex::{Mutex, raw::ThreadModeRawMutex};

use crate::thermometer::*;
use crate::gpio::*;
use crate::led::*;
use crate::util::*;


static ERROR_DETECTOR : Mutex<ThreadModeRawMutex, RefCell<Option<ErrorDetector>>> = Mutex::new(RefCell::new(None));
static CTRL_SEQ:  Mutex<ThreadModeRawMutex, RefCell<State>> = Mutex::new(RefCell::new(State::Initializing));

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
            heater_on_cnt:  Counter::new(100),
            heater_off_cnt: Counter::new(20)
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
            if self.heater_on_cnt.count( temperature < 34.0 ).is_reach_limit() {
                self.heater_on();
                self.heater_on_cnt.reset();
            }
        }
    }

    fn detect_heater_off(&mut self, temperature: f32)
    {
        if self.heater_is_on == true {
            if self.heater_off_cnt.count( temperature >= 35.0 ).is_reach_limit() {
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
            heater_overheat: Counter::new(100),
            heater_thermistor_disconnect: Counter::new(20),
            detected_error: ErrorCode::None,
        }
    }

    pub fn heater_overheat(&mut self)
    {
        let heater1_temp = heater1_temperature();

        if self.heater_overheat.count( heater1_temp >= 45.0 ).is_reach_limit() {
            self.detected_error = ErrorCode::Heater1OverHeatError{ errcode: 1, message: String::from("Heater1 overheat error.") };
        }
    }

    pub fn heater_thermistor_disconnect(&mut self)
    {
        let heater1_temp = heater1_temperature();

        if self.heater_thermistor_disconnect.count( heater1_temp < -10.0 ).is_reach_limit() {
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
    
    loop {
        // input/decision process 
        control_sequence(&mut heater_controller);
        detect_error();

        // output process
        set_led_status();

        Timer::after(Duration::from_millis(50)).await;
    }
}

fn control_sequence(mut heater_controller: &mut HeaterControl)
{
    CTRL_SEQ.lock( |lock| {
        let mut state = lock.borrow_mut();
        let mut next_state = State::Initializing;
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

fn heater_control(mut heater_controller: &mut HeaterControl) -> State
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

