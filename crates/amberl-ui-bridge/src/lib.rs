// SPDX-License-Identifier: MPL-2.0
//! Minimal in-process ABI for the WinUI comparison host. No framework/runtime service.
use amberl_ui_model::Model;
use std::{
    collections::BTreeMap,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};
struct State {
    model: Model,
    response: Vec<u8>,
}
static MODELS: OnceLock<Mutex<BTreeMap<u64, State>>> = OnceLock::new();
static IDS: AtomicU64 = AtomicU64::new(1);
fn models() -> &'static Mutex<BTreeMap<u64, State>> {
    MODELS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

#[unsafe(no_mangle)]
pub extern "C" fn amberl_ui_create() -> u64 {
    let id = IDS.fetch_add(1, Ordering::Relaxed);
    match models().lock() {
        Ok(mut models) if models.len() < 16 => {
            models.insert(
                id,
                State {
                    model: Model::default(),
                    response: vec![],
                },
            );
            id
        }
        _ => 0,
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn amberl_ui_destroy(id: u64) {
    if let Ok(mut models) = models().lock() {
        models.remove(&id);
    }
}

/// Execute one UTF-8 JSON command, retaining its response until the next command.
/// Return required response bytes or a negative error. Read never re-executes a command.
/// # Safety
/// Input must reference a valid buffer of the indicated length.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn amberl_ui_call(id: u64, input: *const u8, length: usize) -> isize {
    if input.is_null() || length > 16384 {
        return -1;
    }
    std::panic::catch_unwind(|| {
        let bytes=unsafe {std::slice::from_raw_parts(input,length)};
        let Ok(request)=serde_json::from_slice::<serde_json::Value>(bytes) else{return -1};
        let Ok(mut models)=models().lock() else{return -2};
        let Some(state)=models.get_mut(&id) else{return -2};
        state.response.clear();
        let model=&mut state.model;
        let command=request.get("command").and_then(|v|v.as_str()).unwrap_or("");
        let value=request.get("value").and_then(|v|v.as_str()).unwrap_or("");
        let result=match command {
            "scan"=>model.scan(value.into()),"cancel_scan"=>{model.cancel_scan();Ok(())},
            "filter"=>model.filter(value),"select"=>model.select(value),
            "start_task"=>{model.start_task();Ok(())},"cancel_task"=>{model.cancel_task();Ok(())},
            "snapshot"=>{model.tick();Ok(())},_=>Err("Unknown comparison command".into()),
        };
        let response=match result {
            Ok(())=>serde_json::json!({"ok":true,"snapshot":model.snapshot(request.get("rows").and_then(|v|v.as_bool()).unwrap_or(true))}),
            Err(error)=>serde_json::json!({"ok":false,"error":error}),
        };
        let Ok(encoded)=serde_json::to_vec(&response) else{return -3};
        if encoded.len()>16*1024*1024 {return -3;}
        let length=encoded.len(); state.response=encoded; length as isize
    }).unwrap_or(-4)
}

/// Copy the retained response without ticking or executing the command again.
/// # Safety
/// Output must reference a writable buffer of capacity bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn amberl_ui_read(id: u64, output: *mut u8, capacity: usize) -> isize {
    if output.is_null() {
        return -1;
    }
    let Ok(models) = models().lock() else {
        return -2;
    };
    let Some(state) = models.get(&id) else {
        return -2;
    };
    if capacity < state.response.len() {
        return -3;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(state.response.as_ptr(), output, state.response.len());
    }
    state.response.len() as isize
}

#[cfg(test)]
mod tests {
    use super::*;
    fn call(id: u64, request: &str) -> (isize, Vec<u8>) {
        let count = unsafe { amberl_ui_call(id, request.as_ptr(), request.len()) };
        assert!(count > 0);
        let mut bytes = vec![0; count as usize];
        assert_eq!(
            unsafe { amberl_ui_read(id, bytes.as_mut_ptr(), bytes.len()) },
            count
        );
        (count, bytes)
    }
    #[test]
    fn responses_are_stable_bounded_and_isolated() {
        let first = amberl_ui_create();
        let second = amberl_ui_create();
        assert_ne!(first, second);
        let (count, bytes) = call(first, r#"{"command":"start_task"}"#);
        let mut short = vec![0; count as usize - 1];
        assert_eq!(
            unsafe { amberl_ui_read(first, short.as_mut_ptr(), short.len()) },
            -3
        );
        let mut again = vec![0; count as usize];
        assert_eq!(
            unsafe { amberl_ui_read(first, again.as_mut_ptr(), again.len()) },
            count
        );
        assert_eq!(again, bytes); // A size/read retry cannot tick or repeat a mutation.
        let (_, other) = call(second, r#"{"command":"snapshot"}"#);
        let other: serde_json::Value = serde_json::from_slice(&other).unwrap();
        assert_eq!(other["snapshot"]["task_running"], false);
        call(second, r#"{"command":"filter","value":"测试中文"}"#);
        assert_eq!(unsafe { amberl_ui_call(first, [0xff].as_ptr(), 1) }, -1);
        assert_eq!(unsafe { amberl_ui_call(first, std::ptr::null(), 0) }, -1);
        amberl_ui_destroy(first);
        amberl_ui_destroy(second);
        assert_eq!(
            unsafe { amberl_ui_read(first, again.as_mut_ptr(), again.len()) },
            -2
        );
    }
}
