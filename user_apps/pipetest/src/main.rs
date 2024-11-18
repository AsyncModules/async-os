#![feature(anonymous_pipe)]
#![feature(noop_waker)]
#![feature(future_join)]

mod implementation;
use implementation::pipe_test;

fn main() {
    pipe_test();
}
