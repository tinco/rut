#![feature(rustc_private)]
#![feature(nll)]
#![feature(once_cell)]
#![recursion_limit = "256"]

extern crate rustc_driver;
extern crate rustc_session;
extern crate rustc_interface;
extern crate libc;
// extern crate rustc_data_structures;  // The Rust compiler interface.

use std::env;
use std::process;
use rustc_driver::{Compilation};
use rustc_interface::interface;

use rustc_session::config::{ErrorOutputType};

// use rustc_data_structures::profiling::{get_resident_set_size, print_time_passes_entry};

#[cfg(all(unix, any(target_env = "gnu", target_os = "macos")))]
mod signal_handler {
    extern "C" {
        fn backtrace_symbols_fd(
            buffer: *const *mut libc::c_void,
            size: libc::c_int,
            fd: libc::c_int,
        );
    }

    extern "C" fn print_stack_trace(_: libc::c_int) {
        const MAX_FRAMES: usize = 256;
        static mut STACK_TRACE: [*mut libc::c_void; MAX_FRAMES] =
            [std::ptr::null_mut(); MAX_FRAMES];
        unsafe {
            let depth = libc::backtrace(STACK_TRACE.as_mut_ptr(), MAX_FRAMES as i32);
            if depth == 0 {
                return;
            }
            backtrace_symbols_fd(STACK_TRACE.as_ptr(), depth, 2);
        }
    }

    // When an error signal (such as SIGABRT or SIGSEGV) is delivered to the
    // process, print a stack trace and then exit.
    pub(super) fn install() {
        unsafe {
            const ALT_STACK_SIZE: usize = libc::MINSIGSTKSZ + 64 * 1024;
            let mut alt_stack: libc::stack_t = std::mem::zeroed();
            alt_stack.ss_sp =
                std::alloc::alloc(std::alloc::Layout::from_size_align(ALT_STACK_SIZE, 1).unwrap())
                    as *mut libc::c_void;
            alt_stack.ss_size = ALT_STACK_SIZE;
            libc::sigaltstack(&alt_stack, std::ptr::null_mut());

            let mut sa: libc::sigaction = std::mem::zeroed();
            sa.sa_sigaction = print_stack_trace as libc::sighandler_t;
            sa.sa_flags = libc::SA_NODEFER | libc::SA_RESETHAND | libc::SA_ONSTACK;
            libc::sigemptyset(&mut sa.sa_mask);
            libc::sigaction(libc::SIGSEGV, &sa, std::ptr::null_mut());
        }
    }
}

#[cfg(not(all(unix, any(target_env = "gnu", target_os = "macos"))))]
mod signal_handler {
    pub(super) fn install() {}
}

pub fn main() -> ! {
    eprintln!("Rut compiler");
    // let start_time = Instant::now();
    // let start_rss = get_resident_set_size();
    rustc_driver::init_rustc_env_logger();
    signal_handler::install();
    let mut callbacks = RutCallbacks{};
    rustc_driver::install_ice_hook();
    let exit_code = rustc_driver::catch_with_exit_code(|| {
        let args = env::args_os()
            .enumerate()
            .map(|(i, arg)| {
                arg.into_string().unwrap_or_else(|arg| {
                    rustc_session::early_error(
                        ErrorOutputType::default(),
                        &format!("argument {} is not valid Unicode: {:?}", i, arg),
                    )
                })
            })
            .collect::<Vec<_>>();
        rustc_driver::RunCompiler::new(&args, &mut callbacks).run()
    });

    // if callbacks.time_passes {
    //     let end_rss = get_resident_set_size();
    //     print_time_passes_entry("total", start_time.elapsed(), start_rss, end_rss);
    // }

    process::exit(exit_code)
}

struct RutCallbacks {
}

impl rustc_driver::Callbacks for RutCallbacks {
    fn after_expansion<'tcx>(
        &mut self,
        _compiler: &interface::Compiler,
        queries: &'tcx rustc_interface::Queries<'tcx>,
    ) -> Compilation {
        let krate = queries.parse().expect("should've made it").peek_mut();
        eprintln!("Got a crate: {:?}", krate);
        Compilation::Continue
    }
}