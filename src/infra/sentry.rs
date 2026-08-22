pub fn init(sentry_dsn: &str) -> sentry::ClientInitGuard {
    let guard = sentry::init((
        sentry_dsn,
        sentry::ClientOptions::default()
            .maybe_release(sentry::release_name!())
            // Capture user IPs and potentially sensitive headers when using HTTP server integrations
            // see https://docs.sentry.io/platforms/rust/data-management/data-collected for more info
            .send_default_pii(true),
    ));

    // sentry_panic registers a hook that calls writeln!(stderr) and panics
    // on error (e.g. Broken pipe / os error 32).  Replace it with a safe
    // version that still prints to stderr but silently drops write errors
    // and runs the sentry hook inside catch_unwind in case its own stderr
    // write fails.
    //
    // Additionally, filter out Broken pipe panics entirely — they are a
    // common side-effect of stderr being closed (e.g. terminal exits, Fly.io
    // restarts) and are never actionable.
    let sentry_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        use std::io::Write;
        let _ = writeln!(std::io::stderr(), "{info}");

        // Don't forward broken-pipe panics to Sentry — they're noise.
        if is_broken_pipe(info) {
            return;
        }

        // If sentry's hook panics while writing to stderr we don't want to
        // double-panic and abort.
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            sentry_hook(info);
        }));
    }));

    guard
}

/// Returns `true` if the panic was caused by a broken pipe on stderr/stdout.
fn is_broken_pipe(info: &std::panic::PanicHookInfo<'_>) -> bool {
    let msg = info
        .payload()
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| info.payload().downcast_ref::<String>().map(|s| s.as_str()));

    msg.is_some_and(|m| m.contains("Broken pipe") || m.contains("os error 32"))
}
