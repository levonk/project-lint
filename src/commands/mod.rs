pub mod init;
pub mod lint;
pub mod watch;
pub mod hook;
pub mod configure;
pub mod configure_cmd;
pub mod install_hook;
pub mod logs;

pub use init::run as init;
pub use lint::run as lint;
pub use watch::run as watch;
pub use hook::run as hook;
pub use configure_cmd::run as configure;
pub use install_hook::run as install_hook;
pub use logs::run as logs;
