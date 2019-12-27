mod common;

use std::os::unix::io::AsRawFd;
use linux_io_uring::opcode::{ self, types };
use linux_io_uring::IoUring;
use common::Fd;


#[test]
fn test_poll_add() -> anyhow::Result<()> {
    let mut io_uring = IoUring::new(1)?;
    let (rp, wp) = nix::unistd::pipe()?;
    let (rp, wp) = (Fd(rp), Fd(wp));

    let entry = opcode::PollAdd::new(
        types::Target::Fd(rp.as_raw_fd()),
        libc::POLLIN
    );
    unsafe {
        io_uring
            .submission()
            .available()
            .push(entry.build().user_data(0x42))
            .ok()
            .expect("queue is full");
    }

    io_uring.submit()?;
    assert!(io_uring.completion().is_empty());

    nix::unistd::write(wp.as_raw_fd(), b"pipe")?;
    io_uring.submit_and_wait(1)?;
    let entry = io_uring
        .completion()
        .available()
        .next()
        .expect("queue is empty");
    assert_eq!(entry.result() as i16 & libc::POLLIN, libc::POLLIN);
    assert_eq!(entry.user_data(), 0x42);

    Ok(())
}

#[test]
fn test_poll_remove() -> anyhow::Result<()> {
    let mut io_uring = IoUring::new(1)?;
    let (rp, wp) = nix::unistd::pipe()?;
    let (rp, wp) = (Fd(rp), Fd(wp));

    let token = 0x43;

    let entry = opcode::PollAdd::new(
        types::Target::Fd(rp.as_raw_fd()),
        libc::POLLIN
    );
    unsafe {
        io_uring
            .submission()
            .available()
            .push(entry.build().user_data(token))
            .ok()
            .expect("queue is full");
    }

    io_uring.submit()?;
    assert!(io_uring.completion().is_empty());

    let entry = opcode::PollRemove::new(token);
    unsafe {
        io_uring
            .submission()
            .available()
            .push(entry.build().user_data(0x44))
            .ok()
            .expect("queue is full");
    }

    io_uring.submit()?;

    nix::unistd::write(wp.as_raw_fd(), b"pipe")?;

    io_uring.submit_and_wait(2)?;

    let mut cqes = io_uring
        .completion()
        .available()
        .collect::<Vec<_>>();
    cqes.sort_by_key(|cqe| cqe.user_data());
    assert_eq!(cqes[0].result(), -libc::ECANCELED);
    assert_eq!(cqes[0].user_data(), token);
    assert_eq!(cqes[1].result(), 0);
    assert_eq!(cqes[1].user_data(), 0x44);

    Ok(())
}