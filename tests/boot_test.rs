use polterdesk::boot::poll_until_ready;
use std::cell::Cell;
use std::time::Duration;

// The boot-readiness retry harness gates GPUI construction on the DirectWrite
// font subsystem becoming ready at logon. The OS probe is injected, so these
// tests exercise the control flow deterministically with a zero delay.

#[test]
fn returns_on_first_success_without_retrying() {
    let calls = Cell::new(0);
    let result = poll_until_ready(5, Duration::ZERO, || {
        calls.set(calls.get() + 1);
        Ok(())
    });
    assert_eq!(result, Ok(1));
    assert_eq!(calls.get(), 1, "must not probe again after first success");
}

#[test]
fn retries_until_probe_succeeds() {
    let calls = Cell::new(0);
    let result = poll_until_ready(10, Duration::ZERO, || {
        let n = calls.get() + 1;
        calls.set(n);
        if n >= 3 {
            Ok(())
        } else {
            Err(format!("font subsystem not ready (attempt {n})"))
        }
    });
    assert_eq!(result, Ok(3));
    assert_eq!(calls.get(), 3);
}

#[test]
fn gives_up_after_max_attempts_returning_last_error() {
    let calls = Cell::new(0);
    let result = poll_until_ready(4, Duration::ZERO, || {
        let n = calls.get() + 1;
        calls.set(n);
        Err(format!("boom {n}"))
    });
    assert_eq!(result, Err("boom 4".to_string()));
    assert_eq!(calls.get(), 4, "must attempt exactly max_attempts times");
}

#[test]
fn zero_max_attempts_never_probes() {
    let calls = Cell::new(0);
    let result = poll_until_ready(0, Duration::ZERO, || {
        calls.set(calls.get() + 1);
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(calls.get(), 0);
}
