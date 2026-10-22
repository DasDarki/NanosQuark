use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use jiff::{Timestamp, Zoned};
use mlua::prelude::*;
use quark_core::{Dispatcher, Options, Resource, create_userdata, runtime};
use tokio::sync::Notify;

use crate::cron::Cron;
use crate::datetime::{to_object, zone_option};

const MAX_SLEEP: Duration = Duration::from_secs(30);

struct State {
    active: AtomicBool,
    stopped: Notify,
    next: Mutex<Option<Zoned>>,
}

impl Resource for State {
    fn close(&self) {
        self.active.store(false, Ordering::SeqCst);
        self.stopped.notify_waiters();
    }
}

pub struct Schedule {
    state: Arc<State>,
    expression: String,
}

async fn run(cron: Cron, state: Arc<State>, dispatcher: Dispatcher, callback: Arc<LuaRegistryKey>) {
    let mut last = Zoned::now();
    while state.active.load(Ordering::SeqCst) {
        let Some(next) = cron.next(&last) else {
            break;
        };
        if let Ok(mut stored) = state.next.lock() {
            *stored = Some(next.clone());
        }

        loop {
            if !state.active.load(Ordering::SeqCst) {
                return;
            }
            let remaining = next.timestamp().as_millisecond() - Timestamp::now().as_millisecond();
            if remaining <= 0 {
                break;
            }
            let wait = Duration::from_millis(remaining as u64).min(MAX_SLEEP);
            tokio::select! {
                _ = tokio::time::sleep(wait) => {}
                _ = state.stopped.notified() => {}
            }
        }

        let callback = Arc::clone(&callback);
        let time = next.clone();
        let current = Arc::clone(&state);
        dispatcher.post(move |lua| {
            if !current.active.load(Ordering::SeqCst) {
                return Ok(());
            }
            let function: LuaFunction = lua.registry_value(&callback)?;
            function.call::<()>(to_object(time))
        });
        last = next;
    }
    state.active.store(false, Ordering::SeqCst);
    if let Ok(mut stored) = state.next.lock() {
        *stored = None;
    }
}

impl LuaUserData for Schedule {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("Stop", |_, schedule, ()| {
            schedule.state.close();
            Ok(())
        });
        methods.add_method("IsActive", |_, schedule, ()| {
            Ok(schedule.state.active.load(Ordering::SeqCst))
        });
        methods.add_method("GetNext", |_, schedule, ()| {
            if !schedule.state.active.load(Ordering::SeqCst) {
                return Ok(None);
            }
            let next = schedule
                .state
                .next
                .lock()
                .map(|next| next.clone())
                .unwrap_or(None);
            Ok(next.map(to_object))
        });
        methods.add_method("GetExpression", |_, schedule, ()| {
            Ok(schedule.expression.clone())
        });
    }
}

pub fn register(lua: &Lua, module: &LuaTable) -> LuaResult<()> {
    module.set(
        "Schedule",
        lua.create_function(
            |lua, (expression, callback, options): (String, LuaFunction, Options)| {
                let cron = match Cron::parse(&expression, zone_option(options.get("Zone")?)?) {
                    Ok(cron) => cron,
                    Err(message) => return (LuaNil, message).into_lua_multi(lua),
                };
                let dispatcher = Dispatcher::get(lua)?;
                let first = cron.next(&Zoned::now());
                let state = Arc::new(State {
                    active: AtomicBool::new(first.is_some()),
                    stopped: Notify::new(),
                    next: Mutex::new(first),
                });
                let weak: Weak<dyn Resource> = Arc::downgrade(&state) as Weak<dyn Resource>;
                dispatcher.track(lua, weak)?;

                let callback = Arc::new(lua.create_registry_value(callback)?);
                runtime().spawn(run(cron.clone(), Arc::clone(&state), dispatcher, callback));

                create_userdata(
                    lua,
                    Schedule {
                        state,
                        expression: cron.expression().to_owned(),
                    },
                )?
                .into_lua_multi(lua)
            },
        )?,
    )
}
