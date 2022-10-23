
use seccomp::*;

fn block(syscall: usize, cond: Compare) -> Rule {
    Rule::new(
        syscall,
        cond,
        Action::Errno(libc::EPERM)
    )
}



extern "C" fn start_sandbox() {
    let mut ctx = Context::default(Action::Allow).unwrap();

    let rules = &[
        block(
            105 /* setuid on x86_64 */,
            Compare::arg(0)
                .with(1000)
                .using(Op::Eq)
                .build().unwrap(),
        ),
        block(
            105 /* setuid on x86_64 */,
            Compare::arg(0)
                .with(1000)
                .using(Op::Eq)
                .build().unwrap(),
        )
    ];

    for r in rules {
        ctx.add_rule(rule).unwrap();
    }

    ctx.load().unwrap();

    let ret = unsafe { libc::setuid(1000) };
    println!("ret = {}, uid = {}", ret, unsafe { libc::getuid() });
}
