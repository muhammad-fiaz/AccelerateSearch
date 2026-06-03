#![forbid(unsafe_code)]
#![deny(unused_must_use)]

fn main() -> std::process::ExitCode {
    #[cfg(feature = "mimalloc")]
    {
        use mimalloc::MiMalloc;
        #[global_allocator]
        static GLOBAL: MiMalloc = MiMalloc;
    }

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("accelerate-worker")
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("failed to build tokio runtime: {e}");
            eprintln!("{}", errors::issue_tracker_hint(Some("tokio-init")));
            return std::process::ExitCode::from(1);
        }
    };

    runtime.block_on(server::run()).map_or_else(
        |e| {
            eprintln!("accelerate exited with error: {e}");
            eprintln!("{}", errors::issue_tracker_hint(Some(e.code())));
            std::process::ExitCode::from(1)
        },
        |()| std::process::ExitCode::SUCCESS,
    )
}
