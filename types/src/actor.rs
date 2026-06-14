use crate::ProcessId;

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Actor {
    Userspace(ProcessId),
    Kernel,
}

impl From<ProcessId> for Actor {
    fn from(value: ProcessId) -> Self {
        Actor::Userspace(value)
    }
}

impl core::fmt::Debug for Actor {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Actor::Userspace(pid) => write!(f, "{}", pid.as_u16()),
            Actor::Kernel => write!(f, "kernel"),
        }
    }
}
