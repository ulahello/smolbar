use crate::protocol::Header;

pub trait Bar {
    fn header(&self) -> Header;

    fn cont(&mut self);
}
